//! Library deduplication check
//!
//! Checks which mods from a modlist are already installed,
//! splitting entries into already_installed and needs_download.

use crate::db::{Database, ModRecord};
use crate::import::modlist_format::ModlistEntry;
use anyhow::Result;
use std::sync::Arc;

/// Result of checking a modlist against the installed library
#[derive(Debug)]
pub struct LibraryCheckResult {
    /// Entries that are already installed (with matching mod record)
    pub already_installed: Vec<(ModlistEntry, ModRecord)>,
    /// Entries that need to be downloaded
    pub needs_download: Vec<ModlistEntry>,
}

/// Check which modlist entries are already installed
pub fn check_library(
    db: &Arc<Database>,
    game_id: &str,
    entries: Vec<ModlistEntry>,
) -> Result<LibraryCheckResult> {
    // Collect all nexus_mod_ids for batch lookup
    let nexus_ids: Vec<i64> = entries
        .iter()
        .filter_map(|e| e.nexus_mod_id)
        .collect();

    // Batch lookup by nexus_mod_id
    let installed_by_nexus_id = db.find_mods_by_nexus_ids(game_id, &nexus_ids)?;

    let mut already_installed = Vec::new();
    let mut needs_download = Vec::new();

    for entry in entries {
        // Primary match: by nexus_mod_id
        if let Some(nexus_id) = entry.nexus_mod_id {
            if let Some(record) = installed_by_nexus_id.get(&nexus_id) {
                already_installed.push((entry, record.clone()));
                continue;
            }
        }

        // Fallback: case-insensitive exact name match
        if let Ok(Some(record)) = db.find_mod_by_name(game_id, &entry.name) {
            already_installed.push((entry, record));
            continue;
        }

        needs_download.push(entry);
    }

    Ok(LibraryCheckResult {
        already_installed,
        needs_download,
    })
}
