//! Native ModSanity modlist format (JSON) and format detection

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Native ModSanity modlist format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModSanityModlist {
    pub meta: ModlistMeta,
    pub mods: Vec<ModlistEntry>,
    pub plugins: Vec<PluginOrderEntry>,
}

/// Modlist metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModlistMeta {
    pub format_version: u32,
    pub modsanity_version: String,
    pub game_id: String,
    pub game_domain: String,
    pub exported_at: String,
    pub profile_name: Option<String>,
}

/// A mod entry in the modlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModlistEntry {
    pub name: String,
    pub version: String,
    pub nexus_mod_id: Option<i64>,
    pub nexus_file_id: Option<i64>,
    pub author: Option<String>,
    pub priority: i32,
    pub enabled: bool,
    pub category: Option<String>,
}

/// A plugin entry with load order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOrderEntry {
    pub filename: String,
    pub load_order: i32,
    pub enabled: bool,
}

/// Detected modlist format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModlistFormat {
    /// Native ModSanity JSON format
    Native,
    /// MO2 modlist.txt format
    Mo2,
}

/// Detect whether a file is native JSON or MO2 text format by content inspection
pub fn detect_format(path: &Path) -> Result<ModlistFormat> {
    let content = std::fs::read_to_string(path).context("Failed to read modlist file")?;

    let trimmed = content.trim_start();

    // JSON files start with '{' (possibly after BOM)
    if trimmed.starts_with('{') {
        Ok(ModlistFormat::Native)
    } else {
        Ok(ModlistFormat::Mo2)
    }
}

/// Load a native ModSanity modlist from a JSON file
pub fn load_native(path: &Path) -> Result<ModSanityModlist> {
    let content = std::fs::read_to_string(path).context("Failed to read modlist file")?;

    serde_json::from_str(&content).context("Failed to parse native modlist JSON")
}

/// Save a native ModSanity modlist to a JSON file
pub fn save_native(path: &Path, modlist: &ModSanityModlist) -> Result<()> {
    let json = serde_json::to_string_pretty(modlist).context("Failed to serialize modlist")?;

    std::fs::write(path, json).context("Failed to write modlist file")
}
