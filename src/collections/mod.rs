//! Nexus Mods collection support

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Nexus Mods collection
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Collection {
    pub info: CollectionInfo,
    pub mods: Vec<CollectionMod>,
}

/// Collection metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectionInfo {
    pub author: String,
    #[serde(rename = "authorUrl")]
    pub author_url: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "installInstructions")]
    pub install_instructions: String,
    #[serde(rename = "domainName")]
    pub domain_name: String,
    #[serde(rename = "gameVersions")]
    pub game_versions: Vec<String>,
}

/// Mod entry in a collection
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollectionMod {
    pub name: String,
    pub version: String,
    pub optional: bool,
    #[serde(rename = "domainName")]
    pub domain_name: String,
    pub source: ModSource,
    pub author: String,
    pub details: ModDetails,
    pub phase: i32,
}

/// Mod source information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "modId")]
    pub mod_id: i64,
    #[serde(rename = "fileId")]
    pub file_id: i64,
    pub md5: String,
    #[serde(rename = "fileSize")]
    pub file_size: i64,
    #[serde(rename = "logicalFilename")]
    pub logical_filename: String,
    #[serde(rename = "updatePolicy")]
    pub update_policy: String,
    pub tag: String,
}

/// Mod details/metadata
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModDetails {
    pub category: String,
    #[serde(rename = "type")]
    pub mod_type: String,
}

/// Load a collection from a JSON file
pub fn load_collection(path: &Path) -> Result<Collection> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read collection at {}", path.display()))?;

    let collection: Collection =
        serde_json::from_str(&content).context("Failed to parse collection JSON")?;

    Ok(collection)
}

/// Collection statistics
#[derive(Debug, Clone)]
pub struct CollectionStats {
    pub total_mods: usize,
    pub required_mods: usize,
    pub optional_mods: usize,
    pub installed_mods: usize,
    pub missing_mods: usize,
}

impl Collection {
    /// Get collection statistics
    pub fn stats(&self) -> CollectionStats {
        let total = self.mods.len();
        let optional = self.mods.iter().filter(|m| m.optional).count();
        let required = total - optional;

        CollectionStats {
            total_mods: total,
            required_mods: required,
            optional_mods: optional,
            installed_mods: 0, // Will be calculated against installed mods
            missing_mods: 0,
        }
    }

    /// Check which mods are already installed
    /// Returns (installed_count, missing_required_mods)
    pub fn check_installed(&self, installed_mod_ids: &[i64]) -> (usize, Vec<&CollectionMod>) {
        let installed_set: std::collections::HashSet<i64> =
            installed_mod_ids.iter().copied().collect();

        let mut installed_count = 0;
        let mut missing_required = Vec::new();

        for mod_entry in &self.mods {
            if installed_set.contains(&mod_entry.source.mod_id) {
                installed_count += 1;
            } else if !mod_entry.optional {
                missing_required.push(mod_entry);
            }
        }

        (installed_count, missing_required)
    }
}
