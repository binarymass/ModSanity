//! Mod management - installation, deployment, and conflict handling

mod archive;
pub mod auto_categorize;
mod conflicts;
mod deploy;
pub mod fomod;

pub use archive::*;
pub use auto_categorize::*;
pub use conflicts::*;
pub use deploy::*;

use crate::config::Config;
use crate::db::{Database, ModFileRecord, ModRecord};
use anyhow::{bail, Context, Result};
use regex_lite::Regex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;

/// Result of an installation attempt
#[derive(Debug)]
pub enum InstallResult {
    /// Installation completed successfully
    Completed(InstalledMod),
    /// FOMOD wizard is required - contains context for launching wizard
    RequiresWizard(FomodInstallContext),
}

/// Context for FOMOD installation that requires wizard interaction
#[derive(Debug, Clone)]
pub struct FomodInstallContext {
    pub game_id: String,
    pub mod_name: String,
    pub version: String,
    pub staging_path: PathBuf,
    pub installer: fomod::FomodInstaller,
    pub priority: i32,
    /// If Some, this is a reconfiguration of existing mod with this ID
    pub existing_mod_id: Option<i64>,
    /// Nexus Mods mod ID (if downloaded from Nexus)
    pub nexus_mod_id: Option<i64>,
    /// Nexus Mods file ID (if downloaded from Nexus)
    pub nexus_file_id: Option<i64>,
}

/// Represents an installed mod
///
/// # Priority System
///
/// Mods are deployed in priority order for conflict resolution:
/// - **Higher number = Higher priority** (wins file conflicts)
/// - Default priority for new mods: `max(existing) + 1`
/// - Priority determines which mod's files overwrite others during deployment
/// - Example: Mod with priority 10 overwrites files from mod with priority 5
///
/// This is **mod load order** (file conflict resolution), distinct from
/// **plugin load order** (.esp/.esm/.esl files loaded by the game engine).
#[derive(Debug, Clone)]
pub struct InstalledMod {
    pub id: i64,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub enabled: bool,

    /// Priority for conflict resolution. Higher number = higher priority (wins conflicts).
    /// New mods get max(existing) + 1 by default.
    pub priority: i32,

    pub nexus_mod_id: Option<i64>,
    pub nexus_file_id: Option<i64>,
    pub file_count: i32,
    pub install_path: PathBuf,
    pub category_id: Option<i64>,
}

/// Summary of a staging rescan operation.
#[derive(Debug, Default, Clone, Copy)]
pub struct RescanStats {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub failed: usize,
}

impl From<ModRecord> for InstalledMod {
    fn from(r: ModRecord) -> Self {
        Self {
            id: r.id.unwrap_or(0),
            name: r.name,
            version: r.version,
            author: r.author,
            enabled: r.enabled,
            priority: r.priority,
            nexus_mod_id: r.nexus_mod_id,
            nexus_file_id: r.nexus_file_id,
            file_count: r.file_count,
            install_path: PathBuf::from(r.install_path),
            category_id: r.category_id,
        }
    }
}

/// Mod manager handles installation, enabling, and deployment
pub struct ModManager {
    config: Arc<RwLock<Config>>,
    db: Arc<Database>,
}

impl ModManager {
    /// Create a new ModManager
    pub fn new(config: Arc<RwLock<Config>>, db: Arc<Database>) -> Self {
        Self { config, db }
    }

    /// Get staging directory for a game
    async fn staging_dir(&self, game_id: &str) -> PathBuf {
        self.config.read().await.game_staging_dir(game_id)
    }

    /// List all installed mods for a game
    pub async fn list_mods(&self, game_id: &str) -> Result<Vec<InstalledMod>> {
        let records = self.db.get_mods_for_game(game_id)?;
        Ok(records.into_iter().map(InstalledMod::from).collect())
    }

    /// Get a specific mod
    pub async fn get_mod(&self, game_id: &str, name: &str) -> Result<InstalledMod> {
        let record = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;
        Ok(InstalledMod::from(record))
    }

    /// Install a mod from an archive
    pub async fn install_from_archive(
        &self,
        game_id: &str,
        archive_path: &str,
        progress_callback: Option<ProgressCallback>,
        nexus_mod_id: Option<i64>,
        nexus_file_id: Option<i64>,
        mod_name_hint: Option<&str>,
    ) -> Result<InstallResult> {
        let archive_path = Path::new(archive_path);
        if !archive_path.exists() {
            bail!("Archive not found: {}", archive_path.display());
        }

        // Extract archive info
        let archive_name = archive_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Parse mod name and version from filename
        let (parsed_name, version) = Self::parse_mod_name(archive_name);
        let name = mod_name_hint
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.replace('_', " ").replace('-', " ").trim().to_string())
            .unwrap_or(parsed_name);

        // Resolve Nexus ID from explicit argument first, then filename fallback.
        let resolved_nexus_mod_id = nexus_mod_id.or_else(|| Self::parse_nexus_ids(archive_name).map(|(mod_id, _)| mod_id));

