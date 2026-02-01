//! Mod conflict detection and resolution

use crate::db::{Database, FileConflict};
use crate::mods::fomod::planner::{ConflictItem, ConflictSeverity, InstallPlan};
use anyhow::Result;
use std::collections::HashMap;

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Higher priority mod wins (default)
    Priority,
    /// Newer mod wins
    Newer,
    /// Keep both (not applicable for identical paths)
    Manual,
}

/// Conflict summary for a mod pair
#[derive(Debug, Clone)]
pub struct ModConflict {
    pub mod1: String,
    pub mod2: String,
    pub files: Vec<String>,
    pub winner: String,
}

/// Get all conflicts for a game, grouped by mod pair
pub fn get_conflicts_grouped(db: &Database, game_id: &str) -> Result<Vec<ModConflict>> {
    let raw_conflicts = db.find_conflicts(game_id)?;

    // Group by mod pair
    let mut grouped: HashMap<(String, String), Vec<FileConflict>> = HashMap::new();

    for conflict in raw_conflicts {
        let key = if conflict.mod1 < conflict.mod2 {
            (conflict.mod1.clone(), conflict.mod2.clone())
        } else {
            (conflict.mod2.clone(), conflict.mod1.clone())
        };
        grouped.entry(key).or_default().push(conflict);
    }

    // Convert to ModConflict
    let mut result = Vec::new();
    for ((mod1, mod2), conflicts) in grouped {
        let winner = conflicts.first().map(|c| c.winner().to_string()).unwrap_or_default();
        let files = conflicts.into_iter().map(|c| c.path).collect();

        result.push(ModConflict {
            mod1,
            mod2,
            files,
            winner,
        });
    }

    // Sort by number of conflicts (most first)
    result.sort_by(|a, b| b.files.len().cmp(&a.files.len()));

    Ok(result)
}

/// Check for potential issues in mod setup
pub fn check_mod_issues(db: &Database, game_id: &str) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    // Check for conflicts
    let conflicts = get_conflicts_grouped(db, game_id)?;
    if !conflicts.is_empty() {
        let total_files: usize = conflicts.iter().map(|c| c.files.len()).sum();
        issues.push(format!(
            "{} file conflicts between {} mod pairs",
            total_files,
            conflicts.len()
        ));
    }

    // Check for missing masters could go here (plugin analysis)

    Ok(issues)
}

/// Format conflict information for display
pub fn format_conflict(conflict: &ModConflict) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Conflict: {} vs {} ({} files)",
        conflict.mod1,
        conflict.mod2,
        conflict.files.len()
    ));
    lines.push(format!("Winner: {}", conflict.winner));

    // Show first few files
    for file in conflict.files.iter().take(5) {
        lines.push(format!("  - {}", file));
    }
    if conflict.files.len() > 5 {
        lines.push(format!("  ... and {} more", conflict.files.len() - 5));
    }

    lines.join("\n")
}

/// Check for conflicts with a FOMOD install plan
///
/// Analyzes the install plan against existing installed files to detect conflicts.
pub fn check_fomod_conflicts(
    plan: &InstallPlan,
    game_id: &str,
    db: &Database,
) -> Result<Vec<ConflictItem>> {
    let mut conflicts = Vec::new();

    // Get all installed files for this game
    let installed_files = db.get_all_files(game_id)?;
    let mut file_map: HashMap<String, String> = HashMap::new();

    for file_record in installed_files {
        file_map.insert(file_record.path, file_record.mod_name);
    }

    // Check each file operation against existing files
    for operation in &plan.file_operations {
        let dest_path = operation.destination.to_string_lossy().to_string();

        if let Some(existing_mod) = file_map.get(&dest_path) {
            // This file already exists from another mod
            if existing_mod != &plan.mod_name {
                let severity = if dest_path.ends_with(".esp")
                    || dest_path.ends_with(".esm")
                    || dest_path.ends_with(".esl")
                {
                    ConflictSeverity::High // Plugin conflicts are high severity
                } else if dest_path.ends_with(".ini") || dest_path.ends_with(".xml") {
                    ConflictSeverity::Medium // Config file conflicts are medium
                } else {
                    ConflictSeverity::Low // Asset conflicts are low
                };

                conflicts.push(ConflictItem {
                    path: operation.destination.clone(),
                    existing_mod: Some(existing_mod.clone()),
                    severity,
                    description: format!(
                        "File already installed by '{}'",
                        existing_mod
                    ),
                });
            }
        }
    }

    Ok(conflicts)
}

