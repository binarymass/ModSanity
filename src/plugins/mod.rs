//! Plugin (ESP/ESM/ESL) management

mod loadorder;
pub mod loot;
pub mod masterlist;
mod parser;
pub mod sort;

pub use loadorder::*;
pub use parser::*;

use crate::games::Game;
use anyhow::Result;
use std::path::PathBuf;

/// Plugin file types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Master file (.esm)
    Master,
    /// Plugin file (.esp)
    Plugin,
    /// Light plugin (.esl)
    Light,
}

impl PluginType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "esm" => Some(Self::Master),
            "esp" => Some(Self::Plugin),
            "esl" => Some(Self::Light),
            _ => None,
        }
    }
}

/// Represents a plugin file
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin filename
    pub filename: String,

    /// Full path to the plugin
    pub path: PathBuf,

    /// Plugin type based on extension and flags
    pub plugin_type: PluginType,

    /// Is the plugin enabled in plugins.txt
    pub enabled: bool,

    /// Load order index
    pub load_order: usize,

    /// Master files this plugin depends on
    pub masters: Vec<String>,

    /// Is this a light plugin (ESL-flagged ESP or .esl)
    pub is_light: bool,

    /// Plugin description from header
    pub description: Option<String>,

    /// Author from header
    pub author: Option<String>,
}

/// Get all plugins for a game
pub fn get_plugins(game: &Game) -> Result<Vec<PluginInfo>> {
    let mut plugins = Vec::new();

    // Read plugins.txt for enabled status
    let enabled_plugins = read_plugins_txt(game)?;

    // Scan data directory for plugin files
    let data_path = &game.data_path;
    if !data_path.exists() {
        return Ok(plugins);
    }

    for entry in std::fs::read_dir(data_path)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let plugin_type = match PluginType::from_extension(&ext) {
            Some(t) => t,
            None => continue,
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let enabled = enabled_plugins.contains(&filename.to_lowercase());

        // Try to parse header
        let header = parse_plugin_header(&path).ok();

        let is_light = plugin_type == PluginType::Light
            || header.as_ref().map(|h| h.is_light).unwrap_or(false);

        plugins.push(PluginInfo {
            filename: filename.clone(),
            path,
            plugin_type,
            enabled,
            load_order: 0, // Will be set when sorting
            masters: header
                .as_ref()
                .map(|h| h.masters.clone())
                .unwrap_or_default(),
            is_light,
            description: header.as_ref().and_then(|h| h.description.clone()),
            author: header.as_ref().and_then(|h| h.author.clone()),
        });
    }

    // Sort by load order
    sort_plugins(&mut plugins, game)?;

    Ok(plugins)
}

/// Sort plugins according to load order rules
fn sort_plugins(plugins: &mut [PluginInfo], game: &Game) -> Result<()> {
    // Read loadorder.txt if it exists
    let load_order = read_loadorder_txt(game)?;

    // Create order map
    let order_map: std::collections::HashMap<String, usize> = load_order
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_lowercase(), i))
        .collect();

    // Sort: masters first, then by loadorder.txt, then alphabetically
    plugins.sort_by(|a, b| {
        // Masters always first
        let a_master = a.plugin_type == PluginType::Master;
        let b_master = b.plugin_type == PluginType::Master;
        if a_master != b_master {
            return b_master.cmp(&a_master);
        }

        // Then by loadorder.txt position
        let a_order = order_map.get(&a.filename.to_lowercase());
        let b_order = order_map.get(&b.filename.to_lowercase());

        match (a_order, b_order) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.filename.to_lowercase().cmp(&b.filename.to_lowercase()),
        }
    });

    // Update load order indices
    for (i, plugin) in plugins.iter_mut().enumerate() {
        plugin.load_order = i;
    }

    Ok(())
}