        // Guard against duplicate installs of the same Nexus mod under different names.
        // This also upgrades legacy unresolved numeric-name installs (e.g. "165498")
        // to the resolved display name so queue/import resolution does not create duplicates.
        if let Some(mid) = resolved_nexus_mod_id {
            let existing = self.db.find_mods_by_nexus_ids(game_id, &[mid])?;
            if let Some(existing_mod) = existing.get(&mid) {
                let is_numeric_name = existing_mod
                    .name
                    .trim()
                    .chars()
                    .all(|c| c.is_ascii_digit());
                if is_numeric_name
                    && !existing_mod.name.eq_ignore_ascii_case(&name)
                    && self.db.get_mod(game_id, &name)?.is_none()
                {
                    let mut upgraded = existing_mod.clone();
                    upgraded.name = name.clone();
                    upgraded.nexus_mod_id = Some(mid);
                    if upgraded.nexus_file_id.is_none() {
                        upgraded.nexus_file_id = nexus_file_id;
                    }
                    upgraded.updated_at = chrono::Utc::now().to_rfc3339();
                    self.db.update_mod(&upgraded)?;
                    bail!(
                        "Mod with Nexus ID {} is already installed; upgraded legacy entry to '{}'",
                        mid,
                        name
                    );
                }
                bail!(
                    "Mod with Nexus ID {} is already installed as '{}'",
                    mid,
                    existing_mod.name
                );
            }

            let legacy_name = mid.to_string();
            if let Some(existing_legacy) = self.db.get_mod(game_id, &legacy_name)? {
                if existing_legacy.nexus_mod_id.is_none()
                    || existing_legacy.nexus_mod_id == Some(mid)
                {
                    let mut upgraded = existing_legacy.clone();
                    upgraded.nexus_mod_id = Some(mid);
                    if upgraded.nexus_file_id.is_none() {
                        upgraded.nexus_file_id = nexus_file_id;
                    }
                    if !upgraded.name.eq_ignore_ascii_case(&name)
                        && self.db.get_mod(game_id, &name)?.is_none()
                    {
                        upgraded.name = name.clone();
                    }
                    upgraded.updated_at = chrono::Utc::now().to_rfc3339();
                    self.db.update_mod(&upgraded)?;
                    bail!(
                        "Mod with Nexus ID {} is already installed; upgraded legacy entry to '{}'",
                        mid,
                        upgraded.name
                    );
                }
            }
        }

        // Check if already installed by resolved display name.
        if self.db.get_mod(game_id, &name)?.is_some() {
            bail!("Mod '{}' is already installed", name);
        }

        // Create staging directory for this mod
        let staging = self.staging_dir(game_id).await.join(&name);
        tokio::fs::create_dir_all(&staging)
            .await
            .context("Failed to create staging directory")?;

        // Extract archive
        tracing::info!("Extracting {} to {}", archive_path.display(), staging.display());
        extract_archive(archive_path, &staging, progress_callback).await?;

        // Check for FOMOD installer (including nested structures)
        if fomod::has_fomod(&staging) {
            tracing::info!("FOMOD installer detected for {}", name);
            match fomod::FomodInstaller::load(&staging) {
                Ok(installer) => {
                    // Check if wizard is actually needed
                    if installer.requires_wizard() {
                        tracing::info!("FOMOD requires wizard interaction");
                        let priority = self.next_priority(game_id).await?;
                        return Ok(InstallResult::RequiresWizard(FomodInstallContext {
                            game_id: game_id.to_string(),
                            mod_name: name,
                            version,
                            staging_path: staging,
                            installer,
                            priority,
                            existing_mod_id: None,
                            nexus_mod_id: resolved_nexus_mod_id,
                            nexus_file_id,
                        }));
                    } else {
                        tracing::info!("FOMOD has only defaults, proceeding with auto-install");
                        // Continue with normal installation
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse FOMOD installer: {}, falling back to simple install", e);
                    // Continue with normal installation
                }
            }
        }

        // Find the data root (handle nested folders)
        let data_root = find_data_root(&staging)?;

        // If data root is different, move files
        if data_root != staging {
            move_contents(&data_root, &staging).await?;
        }

        // Collect file list
        let files = collect_files(&staging)?;

        // Create database record
        let now = chrono::Utc::now().to_rfc3339();
        let record = ModRecord {
            id: None,
            game_id: game_id.to_string(),
            name: name.clone(),
            version: version.clone(),
            author: None,
            description: None,
            nexus_mod_id: resolved_nexus_mod_id,
            nexus_file_id,
            install_path: staging.to_string_lossy().to_string(),
            enabled: true,
            priority: self.next_priority(game_id).await?,
            file_count: files.len() as i32,
            installed_at: now.clone(),
            updated_at: now,
            category_id: None,
        };

        let mod_id = self.db.insert_mod(&record)?;

        // Insert file records
        let file_records: Vec<ModFileRecord> = files
            .into_iter()
            .map(|path| ModFileRecord {
                id: None,
                mod_id,
                relative_path: path,
                hash: None,
                size: None,
            })
            .collect();

        self.db.insert_mod_files(mod_id, &file_records)?;
        let plugin_files = plugin_filenames_from_mod_files(&file_records);
        self.db
            .replace_mod_plugins(mod_id, game_id, &plugin_files)?;

        let installed = InstalledMod {
            id: mod_id,
            name,
            version,
            author: None,
            enabled: true,
            priority: record.priority,
            nexus_mod_id: resolved_nexus_mod_id,
            nexus_file_id,
            file_count: file_records.len() as i32,
            install_path: staging,
            category_id: None,
        };

        Ok(InstallResult::Completed(installed))
    }

