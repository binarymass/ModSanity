//! CLI command action handlers

use super::App;
use crate::config::DeploymentMethod;
use crate::games::GameDetector;
use anyhow::{bail, Context, Result};

impl App {
    // ========== Game Commands ==========

    pub async fn cmd_game_list(&self) -> Result<()> {
        if self.games.is_empty() {
            println!("No games detected. Run 'modsanity game scan' to scan for games.");
            return Ok(());
        }

        let active = self.config.read().await.active_game.clone();

        println!("Detected Games:");
        println!("{:-<60}", "");
        for game in &self.games {
            let marker = if Some(&game.id) == active.as_ref() {
                " [active]"
            } else {
                ""
            };
            println!(
                "  {} ({}){}\n    Path: {}",
                game.name,
                game.id,
                marker,
                game.install_path.display()
            );
        }
        Ok(())
    }

    pub async fn cmd_game_scan(&mut self) -> Result<()> {
        println!("Scanning for games...");
        self.games = GameDetector::detect_all().await;

        if self.games.is_empty() {
            println!("No games found.");
        } else {
            println!("Found {} game(s):", self.games.len());
            for game in &self.games {
                println!("  - {} at {}", game.name, game.install_path.display());
            }
        }
        Ok(())
    }

    pub async fn cmd_game_select(&mut self, name: &str) -> Result<()> {
        let game = self
            .games
            .iter()
            .find(|g| g.id == name || g.name.to_lowercase().contains(&name.to_lowercase()))
            .cloned();

        match game {
            Some(g) => {
                println!("Selected: {} ({})", g.name, g.id);
                self.set_active_game(Some(g)).await?;
            }
            None => bail!("Game '{}' not found. Run 'modsanity game list' to see available games.", name),
        }
        Ok(())
    }

    pub async fn cmd_game_info(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Game Information");
        println!("{:-<40}", "");
        println!("Name:         {}", game.name);
        println!("ID:           {}", game.id);
        println!("Install Path: {}", game.install_path.display());
        println!("Data Path:    {}", game.data_path.display());
        if let Some(prefix) = &game.proton_prefix {
            println!("Proton Prefix: {}", prefix.display());
        }
        if let Some(appdata) = &game.appdata_path {
            println!("AppData:      {}", appdata.display());
        }
        Ok(())
    }

    // ========== Mod Commands ==========

    pub async fn cmd_mod_list(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let mods = self.mods.list_mods(&game.id).await?;

        if mods.is_empty() {
            println!("No mods installed for {}.", game.name);
            return Ok(());
        }

        println!("Installed Mods for {}:", game.name);
        println!("{:-<60}", "");
        for (i, m) in mods.iter().enumerate() {
            let status = if m.enabled { "[✓]" } else { "[ ]" };
            println!("{:>3}. {} {} (v{})", i + 1, status, m.name, m.version);
        }
        Ok(())
    }

    pub async fn cmd_mod_install(&self, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Installing mod from: {}", path);
        match self.mods.install_from_archive(&game.id, path, None, None, None).await? {
            crate::mods::InstallResult::Completed(installed) => {
                println!("Installed: {} (v{})", installed.name, installed.version);
                println!("Run 'modsanity deploy' to apply changes.");
                Ok(())
            }
            crate::mods::InstallResult::RequiresWizard(context) => {
                println!("ERROR: {} requires FOMOD wizard interaction", context.mod_name);
                println!("FOMOD wizards are only supported in TUI mode (run without arguments)");
                bail!("Interactive wizard required")
            }
        }
    }

