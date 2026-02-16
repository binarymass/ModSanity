//! LOOT masterlist parser and loader

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// LOOT masterlist root structure
#[derive(Debug, Deserialize, Serialize)]
pub struct Masterlist {
    #[serde(default)]
    pub plugins: Vec<PluginMetadata>,
    #[serde(default)]
    pub globals: Vec<MessageContent>,
    #[serde(default)]
    pub bash_tags: Vec<BashTag>,
}

/// Plugin metadata from masterlist
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginMetadata {
    pub name: String,

    #[serde(default)]
    pub after: Vec<FileEntry>,

    #[serde(default)]
    pub req: Vec<FileEntry>,

    #[serde(default)]
    pub inc: Vec<FileEntry>,

    #[serde(default)]
    pub tag: Vec<Tag>,

    #[serde(default)]
    pub dirty: Vec<DirtyInfo>,

    #[serde(default)]
    pub clean: Vec<CleanInfo>,

    #[serde(default)]
    pub msg: Vec<Message>,

    #[serde(default)]
    pub url: Vec<Location>,

    #[serde(default)]
    pub group: Option<String>,
}

/// File entry (can be simple string or complex condition)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FileEntry {
    Simple(String),
    Conditional {
        name: String,
        #[serde(default)]
        condition: Option<String>,
    },
}

impl FileEntry {
    pub fn name(&self) -> &str {
        match self {
            FileEntry::Simple(s) => s,
            FileEntry::Conditional { name, .. } => name,
        }
    }
}

/// Tag entry
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Tag {
    Simple {
        name: String,
    },
    Conditional {
        name: String,
        condition: String,
    },
    Suggested {
        name: String,
        condition: Option<String>,
        #[serde(default)]
        suggestion: bool,
    },
}

impl Tag {
    pub fn name(&self) -> &str {
        match self {
            Tag::Simple { name } => name,
            Tag::Conditional { name, .. } => name,
            Tag::Suggested { name, .. } => name,
        }
    }
}

/// Dirty plugin information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DirtyInfo {
    pub crc: u32,
    pub util: String,
    #[serde(default)]
    pub itm: u32,
    #[serde(default)]
    pub udr: u32,
    #[serde(default)]
    pub nav: u32,
}

/// Clean plugin information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CleanInfo {
    pub crc: u32,
    pub util: String,
}

/// Message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    pub content: String,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Say,
    Warn,
    Error,
}

/// Global message content
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageContent {
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    pub content: String,
    #[serde(default)]
    pub condition: Option<String>,
}

/// Location/URL
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Location {
    Simple(String),
    Complex { link: String, name: Option<String> },
}

/// Bash tag
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BashTag {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Load and parse the LOOT masterlist
pub fn load_masterlist(path: &Path) -> Result<Masterlist> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read masterlist at {}", path.display()))?;

    let masterlist: Masterlist =
        serde_yaml::from_str(&content).context("Failed to parse masterlist YAML")?;

    Ok(masterlist)
}

/// Build a lookup map for quick plugin metadata access
pub fn build_metadata_map(masterlist: &Masterlist) -> HashMap<String, PluginMetadata> {
    let mut map = HashMap::new();

    for plugin in &masterlist.plugins {
        map.insert(plugin.name.to_lowercase(), plugin.clone());
    }

    map
}

/// Get load_after rules for a plugin
pub fn get_load_after_rules(
    plugin_name: &str,
    metadata_map: &HashMap<String, PluginMetadata>,
) -> Vec<String> {
    let key = plugin_name.to_lowercase();

    if let Some(metadata) = metadata_map.get(&key) {
        metadata
            .after
            .iter()
            .map(|entry| entry.name().to_lowercase())
            .collect()
    } else {
        Vec::new()
    }
}

/// Get requirements for a plugin
pub fn get_requirements(
    plugin_name: &str,
    metadata_map: &HashMap<String, PluginMetadata>,
) -> Vec<String> {
    let key = plugin_name.to_lowercase();

    if let Some(metadata) = metadata_map.get(&key) {
        metadata
            .req
            .iter()
            .map(|entry| entry.name().to_lowercase())
            .collect()
    } else {
        Vec::new()
    }
}

/// Get group for a plugin (default, early loaders, late loaders, etc.)
pub fn get_group(plugin_name: &str, metadata_map: &HashMap<String, PluginMetadata>) -> String {
    let key = plugin_name.to_lowercase();

    if let Some(metadata) = metadata_map.get(&key) {
        metadata
            .group
            .clone()
            .unwrap_or_else(|| "default".to_string())
    } else {
        "default".to_string()
    }
}

/// Get messages for a plugin
pub fn get_messages(
    plugin_name: &str,
    metadata_map: &HashMap<String, PluginMetadata>,
) -> Vec<Message> {
    let key = plugin_name.to_lowercase();

    if let Some(metadata) = metadata_map.get(&key) {
        metadata.msg.clone()
    } else {
        Vec::new()
    }
}

/// Check if a plugin is dirty
pub fn check_dirty(
    plugin_name: &str,
    crc: u32,
    metadata_map: &HashMap<String, PluginMetadata>,
) -> Option<DirtyInfo> {
    let key = plugin_name.to_lowercase();

    if let Some(metadata) = metadata_map.get(&key) {
        metadata.dirty.iter().find(|d| d.crc == crc).cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_simple() {
        let yaml = r#""SomePlugin.esp""#;
        let entry: FileEntry = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(entry.name(), "SomePlugin.esp");
    }

    #[test]
    fn test_file_entry_conditional() {
        let yaml = r#"
name: "SomePlugin.esp"
condition: "file(\"OtherPlugin.esp\")"
"#;
        let entry: FileEntry = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(entry.name(), "SomePlugin.esp");
    }
}