    /// Enable a mod
    pub async fn enable_mod(&self, game_id: &str, name: &str) -> Result<()> {
        let m = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;

        if m.enabled {
            return Ok(());
        }

        self.db.set_mod_enabled(m.id.unwrap(), true)?;
        Ok(())
    }

    /// Disable a mod
    pub async fn disable_mod(&self, game_id: &str, name: &str) -> Result<()> {
        let m = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;

        if !m.enabled {
            return Ok(());
        }

        self.db.set_mod_enabled(m.id.unwrap(), false)?;
        Ok(())
    }

    /// Complete a FOMOD installation after wizard selections
    pub async fn complete_fomod_install(
        &self,
        context: &FomodInstallContext,
        wizard: &fomod::WizardState,
        _progress_callback: Option<ProgressCallback>,
    ) -> Result<InstalledMod> {
        use fomod::{executor::FomodExecutor, planner::InstallPlan};

        tracing::info!("Compiling FOMOD install plan for {}", context.mod_name);

        // Create target directory (same as staging for now)
        let target_path = context.staging_path.clone();

        // Compile installation plan
        let plan = InstallPlan::from_wizard_state(
            wizard,
            &context.installer,
            context.mod_name.clone(),
            &context.staging_path,
            &target_path,
        )?;

        tracing::info!(
            "FOMOD plan compiled: {} files, {} conflicts",
            plan.estimated_file_count,
            plan.conflicts.len()
        );

        // Execute installation
        let executor = FomodExecutor::new(
            context.staging_path.clone(),
            target_path.clone(),
        );

        // TODO: Convert progress_callback to ExecutionProgress format
        let _result = executor.execute(&plan, None).await?;

        tracing::info!("FOMOD installation completed successfully");

        // Collect installed files
        let files = collect_files(&target_path)?;

        let mod_id = if let Some(existing_id) = context.existing_mod_id {
            // Reconfiguration: Update existing mod
            tracing::info!("Reconfiguring existing mod ID {}", existing_id);

            // Delete old file records
            self.db.delete_mod_files(existing_id)?;

            // Update mod record
            let now = chrono::Utc::now().to_rfc3339();
            // Get existing mod to preserve some fields
            if let Some(existing_mod) = self.db.get_mod_by_id(existing_id)? {
                let updated_record = ModRecord {
                    id: Some(existing_id),
                    game_id: existing_mod.game_id,
                    name: existing_mod.name.clone(),
                    version: context.version.clone(),
                    author: existing_mod.author,
                    description: existing_mod.description,
                    nexus_mod_id: existing_mod.nexus_mod_id,
                    nexus_file_id: existing_mod.nexus_file_id,
                    install_path: target_path.to_string_lossy().to_string(),
                    enabled: existing_mod.enabled,
                    priority: existing_mod.priority,
                    file_count: files.len() as i32,
                    installed_at: existing_mod.installed_at,
                    updated_at: now,
                    category_id: existing_mod.category_id,
                };
                self.db.update_mod(&updated_record)?;
            }

            existing_id
        } else {
            // New installation: Create new mod record
            let now = chrono::Utc::now().to_rfc3339();
            let record = ModRecord {
                id: None,
                game_id: context.game_id.clone(),
                name: context.mod_name.clone(),
                version: context.version.clone(),
                author: None,
                description: None,
                nexus_mod_id: context.nexus_mod_id,
                nexus_file_id: context.nexus_file_id,
                install_path: target_path.to_string_lossy().to_string(),
                enabled: true,
                priority: context.priority,
                file_count: files.len() as i32,
                installed_at: now.clone(),
                updated_at: now,
                category_id: None,
            };

            self.db.insert_mod(&record)?
        };

        // Insert new file records
        let file_records: Vec<ModFileRecord> = files
            .into_iter()
            .map(|path| ModFileRecord {
                id: None,
                mod_id,
                relative_path: path,
                hash: None,
                size: None,
            })
            .collect();

        self.db.insert_mod_files(mod_id, &file_records)?;
        let plugin_files = plugin_filenames_from_mod_files(&file_records);
        self.db
            .replace_mod_plugins(mod_id, &context.game_id, &plugin_files)?;

        // Save FOMOD choices for re-run
        let profile_id = None; // TODO: Get current profile ID
        let manager = fomod::persistence::FomodChoiceManager::new(&self.db);
        manager.save_choice(mod_id, profile_id, &plan)?;

        let installed = InstalledMod {
            id: mod_id,
            name: context.mod_name.clone(),
            version: context.version.clone(),
            author: None,
            enabled: true,
            priority: context.priority,
            nexus_mod_id: None,
            nexus_file_id: None,
            file_count: file_records.len() as i32,
            install_path: target_path,
            category_id: None,
        };

        Ok(installed)
    }

    /// Remove a mod
    pub async fn remove_mod(&self, game_id: &str, name: &str) -> Result<()> {
        let m = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;

        // Delete staging directory
        let staging = self.staging_dir(game_id).await.join(name);
        if staging.exists() {
            tokio::fs::remove_dir_all(&staging)
                .await
                .context("Failed to remove mod directory")?;
        }

        // Delete from database
        self.db.delete_mod(m.id.unwrap())?;

        Ok(())
    }