    pub async fn cmd_mod_enable(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.enable_mod(&game.id, name).await?;
        println!("Enabled: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_mod_disable(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.disable_mod(&game.id, name).await?;
        println!("Disabled: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_mod_remove(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.remove_mod(&game.id, name).await?;
        println!("Removed: {}", name);
        Ok(())
    }

    pub async fn cmd_mod_info(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let m = self.mods.get_mod(&game.id, name).await?;

        println!("Mod Information");
        println!("{:-<40}", "");
        println!("Name:     {}", m.name);
        println!("Version:  {}", m.version);
        println!("Enabled:  {}", if m.enabled { "Yes" } else { "No" });
        println!("Priority: {}", m.priority);
        if let Some(author) = &m.author {
            println!("Author:   {}", author);
        }
        if let Some(nexus_id) = m.nexus_mod_id {
            println!("Nexus ID: {}", nexus_id);
        }
        println!("Files:    {}", m.file_count);
        Ok(())
    }

    // ========== Profile Commands ==========

    pub async fn cmd_profile_list(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let profiles = self.profiles.list_profiles(&game.id).await?;
        let active = self.config.read().await.active_profile.clone();

        if profiles.is_empty() {
            println!("No profiles for {}.", game.name);
            return Ok(());
        }

        println!("Profiles for {}:", game.name);
        println!("{:-<40}", "");
        for p in profiles {
            let marker = if Some(&p.name) == active.as_ref() {
                " [active]"
            } else {
                ""
            };
            println!("  {}{}", p.name, marker);
        }
        Ok(())
    }

    pub async fn cmd_profile_create(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.create_profile(&game.id, name).await?;
        println!("Created profile: {}", name);
        Ok(())
    }

    pub async fn cmd_profile_switch(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.switch_profile(&game.id, name).await?;
        println!("Switched to profile: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_profile_delete(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.delete_profile(&game.id, name).await?;
        println!("Deleted profile: {}", name);
        Ok(())
    }

    pub async fn cmd_profile_export(&self, name: &str, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.export_profile(&game.id, name, path).await?;
        println!("Exported '{}' to: {}", name, path);
        Ok(())
    }

    pub async fn cmd_profile_import(&self, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let profile = self.profiles.import_profile(&game.id, path).await?;
        println!("Imported profile: {}", profile.name);
        Ok(())
    }

    // ========== Other Commands ==========

    pub async fn cmd_deploy(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        println!("Deploying mods to {}...", game.name);
        let stats = self.mods.deploy(&game).await?;
        println!(
            "Deployed {} files from {} mods.",
            stats.files_deployed, stats.mods_deployed
        );
        Ok(())
    }

    pub async fn cmd_set_deployment_method(&self, method: &str) -> Result<()> {
        let parsed = DeploymentMethod::from_cli(method)?;
        self.set_deployment_method(parsed).await?;
        println!(
            "Deployment method set to: {} ({})",
            parsed.display_name(),
            parsed.as_str()
        );
        Ok(())
    }

    pub async fn cmd_deployment_show(&self) -> Result<()> {
        let config = self.config.read().await;
        println!("Deployment Settings");
        println!("{:-<40}", "");
        println!("Method:           {}", config.deployment.method.display_name());
        println!("Method (raw):     {}", config.deployment.method.as_str());
        println!(
            "Backup originals: {}",
            if config.deployment.backup_originals { "Yes" } else { "No" }
        );
        println!(
            "Purge on exit:    {}",
            if config.deployment.purge_on_exit { "Yes" } else { "No" }
        );
        Ok(())
    }

    pub async fn cmd_status(&self) -> Result<()> {
        println!("ModSanity Status");
        println!("{:-<40}", "");

        let config = self.config.read().await;

        // Game status
        match self.active_game().await {
            Some(g) => println!("Active Game: {} ({})", g.name, g.id),
            None => println!("Active Game: None"),
        };

        // Profile status
        match &config.active_profile {
            Some(p) => println!("Profile:     {}", p),
            None => println!("Profile:     Default"),
        };
        println!(
            "Deploy:      {}",
            config.deployment.method.display_name()
        );

        // Mod counts
        if let Some(game) = self.active_game().await {
            let mods = self.mods.list_mods(&game.id).await?;
            let enabled = mods.iter().filter(|m| m.enabled).count();
            println!("Mods:        {} installed, {} enabled", mods.len(), enabled);
        }

        Ok(())
    }

    // ========== Modlist Commands ==========

    pub async fn cmd_modlist_save(&self, path: &str, format: &str) -> Result<()> {
        use crate::import::modlist_format::{ModSanityModlist, ModlistMeta, ModlistEntry, PluginOrderEntry};
        use crate::plugins;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let out_path = std::path::Path::new(path);

        match format {
            "native" | "json" => {
                let mods = self.mods.list_mods(&game.id).await?;

                // Get category names for installed mods
                let categories = self.db.get_all_categories()?;
                let cat_map: std::collections::HashMap<i64, String> = categories
                    .into_iter()
                    .filter_map(|c| c.id.map(|id| (id, c.name)))
                    .collect();

                let mod_entries: Vec<ModlistEntry> = mods.iter().map(|m| {
                    ModlistEntry {
                        name: m.name.clone(),
                        version: m.version.clone(),
                        nexus_mod_id: m.nexus_mod_id,
                        nexus_file_id: m.nexus_file_id,
                        author: m.author.clone(),
                        priority: m.priority,
                        enabled: m.enabled,
                        category: m.category_id.and_then(|id| cat_map.get(&id).cloned()),
                    }
                }).collect();

                // Get plugins from game data directory
                let plugin_entries: Vec<PluginOrderEntry> = match plugins::get_plugins(&game) {
                    Ok(plist) => plist.iter().map(|p| {
                        PluginOrderEntry {
                            filename: p.filename.clone(),
                            load_order: p.load_order as i32,
                            enabled: p.enabled,
                        }
                    }).collect(),
                    Err(_) => Vec::new(),
                };

                let profile_name = self.config.read().await.active_profile.clone();

                let modlist = ModSanityModlist {
                    meta: ModlistMeta {
                        format_version: 1,
                        modsanity_version: crate::APP_VERSION.to_string(),
                        game_id: game.id.clone(),
                        game_domain: game.nexus_game_domain(),
                        exported_at: chrono::Utc::now().to_rfc3339(),
                        profile_name,
                    },
                    mods: mod_entries,
                    plugins: plugin_entries,
                };

                crate::import::modlist_format::save_native(out_path, &modlist)?;
                println!("Saved native modlist to: {}", path);
                println!("  {} mods, {} plugins", modlist.mods.len(), modlist.plugins.len());
            }
            "mo2" => {
                // Write MO2 modlist.txt format (plugin list)
                let plugin_list = match plugins::get_plugins(&game) {
                    Ok(plist) => plist,
                    Err(e) => bail!("Failed to read plugins: {}", e),
                };

                let mut lines = Vec::new();
                for plugin in &plugin_list {
                    let prefix = if plugin.enabled { "*" } else { "" };
                    lines.push(format!("{}{}", prefix, plugin.filename));
                }

                std::fs::write(out_path, lines.join("\n"))
                    .context("Failed to write MO2 modlist file")?;

                println!("Saved MO2 modlist to: {}", path);
                println!("  {} plugins", plugin_list.len());
            }
            _ => bail!("Unknown format '{}'. Use 'native' or 'mo2'.", format),
        }

        Ok(())
    }

    pub async fn cmd_modlist_load(&self, path: &str, auto_approve: bool) -> Result<()> {
        use crate::import::{detect_format, ModlistFormat};

        let file_path = std::path::Path::new(path);
        let format = detect_format(file_path)?;

        match format {
            ModlistFormat::Native => {
                self.cmd_modlist_load_native(path, auto_approve).await
            }
            ModlistFormat::Mo2 => {
                println!("Detected MO2 format, delegating to import command...");
                self.cmd_import_modlist(path, auto_approve).await
            }
        }
    }

    async fn cmd_modlist_load_native(&self, path: &str, auto_approve: bool) -> Result<()> {
        use crate::import::modlist_format;
        use crate::import::library_check;
        use crate::queue::QueueManager;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let modlist = modlist_format::load_native(std::path::Path::new(path))?;

        // Validate game matches
        if modlist.meta.game_id != game.id {
            bail!(
                "Modlist is for game '{}' but active game is '{}'. Select the correct game first.",
                modlist.meta.game_id, game.id
            );
        }

        println!("Loading native modlist from: {}", path);
        println!("Game: {} ({})", game.name, game.id);
        println!("Modlist version: {}, exported: {}", modlist.meta.modsanity_version, modlist.meta.exported_at);
        println!("  {} mods, {} plugins", modlist.mods.len(), modlist.plugins.len());

        // Library check: skip already-installed mods
        let check_result = library_check::check_library(&self.db, &game.id, modlist.mods)?;

        println!("\nLibrary Check:");
        println!("  Already installed: {}", check_result.already_installed.len());
        println!("  Needs download:    {}", check_result.needs_download.len());

        if check_result.needs_download.is_empty() {
            println!("\nAll mods are already installed!");
            return Ok(());
        }

        // Queue missing mods for download (nexus IDs already resolved, no matching needed)
        let queue_manager = QueueManager::new(self.db.clone());
        let batch_id = queue_manager.create_batch();

        println!("\nCreating download queue (batch: {})...", batch_id);

        let mut queue_position = 0;
        let mut skipped_no_id = 0;

        for entry in &check_result.needs_download {
            let nexus_mod_id = match entry.nexus_mod_id {
                Some(id) if id > 0 => id,
                _ => {
                    skipped_no_id += 1;
                    continue;
                }
            };

            let queue_entry = crate::queue::QueueEntry {
                id: 0,
                batch_id: batch_id.clone(),
                game_id: game.id.clone(),
                queue_position,
                plugin_name: entry.name.clone(),
                mod_name: entry.name.clone(),
                nexus_mod_id,
                selected_file_id: entry.nexus_file_id,
                auto_install: true,
                match_confidence: Some(1.0),
                alternatives: Vec::new(),
                status: crate::queue::QueueStatus::Matched,
                progress: 0.0,
                error: None,
            };

            queue_manager.add_entry(queue_entry)?;
            queue_position += 1;
        }

        println!("Added {} entries to download queue", queue_position);
        if skipped_no_id > 0 {
            println!("Skipped {} entries without Nexus mod IDs", skipped_no_id);
        }

        if auto_approve {
            println!("\nAuto-approve enabled. Processing queue...");
            self.cmd_queue_process(Some(&batch_id), false).await?;
        } else {
            println!("\nQueue created. Use 'modsanity queue process --batch-id {}' to start downloads", batch_id);
        }

        Ok(())
    }

    // ========== Import Commands ==========

    pub async fn cmd_import_modlist(&self, path: &str, auto_approve: bool) -> Result<()> {
        use crate::import::ModlistImporter;
        use crate::queue::QueueManager;
        use std::path::Path;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let nexus = match &self.nexus {
            Some(client) => client.clone(),
            None => bail!("NexusMods API key not configured. Set NEXUS_API_KEY environment variable or add to config."),
        };

        println!("Importing modlist from: {}", path);
        println!("Game: {} ({})", game.name, game.id);

        let importer = ModlistImporter::with_catalog(&game.id, (*nexus).clone(), Some(self.db.clone()));
        let started = std::time::Instant::now();
        let result = importer
            .import_modlist_with_progress(
                Path::new(path),
                Some(|current: usize, total: usize, plugin: &str| {
                    if current == 1 || current % 25 == 0 || current == total {
                        println!(
                            "Matching {:>4}/{}: {}",
                            current,
                            total.max(current),
                            plugin
                        );
                    }
                }),
            )
            .await?;
        println!(
            "Matching completed in {:.1}s",
            started.elapsed().as_secs_f32()
        );

        println!("\nImport Results:");
        println!("{:-<60}", "");
        println!("Total plugins: {}", result.total_plugins);
        println!("Auto-matched:  {}", result.auto_matched().count());
        println!("Needs review:  {}", result.needs_review().count());
        println!("No matches:    {}", result.no_matches().count());

        // Library check: skip mods that are already installed
        let matched_nexus_ids: Vec<i64> = result.matches.iter()
            .filter_map(|m| m.best_match.as_ref().map(|bm| bm.mod_id))
            .filter(|id| *id > 0)
            .collect();

        let installed_mods = self.db.find_mods_by_nexus_ids(&game.id, &matched_nexus_ids)?;
        let installed_count = installed_mods.len();

        if installed_count > 0 {
            println!("Already installed: {} (will be skipped)", installed_count);
        }

        // Create download queue batch
        let queue_manager = QueueManager::new(self.db.clone());
        let batch_id = queue_manager.create_batch();

        println!("\nCreating download queue (batch: {})...", batch_id);

        let mut queue_position = 0;
        let mut skipped = 0;
        for match_result in &result.matches {
            // Skip already-installed mods
            if let Some(best_match) = &match_result.best_match {
                if installed_mods.contains_key(&best_match.mod_id) {
                    skipped += 1;
                    continue;
                }
            }

            let alternatives = match_result.alternatives.iter().map(|alt| {
                crate::queue::QueueAlternative {
                    mod_id: alt.mod_id,
                    name: alt.name.clone(),
                    summary: alt.summary.clone(),
                    downloads: alt.downloads,
                    score: alt.score,
                    thumbnail_url: None,
                }
            }).collect();

            let (mod_name, nexus_mod_id, status) = if let Some(best_match) = &match_result.best_match {
                (
                    best_match.name.clone(),
                    best_match.mod_id,
                    if match_result.confidence.is_high() {
                        crate::queue::QueueStatus::Matched
                    } else if match_result.confidence.needs_review() {
                        crate::queue::QueueStatus::NeedsReview
                    } else {
                        crate::queue::QueueStatus::NeedsManual
                    }
                )
            } else {
                (
                    match_result.mod_name.clone(),
                    0,
                    crate::queue::QueueStatus::NeedsManual
                )
            };

            let entry = crate::queue::QueueEntry {
                id: 0,
                batch_id: batch_id.clone(),
                game_id: game.id.clone(),
                queue_position,
                plugin_name: match_result.plugin.plugin_name.clone(),
                mod_name,
                nexus_mod_id,
                selected_file_id: None,
                auto_install: true,
                match_confidence: Some(match_result.confidence.score()),
                alternatives,
                status,
                progress: 0.0,
                error: None,
            };

            queue_manager.add_entry(entry)?;
            queue_position += 1;
        }

        println!("Added {} entries to download queue", queue_position);
        if skipped > 0 {
            println!("Skipped {} already-installed mods", skipped);
        }

        if auto_approve {
            println!("\nAuto-approve enabled. Processing queue...");
            self.cmd_queue_process(Some(&batch_id), false).await?;
        } else {
            println!("\nQueue created. Use 'modsanity queue process --batch-id {}' to start downloads", batch_id);
            println!("Or use 'modsanity import status {}' to review matches", batch_id);
        }

        Ok(())
    }

    pub async fn cmd_import_status(&self, batch_id: Option<&str>) -> Result<()> {
        use crate::queue::QueueManager;

        let batch = match batch_id {
            Some(id) => id.to_string(),
            None => bail!("Batch ID required. Use 'modsanity import status <batch-id>'"),
        };

        let queue_manager = QueueManager::new(self.db.clone());
        let entries = queue_manager.get_batch(&batch)?;

        if entries.is_empty() {
            println!("No entries found for batch: {}", batch);
            return Ok(());
        }

        println!("Import Batch: {}", batch);
        println!("{:-<80}", "");

        for entry in entries {
            let status_icon = match entry.status {
                crate::queue::QueueStatus::Completed => "✓",
                crate::queue::QueueStatus::Failed => "✗",
                crate::queue::QueueStatus::NeedsReview => "⚠",
                crate::queue::QueueStatus::NeedsManual => "!",
                crate::queue::QueueStatus::Downloading => "↓",
                _ => "○",
            };

            println!("{} {} -> {}", status_icon, entry.plugin_name, entry.mod_name);

            if entry.match_confidence.is_some() {
                println!("   Confidence: {:.1}%", entry.match_confidence.unwrap() * 100.0);
            }

            if !entry.alternatives.is_empty() {
                println!("   {} alternative(s) available", entry.alternatives.len());
            }

            if let Some(err) = &entry.error {
                println!("   Error: {}", err);
            }
        }

        Ok(())
    }

    // ========== Queue Commands ==========

    pub async fn cmd_queue_list(&self) -> Result<()> {
        use crate::queue::QueueManager;

        let queue_manager = QueueManager::new(self.db.clone());
        let active_game = self.active_game().await;
        let game_id = active_game.as_ref().map(|g| g.id.as_str());
        let batches = queue_manager.list_batches(game_id)?;

        if batches.is_empty() {
            if let Some(game) = active_game {
                println!("No queue batches found for {}.", game.name);
            } else {
                println!("No queue batches found.");
            }
            return Ok(());
        }

        println!("Queue Batches:");
        println!("{:-<100}", "");
        for batch in batches {
            println!(
                "Batch: {}\n  Game: {}\n  Total: {} | Pending: {} | Matched: {} | Review: {} | Manual: {}\n  Active: {} downloading, {} installing | Done: {} completed, {} failed\n  Created: {}",
                batch.batch_id,
                batch.game_id,
                batch.total,
                batch.pending,
                batch.matched,
                batch.needs_review,
                batch.needs_manual,
                batch.downloading,
                batch.installing,
                batch.completed,
                batch.failed,
                batch.created_at,
            );
            println!("{:-<100}", "");
        }

        Ok(())
    }

    pub async fn cmd_queue_process(&self, batch_id: Option<&str>, download_only: bool) -> Result<()> {
        use crate::queue::QueueProcessor;

        let batch = match batch_id {
            Some(id) => id.to_string(),
            None => bail!("Batch ID required. Use 'modsanity queue process --batch-id <batch-id>'"),
        };

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let nexus = match &self.nexus {
            Some(client) => client.clone(),
            None => bail!("NexusMods API key not configured."),
        };

        let config = self.config.read().await;
        let download_dir = config.paths.downloads_dir();

        let game_domain = game.nexus_game_domain();
        let processor = QueueProcessor::new(
            self.db.clone(),
            (*nexus).clone(),
            game_domain,
            game.id.clone(),
            download_dir,
            self.mods.clone(),
        );

        println!("Processing batch: {}", batch);
        if download_only {
            println!("Download-only mode enabled");
        }

        processor.process_batch(&batch, download_only).await?;

        println!("Batch processing complete!");
        Ok(())
    }

    pub async fn cmd_queue_retry(&self) -> Result<()> {
        use crate::queue::QueueManager;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let queue_manager = QueueManager::new(self.db.clone());
        let failed_batches = queue_manager.failed_batches(Some(&game.id))?;

        if failed_batches.is_empty() {
            println!("No failed downloads found for {}.", game.name);
            return Ok(());
        }

        let mut total_retried = 0usize;
        for batch_id in failed_batches {
            let retried = queue_manager.retry_failed_in_batch(&batch_id)?;
            if retried == 0 {
                continue;
            }

            total_retried += retried;
            println!("Retrying {} failed entries in batch {}", retried, batch_id);
            self.cmd_queue_process(Some(&batch_id), false).await?;
        }

        if total_retried == 0 {
            println!("No failed entries were eligible for retry.");
        } else {
            println!("Retried {} failed download(s).", total_retried);
        }

        Ok(())
    }

    pub async fn cmd_queue_clear(&self, batch_id: Option<&str>) -> Result<()> {
        use crate::queue::QueueManager;

        let queue_manager = QueueManager::new(self.db.clone());

        if let Some(batch) = batch_id {
            println!("Clearing batch: {}", batch);
            queue_manager.clear_batch(batch)?;
            println!("Batch cleared");
        } else {
            bail!("Batch ID required. Use 'modsanity queue clear <batch-id>'");
        }

        Ok(())
    }

    // ========== Nexus Catalog Commands ==========

    pub async fn cmd_nexus_populate(
        &self,
        game_domain: &str,
        reset: bool,
        per_page: i32,
        max_pages: Option<i32>,
    ) -> Result<()> {
        use crate::nexus::{NexusRestClient, CatalogPopulator, PopulateOptions};

        // Get API key
        let api_key = match &self.config.read().await.nexus_api_key {
            Some(key) => key.clone(),
            None => bail!("NexusMods API key not configured. Set NEXUS_API_KEY environment variable or add to config."),
        };

        // Create REST client
        let rest_client = NexusRestClient::new(&api_key)
            .context("Failed to create REST API client")?;

        // Create populator
        let populator = CatalogPopulator::new(
            self.db.clone(),
            rest_client,
            game_domain.to_string(),
        )?;

        // Set up options
        let options = PopulateOptions {
            reset,
            per_page,
            max_pages,
            delay_between_pages_ms: 500,
        };

        println!("Nexus Mods Catalog Population");
        println!("{:-<60}", "");
        println!("Game domain:  {}", game_domain);
        println!("Mods per page: {}", per_page);
        if let Some(max) = max_pages {
            println!("Max pages:    {}", max);
        } else {
            println!("Max pages:    unlimited");
        }
        if reset {
            println!("Mode:         RESET (starting from beginning)");
        } else {
            println!("Mode:         RESUME (continuing from checkpoint)");
        }
        println!("{:-<60}", "");
        println!();

        // Run population (no callback for CLI - direct output only)
        let stats = populator.populate(options, None::<fn(i32, i64, i64, i64, i32)>).await?;

        // Display results
        println!();
        println!("Population Complete!");
        println!("{:-<60}", "");
        println!("Pages fetched:   {}", stats.pages_fetched);
        println!("Mods inserted:   {}", stats.mods_inserted);
        println!("Mods updated:    {}", stats.mods_updated);
        println!("Total mods:      {}", stats.total_mods);
        println!("{:-<60}", "");

        Ok(())
    }

    pub async fn cmd_nexus_status(&self, game_domain: &str) -> Result<()> {
        // Validate game domain
        if !game_domain.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
            bail!("Invalid game domain: must contain only lowercase letters, numbers, hyphens, and underscores");
        }

        println!("Nexus Catalog Status");
        println!("{:-<60}", "");
        println!("Game domain: {}", game_domain);
        println!();

        // Get sync state
        let state = self.db.get_sync_state(game_domain)?;

        if state.completed {
            println!("Status:      ✓ Completed");
        } else {
            println!("Status:      In progress / Incomplete");
        }

        println!("Current page: {}", state.current_page);

        if let Some(last_sync) = &state.last_sync {
            println!("Last sync:    {}", last_sync);
        } else {
            println!("Last sync:    Never");
        }

        if let Some(error) = &state.last_error {
            println!("Last error:   {}", error);
        }

        // Get mod count
        let count = self.db.count_catalog_mods(game_domain)?;
        println!();
        println!("Total mods:   {}", count);
        println!("{:-<60}", "");

        if !state.completed {
            println!();
            println!("To resume: modsanity nexus populate --game {}", game_domain);
            println!("To restart: modsanity nexus populate --game {} --reset", game_domain);
        }

        Ok(())
    }
}
