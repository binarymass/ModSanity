//! Symlink-based mod deployment

use crate::config::{Config, DeploymentMethod};
use crate::db::Database;
use crate::games::Game;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
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
        let staging_dir = config.game_staging_dir(&game.id);
        purge_deployment(game, &config.deployment.method, &staging_dir).await?;
        purge_skse_root_files(game).await?;
        tracing::info!("Game restored to factory state (all mod files removed)");
        return Ok(stats);
    }

    // Build file map: normalized relative path -> (source, mod_name, priority, canonical_relative_path)
    // Higher priority mods overwrite lower priority.
    let mut file_map: HashMap<PathBuf, (PathBuf, String, i32, PathBuf)> = HashMap::new();
    let mut dir_case_map: HashMap<PathBuf, PathBuf> = HashMap::new();

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

            let source = entry.path().to_path_buf();
            let normalized_relative = normalize_relative_path(relative);
            let canonical_relative = canonicalize_relative_path(relative, &mut dir_case_map);

            // Check if we already have this file from a lower priority mod (case-insensitive path)
            if let Some((existing_source, existing_mod, existing_priority, _)) =
                file_map.get_mut(&normalized_relative)
            {
                if mod_record.priority > *existing_priority {
                    stats.conflicts_resolved += 1;
                    tracing::debug!(
                        "Conflict: {} overwrites {} for {}",
                        mod_record.name,
                        existing_mod,
                        relative.display()
                    );
                    *existing_source = source;
                    *existing_mod = mod_record.name.clone();
                    *existing_priority = mod_record.priority;
                } else {
                    // Keep existing (higher or equal priority)
                    continue;
                }
            } else {
                file_map.insert(
                    normalized_relative,
                    (
                        source,
                        mod_record.name.clone(),
                        mod_record.priority,
                        canonical_relative,
                    ),
                );
            }
        }

        stats.mods_deployed += 1;
    }

    // Clear existing deployment
    let staging_dir = config.game_staging_dir(&game.id);
    purge_deployment(game, &config.deployment.method, &staging_dir).await?;
    purge_skse_root_files(game).await?;

    // Create all symlinks/hardlinks/copies
    for (_, (source, mod_name, _, canonical_relative)) in &file_map {
        let (dest, force_copy) = resolve_deploy_destination(game, canonical_relative);
        if let Err(e) = deploy_file(&config.deployment.method, source, &dest, force_copy).await {
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

/// Resolve destination path for a deployed file and whether deployment must be a hard copy.
///
/// Rules:
/// - Paths rooted at `Data/` are normalized into the game's `Data` folder.
/// - SKSE runtime binaries (`skse*.exe` / `skse*.dll`) at mod root deploy next to the game EXE.
/// - Any SKSE-related path (filename starts with `skse` or path contains `SKSE`) is always copied.
fn resolve_deploy_destination(game: &Game, relative: &Path) -> (PathBuf, bool) {
    let relative = strip_leading_data_component(relative);
    let filename = relative
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_root_level = relative.components().count() == 1;
    let is_skse_runtime_binary = is_root_level
        && filename.starts_with("skse")
        && (filename.ends_with(".exe") || filename.ends_with(".dll"));

    let mut force_copy = filename.starts_with("skse");
    if !force_copy {
        force_copy = relative.components().any(|c| {
            matches!(c, Component::Normal(part) if part.to_string_lossy().eq_ignore_ascii_case("skse"))
        });
    }

    let dest = if is_skse_runtime_binary {
        game.install_path.join(relative)
    } else {
        game.data_path.join(relative)
    };

    (dest, force_copy)
}

/// Strip a leading `Data` component from a relative path (case-insensitive).
fn strip_leading_data_component(relative: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    let mut iter = relative.components();
    let mut skipped = false;

    while let Some(component) = iter.next() {
        if !skipped {
            if let Component::Normal(part) = component {
                if part.to_string_lossy().eq_ignore_ascii_case("data") {
                    skipped = true;
                    continue;
                }
            }
            skipped = true;
        }
        out.push(component.as_os_str());
    }

    if out.as_os_str().is_empty() {
        relative.to_path_buf()
    } else {
        out
    }
}

/// Normalize a relative path for case-insensitive matching.
fn normalize_relative_path(relative: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        if let Component::Normal(part) = component {
            normalized.push(part.to_string_lossy().to_lowercase());
        }
    }
    normalized
}

/// Canonicalize path casing by reusing the first seen casing for each path segment,
/// including the filename.
fn canonicalize_relative_path(
    relative: &Path,
    dir_case_map: &mut HashMap<PathBuf, PathBuf>,
) -> PathBuf {
    let mut normalized_path = PathBuf::new();
    let mut canonical_path = PathBuf::new();

    for component in relative.components() {
        if let Component::Normal(part) = component {
            normalized_path.push(part.to_string_lossy().to_lowercase());
            let stored = dir_case_map
                .entry(normalized_path.clone())
                .or_insert_with(|| canonical_path.join(part))
                .clone();
            canonical_path = stored;
        }
    }

    canonical_path
}

/// Deploy a single file
async fn deploy_file(
    method: &DeploymentMethod,
    source: &Path,
    dest: &Path,
    force_copy: bool,
) -> Result<()> {
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

    if force_copy {
        tokio::fs::copy(source, dest)
            .await
            .context("Failed to copy file")?;
    } else {
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
    }

    Ok(())
}

/// Remove SKSE runtime binaries from the game root so redeploys don't leave stale copies.
async fn purge_skse_root_files(game: &Game) -> Result<()> {
    if !game.install_path.exists() {
        return Ok(());
    }

    let mut removed = 0usize;
    for entry in WalkDir::new(&game.install_path)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("skse") && (lower.ends_with(".exe") || lower.ends_with(".dll")) {
            tokio::fs::remove_file(path).await.ok();
            removed += 1;
        }
    }

    if removed > 0 {
        tracing::info!("Removed {} SKSE runtime binaries from game root", removed);
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
        let staging_dir = config.game_staging_dir(&game.id);
        purge_deployment(game, &config.deployment.method, &staging_dir).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_relative_path_is_case_insensitive() {
        assert_eq!(
            normalize_relative_path(Path::new("Bodyslides/Foo.nif")),
            normalize_relative_path(Path::new("bodyslides/foo.NIF"))
        );
    }

    #[test]
    fn canonicalize_relative_path_reuses_first_seen_directory_casing() {
        let mut dir_case_map = HashMap::new();

        let first =
            canonicalize_relative_path(Path::new("Bodyslides/ShapeA.nif"), &mut dir_case_map);
        let second =
            canonicalize_relative_path(Path::new("bodyslides/ShapeB.nif"), &mut dir_case_map);

        assert_eq!(first, PathBuf::from("Bodyslides/ShapeA.nif"));
        assert_eq!(second, PathBuf::from("Bodyslides/ShapeB.nif"));
    }

    #[test]
    fn canonicalize_relative_path_reuses_first_seen_filename_casing() {
        let mut dir_case_map = HashMap::new();

        let first =
            canonicalize_relative_path(Path::new("Meshes/Bodyslides/Body_0.NIF"), &mut dir_case_map);
        let second =
            canonicalize_relative_path(Path::new("meshes/bodyslides/body_0.nif"), &mut dir_case_map);

        assert_eq!(first, PathBuf::from("Meshes/Bodyslides/Body_0.NIF"));
        assert_eq!(second, PathBuf::from("Meshes/Bodyslides/Body_0.NIF"));
    }
}