    /// Check for missing requirements of a mod
    /// Returns list of missing required plugins and their required-by plugin
    pub async fn check_requirements(
        &self,
        game_id: &str,
        mod_name: &str,
    ) -> Result<Vec<(String, String)>> {
        use crate::plugins::masterlist::{build_metadata_map, get_requirements, load_masterlist};
        use std::path::Path;

        // Load masterlist
        let metadata_map = if let Ok(masterlist) = load_masterlist(Path::new("masterlist.yaml")) {
            build_metadata_map(&masterlist)
        } else if let Ok(masterlist) = load_masterlist(Path::new("loot-master/masterlist.yaml")) {
            build_metadata_map(&masterlist)
        } else {
            // No masterlist available
            return Ok(Vec::new());
        };

        // Get mod's install path
        let _mod_record = self.db.get_mod(game_id, mod_name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", mod_name))?;

        let staging = self.staging_dir(game_id).await.join(mod_name);

        // Find all plugin files in this mod
        let mut mod_plugins = Vec::new();
        for entry in WalkDir::new(&staging).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                    if matches!(ext_str.as_str(), "esp" | "esm" | "esl") {
                        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                            mod_plugins.push(filename.to_string());
                        }
                    }
                }
            }
        }

        // Get all currently installed mods' plugins
        let mut installed_plugins = std::collections::HashSet::new();
        let all_mods = self.db.get_mods_for_game(game_id)?;
        for m in all_mods {
            let staging_path = self.staging_dir(game_id).await.join(&m.name);
            for entry in WalkDir::new(&staging_path).max_depth(3) {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_str().unwrap_or("").to_lowercase();
                        if matches!(ext_str.as_str(), "esp" | "esm" | "esl") {
                            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                                installed_plugins.insert(filename.to_lowercase());
                            }
                        }
                    }
                }
            }
        }

        // Check requirements for each plugin in the mod
        let mut missing = Vec::new();
        for plugin in &mod_plugins {
            let requirements = get_requirements(plugin, &metadata_map);
            for req in requirements {
                if !installed_plugins.contains(&req) {
                    missing.push((req, plugin.clone()));
                }
            }
        }

        Ok(missing)
    }

    /// Change mod priority (increase or decrease)
    pub async fn change_priority(&self, game_id: &str, name: &str, delta: i32) -> Result<i32> {
        let m = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;

        let new_priority = (m.priority + delta).max(0);
        self.db.set_mod_priority(m.id.unwrap(), new_priority)?;
        Ok(new_priority)
    }

    /// Set mod priority to a specific value
    pub async fn set_priority(&self, game_id: &str, name: &str, priority: i32) -> Result<()> {
        let m = self
            .db
            .get_mod(game_id, name)?
            .ok_or_else(|| anyhow::anyhow!("Mod '{}' not found", name))?;

        self.db.set_mod_priority(m.id.unwrap(), priority)?;
        Ok(())
    }

    /// Batch-save a complete priority ordering from the Load Order screen.
    /// Takes a slice of (mod_id, new_priority) pairs.
    pub async fn save_priority_order(&self, order: &[(i64, i32)]) -> Result<()> {
        for &(mod_id, priority) in order {
            self.db.set_mod_priority(mod_id, priority)?;
        }
        Ok(())
    }

    /// Auto-sort mods by category order
    /// Categories are ordered by display_order, and mods within a category maintain relative order
    pub async fn auto_sort_by_category(&self, game_id: &str) -> Result<()> {
        let mods = self.db.get_mods_for_game(game_id)?;
        let categories = self.db.get_all_categories()?;

        // Create category order map
        let category_order: std::collections::HashMap<i64, i32> = categories
            .iter()
            .filter_map(|c| c.id.map(|id| (id, c.display_order)))
            .collect();

        // Sort mods by category display order, then by current priority within category
        let mut sorted_mods = mods.clone();
        sorted_mods.sort_by_key(|m| {
            let cat_order = m.category_id
                .and_then(|id| category_order.get(&id).copied())
                .unwrap_or(999); // Uncategorized mods go last
            (cat_order, m.priority)
        });

        // Reassign priorities in order
        for (new_priority, mod_rec) in sorted_mods.iter().enumerate() {
            if let Some(id) = mod_rec.id {
                self.db.set_mod_priority(id, new_priority as i32)?;
            }
        }

        Ok(())
    }

    /// Get the next priority value for a new mod
    async fn next_priority(&self, game_id: &str) -> Result<i32> {
        let mods = self.db.get_mods_for_game(game_id)?;
        Ok(mods.iter().map(|m| m.priority).max().unwrap_or(-1) + 1)
    }

    /// Rescan the mods directory and rebuild the database from existing mod folders
    /// This is useful for recovering from database loss while preserving mod files
    pub async fn rescan_mods(
        &self,
        game_id: &str,
        progress_callback: Option<Box<dyn Fn(usize, usize, String) + Send + Sync>>,
    ) -> Result<RescanStats> {
        let mods_dir = self.staging_dir(game_id).await;

        if !mods_dir.exists() {
            bail!("Mods directory not found: {}", mods_dir.display());
        }

        tracing::info!("Scanning mods directory: {}", mods_dir.display());

        // First pass: count directories (fast)
        let total = std::fs::read_dir(&mods_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count();

        tracing::info!("Found {} mod directories to scan", total);

        let mut stats = RescanStats::default();
        let mut current = 0;

        // Second pass: process one directory at a time
        for entry in std::fs::read_dir(&mods_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            // Only process directories
            let is_dir = match entry.file_type() {
                Ok(ft) => ft.is_dir(),
                Err(e) => {
                    tracing::warn!("Failed to get file type: {}", e);
                    continue;
                }
            };

            if !is_dir {
                continue;
            }

            current += 1;
            let mod_path = entry.path();
            let mod_name = mod_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Report progress
            if let Some(ref callback) = progress_callback {
                callback(current, total, mod_name.clone());
            }

            tracing::info!("Processing {}/{}: {}", current, total, mod_name);

            let scanned = scan_mod_metadata(&mod_path);
            let files = match collect_files(&mod_path) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("Failed to catalog files for '{}': {}", mod_name, e);
                    stats.failed += 1;
                    continue;
                }
            };
            let file_records: Vec<ModFileRecord> = files
                .iter()
                .cloned()
                .map(|path| ModFileRecord {
                    id: None,
                    mod_id: 0,
                    relative_path: path,
                    hash: None,
                    size: None,
                })
                .collect();
            let plugin_files = plugin_filenames_from_mod_files(&file_records);

            let existing = self.db.find_mod_by_name(game_id, &scanned.name)?;

            match existing {
                None => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let record = ModRecord {
                        id: None,
                        game_id: game_id.to_string(),
                        name: scanned.name.clone(),
                        version: scanned.version.clone(),
                        author: None,
                        description: scanned.description.clone(),
                        nexus_mod_id: scanned.nexus_mod_id,
                        nexus_file_id: scanned.nexus_file_id,
                        install_path: mod_path.to_string_lossy().to_string(),
                        enabled: false,
                        priority: stats.added as i32,
                        file_count: files.len() as i32,
                        installed_at: now.clone(),
                        updated_at: now,
                        category_id: None,
                    };

                    match self.db.insert_mod(&record) {
                        Ok(mod_id) => {
                            let mut inserted_files = file_records.clone();
                            for rec in &mut inserted_files {
                                rec.mod_id = mod_id;
                            }
                            if let Err(e) = self.db.insert_mod_files(mod_id, &inserted_files) {
                                tracing::warn!("Failed to save file index for '{}': {}", scanned.name, e);
                            }
                            if let Err(e) = self.db.replace_mod_plugins(mod_id, game_id, &plugin_files) {
                                tracing::warn!("Failed to index plugins for '{}': {}", scanned.name, e);
                            }
                            stats.added += 1;
                            tracing::info!("Imported mod '{}' v{}", scanned.name, scanned.version);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to insert mod '{}': {}", scanned.name, e);
                            stats.failed += 1;
                        }
                    }
                }
                Some(mut existing_mod) => {
                    let mod_id = existing_mod.id.unwrap_or(0);
                    if mod_id == 0 {
                        stats.failed += 1;
                        continue;
                    }

                    let mut existing_files = self
                        .db
                        .get_mod_files(mod_id)?
                        .into_iter()
                        .map(|f| f.relative_path)
                        .collect::<Vec<_>>();
                    existing_files.sort();

                    let mut scanned_files = files.clone();
                    scanned_files.sort();

                    let resolved_nexus_mod_id = scanned.nexus_mod_id.or(existing_mod.nexus_mod_id);
                    let resolved_nexus_file_id = scanned.nexus_file_id.or(existing_mod.nexus_file_id);
                    let resolved_description = scanned
                        .description
                        .clone()
                        .or(existing_mod.description.clone());

                    let changed =
                        existing_mod.version != scanned.version
                            || existing_mod.install_path != mod_path.to_string_lossy()
                            || existing_mod.nexus_mod_id != resolved_nexus_mod_id
                            || existing_mod.nexus_file_id != resolved_nexus_file_id
                            || existing_mod.description != resolved_description
                            || existing_files != scanned_files;

                    if changed {
                        existing_mod.version = scanned.version.clone();
                        existing_mod.install_path = mod_path.to_string_lossy().to_string();
                        existing_mod.nexus_mod_id = resolved_nexus_mod_id;
                        existing_mod.nexus_file_id = resolved_nexus_file_id;
                        existing_mod.description = resolved_description;
                        existing_mod.file_count = files.len() as i32;
                        existing_mod.updated_at = chrono::Utc::now().to_rfc3339();

                        if let Err(e) = self.db.update_mod(&existing_mod) {
                            tracing::warn!("Failed to update mod '{}': {}", existing_mod.name, e);
                            stats.failed += 1;
                            continue;
                        }

                        if let Err(e) = self.db.delete_mod_files(mod_id) {
                            tracing::warn!("Failed clearing old files for '{}': {}", existing_mod.name, e);
                        }
                        let mut updated_files = file_records.clone();
                        for rec in &mut updated_files {
                            rec.mod_id = mod_id;
                        }
                        if let Err(e) = self.db.insert_mod_files(mod_id, &updated_files) {
                            tracing::warn!("Failed indexing files for '{}': {}", existing_mod.name, e);
                        }
                        if let Err(e) = self.db.replace_mod_plugins(mod_id, game_id, &plugin_files) {
                            tracing::warn!("Failed indexing plugins for '{}': {}", existing_mod.name, e);
                        }
                        stats.updated += 1;
                    } else {
                        // Keep plugin index in sync even when core mod record is unchanged.
                        if let Err(e) = self.db.replace_mod_plugins(mod_id, game_id, &plugin_files) {
                            tracing::warn!("Failed indexing plugins for '{}': {}", existing_mod.name, e);
                        }
                        stats.unchanged += 1;
                    }
                }
            }
        }

        tracing::info!(
            "Rescan complete: {} added, {} updated, {} unchanged, {} failed",
            stats.added, stats.updated, stats.unchanged, stats.failed
        );
        Ok(stats)
    }

    /// Parse mod name and version from archive filename
    fn parse_mod_name(filename: &str) -> (String, String) {
        // Common patterns:
        // "ModName-1.2.3"
        // "ModName v1.2.3"
        // "ModName_1.2.3"
        // "ModName 1.2.3"

        let version_patterns = [
            r"-(\d+(?:\.\d+)*)",
            r"[_\s]v?(\d+(?:\.\d+)*)",
            r"[_\s](\d+(?:\.\d+)+)",
        ];

        for pattern in version_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(caps) = re.captures(filename) {
                    let version = caps.get(1).map(|m| m.as_str()).unwrap_or("1.0.0");
                    let name = filename[..caps.get(0).unwrap().start()].to_string();
                    let name = name
                        .replace('-', " ")
                        .replace('_', " ")
                        .trim()
                        .to_string();
                    return (name, version.to_string());
                }
            }
        }

        // No version found
        (filename.replace('-', " ").replace('_', " "), "1.0.0".to_string())
    }

    /// Extract Nexus mod ID from filename
    /// Pattern: "ModName-MODID-version-FILEID.ext"
    /// Example: "Gore-85298-1-7-5-1739059080.zip" -> mod_id: 85298, file_id: 1739059080
    fn parse_nexus_ids(filename: &str) -> Option<(i64, i64)> {
        let parts: Vec<&str> = filename.split('-').collect();

        if parts.len() < 2 {
            return None;
        }

        // Collect all numeric parts
        let mut numbers: Vec<i64> = Vec::new();
        for part in &parts {
            if let Ok(num) = part.parse::<i64>() {
                numbers.push(num);
            }
        }

        if numbers.is_empty() {
            return None;
        }

        // Check if last number is a timestamp (10 digits)
        let last_num = *numbers.last()?;
        let last_digits = last_num.to_string().len();

        if last_digits == 10 {
            // Has timestamp - find first substantial number (3+ digits) as mod_id
            for num in &numbers[..numbers.len() - 1] {
                let digits = num.to_string().len();
                if digits >= 3 && digits <= 7 {
                    tracing::debug!("Parsed '{}' -> mod_id: {}, file_id: {}", filename, num, last_num);
                    return Some((*num, last_num));
                }
            }
        } else if last_digits >= 3 && last_digits <= 7 {
            // No timestamp - last number is probably the mod_id
            tracing::debug!("Parsed '{}' (no timestamp) -> mod_id: {}", filename, last_num);
            return Some((last_num, 0));
        }

        tracing::trace!("Rejecting '{}': no valid pattern in {:?}", filename, numbers);
        None
    }

    /// Normalize a name for matching (lowercase + collapse spaces)
    fn normalize_name(name: &str) -> String {
        name.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Update Nexus IDs for mods that don't have them by matching with archives directory
    pub async fn update_missing_nexus_ids(&self, game_id: &str, archive_dir: Option<&str>) -> Result<usize> {
        let mods = self.list_mods(game_id).await?;
        let mut updated = 0;

        // Use provided archive directory or fall back to cache
        let scan_dir = if let Some(dir) = archive_dir {
            std::path::PathBuf::from(dir)
        } else {
            directories::ProjectDirs::from("", "", "modsanity")
                .map(|dirs| dirs.cache_dir().join("downloads"))
                .unwrap_or_else(|| std::path::PathBuf::from("~/.cache/modsanity/downloads"))
        };

        tracing::info!("Scanning archive directory: {}", scan_dir.display());

        // Build a map of normalized archive name -> (mod_id, file_id, original_name)
        let mut archive_map = std::collections::HashMap::new();
        let mut total_files = 0;
        if scan_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&scan_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    total_files += 1;
                    let path = entry.path();
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Some((mod_id, file_id)) = Self::parse_nexus_ids(stem) {
                            // Extract the clean name (before the first numeric ID)
                            let clean_name = Self::parse_mod_name(stem).0;
                            let normalized = Self::normalize_name(&clean_name);
                            tracing::debug!("Archive '{}' -> '{}' (IDs: {}, {})",
                                stem, normalized, mod_id, file_id);
                            archive_map.insert(normalized, (mod_id, file_id, clean_name));
                        }
                    }
                }
            }
        }

        tracing::info!("Scanned {} files, found {} archives with Nexus IDs", total_files, archive_map.len());

        let total_mods = mods.len();
        tracing::debug!("Attempting to match {} installed mods to archives", total_mods);

        // Match installed mods to archives
        let mut matched = 0;
        let mut skipped_with_valid_id = 0;
        for mod_info in &mods {
            // Skip if already has a valid Nexus ID (but overwrite if it looks like a timestamp)
            if let Some(existing_id) = mod_info.nexus_mod_id {
                let digits = existing_id.to_string().len();
                // If it's 10 digits, it's probably a timestamp - overwrite it
                if digits != 10 {
                    tracing::debug!("Mod '{}' already has valid ID: {}", mod_info.name, existing_id);
                    skipped_with_valid_id += 1;
                    continue;
                }
                tracing::debug!("Mod '{}' has timestamp ID ({}), will overwrite", mod_info.name, existing_id);
            }

            // Try to find matching archive
            let normalized_name = Self::normalize_name(&mod_info.name);

            if let Some(&(mod_id, file_id, ref archive_name)) = archive_map.get(&normalized_name) {
                tracing::debug!(
                    "Matched '{}' to archive '{}': mod_id={}, file_id={}",
                    mod_info.name,
                    archive_name,
                    mod_id,
                    file_id
                );

                // Update in database
                if let Some(mut record) = self.db.get_mod(game_id, &mod_info.name)? {
                    record.nexus_mod_id = Some(mod_id);
                    record.nexus_file_id = Some(file_id);
                    self.db.update_mod(&record)?;
                    matched += 1;
                    updated += 1;
                }
            }
        }

        if matched > 0 {
            tracing::info!("Successfully matched {} mod(s) to archives", matched);
        }
        if skipped_with_valid_id > 0 {
            tracing::debug!("Skipped {} mods that already have valid IDs", skipped_with_valid_id);
        }
        let without_matches = total_mods - matched - skipped_with_valid_id;
        if without_matches > 0 {
            tracing::debug!("{} mods without matching archives", without_matches);
        }
        Ok(updated)
    }

    /// Check for updates to installed mods using Nexus Mods API
    /// Returns a list of mods that have updates available
    pub async fn check_for_updates(
        &self,
        game_id: &str,
        nexus_client: &crate::nexus::NexusClient,
    ) -> Result<Vec<crate::nexus::graphql::ModUpdateInfo>> {
        // Get all installed mods for this game that have Nexus mod IDs
        let mods = self.db.get_mods_for_game(game_id)?;

        let mod_ids: Vec<i64> = mods
            .iter()
            .filter_map(|m| m.nexus_mod_id)
            .collect();

        if mod_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Get game domain name (e.g., "skyrimspecialedition")
        let game_domain = match game_id {
            "skyrimse" => "skyrimspecialedition",
            "skyrimvr" => "skyrimspecialedition", // VR uses same domain
            id => id, // Use game_id as fallback
        };

        // Query for updates
        let updates = nexus_client
            .check_mod_updates(game_domain, &mod_ids)
            .await
            .context("Failed to check for mod updates")?;

        // Filter to only mods that actually have updates
        let updates_available: Vec<_> = updates
            .into_iter()
            .filter(|u| u.has_update)
            .collect();

        Ok(updates_available)
    }

    /// Check Nexus mod requirements using GraphQL API
    /// Returns (missing_requirements, dlc_requirements, already_installed_count)
    pub async fn check_nexus_requirements(
        &self,
        game_id: &str,
        mod_id: i64,
        nexus_client: &crate::nexus::NexusClient,
    ) -> Result<(Vec<crate::nexus::graphql::ModRequirement>, Vec<crate::nexus::graphql::ModRequirement>, usize)> {
        // Get game domain
        let game_domain = match game_id {
            "skyrimse" => "skyrimspecialedition",
            "skyrimvr" => "skyrimspecialedition",
            id => id,
        };

        // Get requirements from Nexus
        let requirements = nexus_client
            .get_mod_requirements(game_domain, mod_id)
            .await
            .context("Failed to fetch mod requirements from Nexus")?;

        // Get currently installed mods
        let installed_mods = self.db.get_mods_for_game(game_id)?;
        let installed_mod_ids: std::collections::HashSet<i64> = installed_mods
            .iter()
            .filter_map(|m| m.nexus_mod_id)
            .collect();

        // Separate requirements into missing mods, DLCs, and already installed
        let mut missing = Vec::new();
        let mut dlcs = Vec::new();
        let mut already_installed = 0;

        for req in requirements {
            if req.is_dlc {
                dlcs.push(req);
            } else if installed_mod_ids.contains(&req.mod_id) {
                already_installed += 1;
            } else {
                missing.push(req);
            }
        }

        Ok((missing, dlcs, already_installed))
    }
}

