//! Symlink-based mod deployment

use crate::config::{Config, DeploymentMethod};
use crate::db::Database;
use crate::games::Game;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;

/// Deployment statistics
#[derive(Debug, Default)]
pub struct DeploymentStats {
    pub mods_deployed: usize,
    pub files_deployed: usize,
    pub conflicts_resolved: usize,
    pub errors: Vec<String>,
}

/// Deploy mods to the game directory
///
/// # Priority System
///
/// Mods are deployed in priority order (ascending). When multiple mods contain
/// the same file path:
/// - The mod with the **highest priority number wins** (overwrites earlier files)
/// - Lower priority mods deploy first, higher priority mods overwrite them
/// - This implements "last write wins" conflict resolution
///
/// Example: If both ModA (priority 5) and ModB (priority 10) have `textures/sky.dds`,
/// ModB's version will be deployed because 10 > 5.
pub async fn deploy_mods(
    config: &Arc<RwLock<Config>>,
    db: &Arc<Database>,
    game: &Game,
) -> Result<DeploymentStats> {
    let config = config.read().await;
    let mut stats = DeploymentStats::default();

    // Get all enabled mods sorted by priority
    let mods = db.get_mods_for_game(&game.id)?;
    let enabled_mods: Vec<_> = mods.into_iter().filter(|m| m.enabled).collect();

    if enabled_mods.is_empty() {
        tracing::info!("No enabled mods - purging deployment to restore factory state");
        // Purge all deployed files to restore game to clean state
        let staging_dir = config.paths.game_mods_dir(&game.id);
        purge_deployment(game, &config.deployment.method, &staging_dir).await?;
        tracing::info!("Game restored to factory state (all mod files removed)");
        return Ok(stats);
    }

    // Build file map: destination -> (source, mod_name, priority)
    // Higher priority mods overwrite lower priority
    let mut file_map: HashMap<PathBuf, (PathBuf, String, i32)> = HashMap::new();

    for mod_record in &enabled_mods {
        let mod_path = PathBuf::from(&mod_record.install_path);
        if !mod_path.exists() {
            stats.errors.push(format!(
                "Mod directory not found: {}",
                mod_record.name
            ));
            continue;
        }

        for entry in WalkDir::new(&mod_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let relative = entry
                .path()
                .strip_prefix(&mod_path)
                .expect("Path should be relative to mod path");

            let dest = game.data_path.join(relative);
            let source = entry.path().to_path_buf();

            // Check if we already have this file from a lower priority mod
            if let Some((_, existing_mod, existing_priority)) = file_map.get(&dest) {
                if mod_record.priority > *existing_priority {
                    stats.conflicts_resolved += 1;
                    tracing::debug!(
                        "Conflict: {} overwrites {} for {}",
                        mod_record.name,
                        existing_mod,
                        relative.display()
                    );
                } else {
                    // Keep existing (higher or equal priority)
                    continue;
                }
            }

            file_map.insert(
                dest,
                (source, mod_record.name.clone(), mod_record.priority),
            );
        }

        stats.mods_deployed += 1;
    }

    // Clear existing deployment
    let staging_dir = config.paths.game_mods_dir(&game.id);
    purge_deployment(game, &config.deployment.method, &staging_dir).await?;

    // Create all symlinks/hardlinks/copies
    for (dest, (source, mod_name, _)) in &file_map {
        if let Err(e) = deploy_file(&config.deployment.method, source, dest).await {
            stats.errors.push(format!(
                "Failed to deploy {} from {}: {}",
                dest.display(),
                mod_name,
                e
            ));
        } else {
            stats.files_deployed += 1;
        }
    }

    tracing::info!(
        "Deployed {} files from {} mods ({} conflicts resolved)",
        stats.files_deployed,
        stats.mods_deployed,
        stats.conflicts_resolved
    );

    Ok(stats)
}

/// Deploy a single file
async fn deploy_file(method: &DeploymentMethod, source: &Path, dest: &Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create parent directory")?;
    }

    // Remove existing file/link if present (including broken symlinks)
    // Use symlink_metadata to detect symlinks without following them
    if let Ok(_metadata) = tokio::fs::symlink_metadata(dest).await {
        // Remove file or symlink (symlink_metadata doesn't follow symlinks)
        tokio::fs::remove_file(dest).await.ok();
    }

    match method {
        DeploymentMethod::Symlink => {
            symlink(source, dest).context("Failed to create symlink")?;
        }
        DeploymentMethod::Hardlink => {
            std::fs::hard_link(source, dest).context("Failed to create hardlink")?;
        }
        DeploymentMethod::Copy => {
            tokio::fs::copy(source, dest)
                .await
                .context("Failed to copy file")?;
        }
    }

    Ok(())
}

/// Remove all deployed mod files (symlinks only)
///
/// Safety: Only removes symlinks that point to paths under `staging_dir` to avoid
/// accidentally deleting unrelated symlinks.
pub async fn purge_deployment(
    game: &Game,
    method: &DeploymentMethod,
    staging_dir: &Path,
) -> Result<()> {
    if *method != DeploymentMethod::Symlink {
        tracing::warn!(
            "Purge only works reliably with symlink deployment. \
             Manual cleanup may be needed for hardlinks/copies."
        );
    }

    let data_path = &game.data_path;
    if !data_path.exists() {
        return Ok(());
    }

    // Canonicalize staging directory for accurate comparison
    let canonical_staging = staging_dir.canonicalize().unwrap_or_else(|_| staging_dir.to_path_buf());

    let mut removed = 0;

    for entry in WalkDir::new(data_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only remove symlinks using symlink_metadata to avoid following the link
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                // Check if it points to our staging directory
                if let Ok(target) = std::fs::read_link(path) {
                    // Resolve relative symlinks
                    let target_absolute = if target.is_absolute() {
                        target
                    } else {
                        path.parent().unwrap_or(path).join(&target)
                    };

                    // Canonicalize and check if under our staging directory
                    if let Ok(canonical_target) = target_absolute.canonicalize() {
                        if canonical_target.starts_with(&canonical_staging) {
                            tokio::fs::remove_file(path).await.ok();
                            removed += 1;
                        }
                    }
                }
            }
        }
    }

    // Clean up empty directories
    clean_empty_dirs(data_path).await?;

    tracing::info!("Purged {} symlinks from game directory", removed);
    Ok(())
}

/// Remove empty directories recursively
async fn clean_empty_dirs(path: &Path) -> Result<()> {
    for entry in WalkDir::new(path)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            // Try to remove - will fail if not empty
            tokio::fs::remove_dir(entry.path()).await.ok();
        }
    }
    Ok(())
}

// Add deploy method to ModManager
impl super::ModManager {
    /// Deploy all enabled mods to the game directory
    pub async fn deploy(&self, game: &Game) -> Result<DeploymentStats> {
        deploy_mods(&self.config, &self.db, game).await
    }

    /// Remove all deployed mods
    pub async fn purge(&self, game: &Game) -> Result<()> {
        let config = self.config.read().await;
        let staging_dir = config.paths.game_mods_dir(&game.id);
        purge_deployment(game, &config.deployment.method, &staging_dir).await
    }
}