/// Find the actual data root (handles nested folders like "ModName/Data/")
///
/// # FOMOD Support Status
///
/// **Current implementation is simplified:**
/// - Auto-selects folders starting with "00" (typically "Required" content)
/// - Does NOT present FOMOD wizard UI for user selection
/// - Does NOT handle complex installer conditions or dependencies
/// - Works for simple FOMODs with obvious default choices
///
/// **Known limitations:**
/// - Multi-option installers may install incorrect variant
/// - Conditional logic in FOMOD XML is ignored
/// - User has no control over which components are installed
///
/// TODO: Full FOMOD wizard implementation using the fomod module
fn find_data_root(path: &Path) -> Result<PathBuf> {
    use crate::mods::fomod;

    // Check for common data indicators
    // Includes BodySlide/CBBE specific directories
    let data_indicators = [
        "meshes",
        "textures",
        "scripts",
        "interface",
        "sound",
        "skse",
        "calientetools", // BodySlide files
        "shapedata",     // BodySlide presets
        "tools",         // Various mod tools
        "strings",       // Translation files
        "seq",           // Animation sequences
        "music",         // Music files
        "video",         // Video files
        "shadersfx",     // Shader effects
    ];

    // If this is a FOMOD with numbered folders, look for "00" Required folder
    if fomod::has_numbered_folders(path) {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Look for "00" folders (required components in FOMOD)
            if name_str.starts_with("00") {
                return Ok(entry.path());
            }
        }
        // If no "00" folder, this needs manual component selection
        // For now, just use the root and let user deal with it
        tracing::warn!("FOMOD detected but no '00 Required' folder found");
    }

    // Check if current directory has data indicators
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if data_indicators.contains(&name.as_str())
            || name.ends_with(".esp")
            || name.ends_with(".esm")
            || name.ends_with(".esl")
        {
            return Ok(path.to_path_buf());
        }
    }

    // Check one level deep (skip numbered FOMOD folders)
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let subdir = entry.path();
            let dir_name = entry.file_name().to_string_lossy().to_string();

            // Skip numbered folders (FOMOD components)
            if dir_name.len() >= 2
                && dir_name.chars().nth(0).map(|c| c.is_ascii_digit()).unwrap_or(false)
                && dir_name.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
            {
                continue;
            }
            for subentry in std::fs::read_dir(&subdir)? {
                let subentry = subentry?;
                let name = subentry.file_name().to_string_lossy().to_lowercase();
                if data_indicators.contains(&name.as_str())
                    || name.ends_with(".esp")
                    || name.ends_with(".esm")
                    || name.ends_with(".esl")
                {
                    return Ok(subdir);
                }
            }
        }
    }

    // No specific data root found, use the extraction path
    Ok(path.to_path_buf())
}

/// Collect all files in a directory (relative paths)
fn collect_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Ok(relative) = entry.path().strip_prefix(root) {
                files.push(relative.to_string_lossy().to_string());
            }
        }
    }

    Ok(files)
}

/// Extract plugin filenames (.esp/.esm/.esl) from mod file records.
fn plugin_filenames_from_mod_files(files: &[ModFileRecord]) -> Vec<String> {
    let mut plugins = std::collections::BTreeSet::new();
    for file in files {
        if let Some(name) = Path::new(&file.relative_path).file_name().and_then(|n| n.to_str()) {
            let lower = name.to_lowercase();
            if lower.ends_with(".esp") || lower.ends_with(".esm") || lower.ends_with(".esl") {
                plugins.insert(name.to_string());
            }
        }
    }
    plugins.into_iter().collect()
}

#[derive(Debug, Clone)]
struct ScannedModMetadata {
    name: String,
    version: String,
    nexus_mod_id: Option<i64>,
    nexus_file_id: Option<i64>,
    description: Option<String>,
}

fn scan_mod_metadata(mod_path: &Path) -> ScannedModMetadata {
    let dir_name = mod_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let (mut name, mut version) = ModManager::parse_mod_name(dir_name);
    let mut nexus_mod_id = ModManager::parse_nexus_ids(dir_name).map(|(mid, _)| mid);
    let mut nexus_file_id = ModManager::parse_nexus_ids(dir_name).map(|(_, fid)| fid);
    let mut description = None;

    let meta_ini = mod_path.join("meta.ini");
    if meta_ini.exists() {
        if let Ok(meta) = parse_meta_ini(&meta_ini) {
            if let Some(meta_name) = meta.name {
                name = meta_name;
            }
            if let Some(meta_version) = meta.version {
                version = meta_version;
            }
            nexus_mod_id = meta.nexus_mod_id.or(nexus_mod_id);
            nexus_file_id = meta.nexus_file_id.or(nexus_file_id);
            description = meta.description;
        }
    }

    if description.is_none() {
        description = extract_short_description(mod_path);
    }

    ScannedModMetadata {
        name,
        version,
        nexus_mod_id,
        nexus_file_id,
        description,
    }
}

#[derive(Debug, Default)]
struct ParsedMetaIni {
    name: Option<String>,
    version: Option<String>,
    nexus_mod_id: Option<i64>,
    nexus_file_id: Option<i64>,
    description: Option<String>,
}

fn parse_meta_ini(path: &Path) -> Result<ParsedMetaIni> {
    let content = std::fs::read_to_string(path)?;
    let mut parsed = ParsedMetaIni::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match key.as_str() {
            "name" => parsed.name = Some(value.to_string()),
            "version" => parsed.version = Some(value.to_string()),
            "modid" | "nexus_mod_id" => {
                if let Ok(id) = value.parse::<i64>() {
                    parsed.nexus_mod_id = Some(id);
                }
            }
            "fileid" | "nexus_file_id" => {
                if let Ok(id) = value.parse::<i64>() {
                    parsed.nexus_file_id = Some(id);
                }
            }
            "description" | "notes" | "comments" => {
                parsed.description = Some(value.to_string());
            }
            _ => {}
        }
    }

    Ok(parsed)
}

fn extract_short_description(mod_path: &Path) -> Option<String> {
    let entries = std::fs::read_dir(mod_path).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().ok()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !(name.starts_with("readme") || name.starts_with("description") || name.ends_with(".txt")) {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).ok()?;
        let first_line = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))?;
        let short = first_line.chars().take(240).collect::<String>();
        if !short.is_empty() {
            return Some(short);
        }
    }
    None
}

/// Move contents from one directory to another
async fn move_contents(from: &Path, to: &Path) -> Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());

        if entry.path() == to.to_path_buf() {
            continue;
        }

        if dest.exists() {
            if dest.is_dir() {
                tokio::fs::remove_dir_all(&dest).await?;
            } else {
                tokio::fs::remove_file(&dest).await?;
            }
        }

        tokio::fs::rename(entry.path(), dest).await?;
    }
    Ok(())
}
