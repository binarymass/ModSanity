//! Parser for MO2 modlist.txt format

use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Parser for Mod Organizer 2 modlist.txt files
pub struct ModlistParser;

impl ModlistParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a modlist.txt file
    /// Format: `*[index] [form_id] [spaces] [plugin_name]`
    /// Example: `*  1 FE 002 Skyrim.esm`
    pub fn parse_file(&self, path: &Path) -> Result<Vec<PluginEntry>> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open modlist file: {}", path.display()))?;

        let reader = BufReader::new(file);
        let mut plugins = Vec::new();
        let mut line_num = 0;

        for line in reader.lines() {
            line_num += 1;
            let line = line.context("Failed to read line")?;

            // Skip empty lines and comments
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            match self.parse_line(&line) {
                Ok(Some(entry)) => plugins.push(entry),
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!("Failed to parse line {}: {} - {}", line_num, line, e);
                    continue;
                }
            }
        }

        Ok(plugins)
    }

    /// Parse a single line from modlist.txt
    fn parse_line(&self, line: &str) -> Result<Option<PluginEntry>> {
        // Handle MO2 format variations:
        // 1. `+Mod Name` / `-Mod Name` from modlist.txt (mod entries)
        // 2. `*[index] [form_id] [plugin_name]` (plugin-like entries)
        // 3. `[plugin_name]` simple fallback (no index)

        let trimmed = line.trim();

        // MO2 modlist.txt format: leading '+' for enabled, '-' for disabled.
        if let Some(first) = trimmed.chars().next() {
            if first == '+' || first == '-' {
                let name = trimmed[1..].trim();
                if name.is_empty() || is_separator_entry(name) {
                    return Ok(None);
                }

                return Ok(Some(PluginEntry {
                    plugin_name: name.to_string(),
                    load_order: 0,
                    enabled: first == '+',
                }));
            }
        }

        // Check if enabled (starts with *)
        let enabled = trimmed.starts_with('*');
        let rest = if enabled {
            trimmed.strip_prefix('*').unwrap().trim()
        } else {
            trimmed
        };

        // Split by whitespace
        let parts: Vec<&str> = rest.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(None);
        }

        // Try to parse first part as number - if it fails, assume simple format
        let (load_order, plugin_name) = if let Ok(order) = parts[0].parse::<i32>() {
            // Has load order index
            if parts.len() < 2 {
                return Ok(None);
            }
            // Last part is the plugin name
            let name = parts[parts.len() - 1].to_string();
            (order, name)
        } else {
            // Simple format - just plugin name (or multiple parts that form the name)
            // Join all parts in case name has spaces
            let name = parts.join(" ");
            (0, name)
        };

        if plugin_name.is_empty() || is_separator_entry(&plugin_name) {
            return Ok(None);
        }

        Ok(Some(PluginEntry {
            plugin_name,
            load_order,
            enabled,
        }))
    }
}

/// A plugin entry from modlist.txt
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub plugin_name: String,
    pub load_order: i32,
    pub enabled: bool,
}

impl PluginEntry {
    /// Extract Nexus mod ID from archive-style names, if present.
    ///
    /// Example:
    /// - `Alternate Start - Live Another Life-272-4-2-5-1751059176` => `Some(272)`
    pub fn extract_nexus_mod_id(&self) -> Option<i64> {
        parse_nexus_archive_metadata(&self.plugin_name).1
    }

    /// Extract mod name from plugin filename
    /// Removes extension, version patterns, and cleans up formatting
    pub fn extract_mod_name(&self) -> String {
        let raw_name =
            parse_nexus_archive_metadata(strip_leading_state_marker(&self.plugin_name)).0;
        let name = raw_name.as_str();

        // Remove extension
        let name = name
            .strip_suffix(".esp")
            .or_else(|| name.strip_suffix(".esm"))
            .or_else(|| name.strip_suffix(".esl"))
            .unwrap_or(name);

        // Remove common version patterns
        // Examples: v1.2, -1.2.3, _v1_2, V1.0, etc.
        let re = regex_lite::Regex::new(r"[_\-\s]?[vV]?[\d]+[._][\d]+([._][\d]+)?").unwrap();
        let name = re.replace_all(name, "");

        // Replace underscores and hyphens with spaces
        let name = name.replace('_', " ").replace('-', " ");

        // Clean up multiple spaces
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");

        // Expand known abbreviations
        expand_abbreviations(&name)
    }
}

/// Parse Nexus download/archive naming conventions.
/// Returns `(normalized_name_part, nexus_mod_id_hint)`.
fn parse_nexus_archive_metadata(name: &str) -> (String, Option<i64>) {
    let mut base = name.trim().to_string();

    // Remove common archive extensions if present.
    for ext in [".zip", ".7z", ".rar"] {
        if base.to_lowercase().ends_with(ext) {
            let len = base.len() - ext.len();
            base.truncate(len);
            break;
        }
    }

    let parts: Vec<&str> = base.split('-').collect();
    if parts.len() < 3 {
        return (base, None);
    }

    // In Nexus archive names, the first significant all-digit token after the
    // name segment is typically the Nexus mod ID.
    for (idx, part) in parts.iter().enumerate().skip(1) {
        let token = part.trim();
        if token.len() < 2 || token.len() > 7 || !token.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        if let Ok(mod_id) = token.parse::<i64>() {
            let mut name_part = parts[..idx].join("-").trim().to_string();

            // Strip list-style numeric prefix like `0-` or `1-`.
            if let Ok(re) = regex_lite::Regex::new(r"^\d{1,2}\s*-\s*") {
                name_part = re.replace(&name_part, "").to_string();
            }

            return (name_part, Some(mod_id));
        }
    }

    (base, None)
}

fn strip_leading_state_marker(name: &str) -> &str {
    let trimmed = name.trim();
    if let Some(rest) = trimmed.strip_prefix('+') {
        return rest.trim_start();
    }
    if let Some(rest) = trimmed.strip_prefix('-') {
        return rest.trim_start();
    }
    if let Some(rest) = trimmed.strip_prefix('*') {
        return rest.trim_start();
    }
    trimmed
}

fn is_separator_entry(name: &str) -> bool {
    name.trim_end().to_ascii_lowercase().ends_with("_separator")
}

/// Expand common mod name abbreviations
fn expand_abbreviations(name: &str) -> String {
    let expansions = [
        ("USSEP", "Unofficial Skyrim Special Edition Patch"),
        ("USLEEP", "Unofficial Skyrim Legendary Edition Patch"),
        ("SMIM", "Static Mesh Improvement Mod"),
        ("ENB", "Enhanced Natural Beauty"),
        ("SkyUI", "SkyUI"),
        ("MCM", "Mod Configuration Menu"),
        ("SKSE", "Skyrim Script Extender"),
        ("FNIS", "Fores New Idles in Skyrim"),
        ("RaceMenu", "RaceMenu"),
        ("CBBE", "Caliente's Beautiful Bodies Edition"),
        ("UNP", "UNP Female Body Renewal"),
        ("HDT", "HDT Physics Extension"),
    ];

    // Check if entire name is the abbreviation
    for (abbr, expansion) in &expansions {
        if name.eq_ignore_ascii_case(abbr) {
            return expansion.to_string();
        }
    }

    // Check if name contains abbreviation as a word and replace it
    let mut result = name.to_string();
    for (abbr, expansion) in &expansions {
        // Split name into words and check each word
        let words: Vec<&str> = result.split_whitespace().collect();
        if words.iter().any(|w| w.eq_ignore_ascii_case(abbr)) {
            // Replace the abbreviation word with expansion
            let re = regex_lite::Regex::new(&format!(r"\b{}\b", regex_lite::escape(abbr))).unwrap();
            result = re.replace(&result, *expansion).to_string();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line() {
        let parser = ModlistParser::new();

        // Test enabled plugin
        let entry = parser
            .parse_line("*  1 FE 002 SkyUI_SE.esp")
            .unwrap()
            .unwrap();
        assert_eq!(entry.plugin_name, "SkyUI_SE.esp");
        assert_eq!(entry.load_order, 1);
        assert!(entry.enabled);

        // Test disabled plugin
        let entry = parser
            .parse_line("  2 Unofficial Skyrim Patch.esp")
            .unwrap()
            .unwrap();
        assert_eq!(entry.plugin_name, "Patch.esp");
        assert_eq!(entry.load_order, 2);
        assert!(!entry.enabled);

        // Test MO2 modlist format
        let entry = parser.parse_line("+SkyUI").unwrap().unwrap();
        assert_eq!(entry.plugin_name, "SkyUI");
        assert_eq!(entry.load_order, 0);
        assert!(entry.enabled);

        let entry = parser.parse_line("-Lux Test").unwrap().unwrap();
        assert_eq!(entry.plugin_name, "Lux Test");
        assert_eq!(entry.load_order, 0);
        assert!(!entry.enabled);

        // Ignore MO2 separators
        assert!(parser.parse_line("-Fixes_separator").unwrap().is_none());
    }

    #[test]
    fn test_extract_mod_name() {
        let entry = PluginEntry {
            plugin_name: "SkyUI_SE.esp".to_string(),
            load_order: 1,
            enabled: true,
        };
        assert_eq!(entry.extract_mod_name(), "SkyUI SE");

        let entry = PluginEntry {
            plugin_name: "SMIM-SE-Merged-All.esp".to_string(),
            load_order: 2,
            enabled: true,
        };
        assert_eq!(
            entry.extract_mod_name(),
            "Static Mesh Improvement Mod SE Merged All"
        );
    }

    #[test]
    fn test_extract_nexus_mod_id_from_archive_style_name() {
        let entry = PluginEntry {
            plugin_name: "Alternate Start - Live Another Life-272-4-2-5-1751059176".to_string(),
            load_order: 0,
            enabled: true,
        };
        assert_eq!(entry.extract_nexus_mod_id(), Some(272));
        assert_eq!(
            entry.extract_mod_name(),
            "Alternate Start Live Another Life"
        );
    }

    #[test]
    fn test_extract_nexus_mod_id_with_prefixed_index() {
        let entry = PluginEntry {
            plugin_name: "0-Elden Rim-Base 3.7.0-65625-3-7-0-1758403615".to_string(),
            load_order: 0,
            enabled: true,
        };
        assert_eq!(entry.extract_nexus_mod_id(), Some(65625));
        assert!(entry.extract_mod_name().starts_with("Elden Rim"));
    }

    #[test]
    fn test_extract_mod_name_strips_leading_markers() {
        let entry = PluginEntry {
            plugin_name: "+Soul Cairn Paper Map for FWMF".to_string(),
            load_order: 0,
            enabled: true,
        };
        assert_eq!(entry.extract_mod_name(), "Soul Cairn Paper Map for FWMF");

        let entry = PluginEntry {
            plugin_name: "-Flat World Map Framework".to_string(),
            load_order: 0,
            enabled: false,
        };
        assert_eq!(entry.extract_mod_name(), "Flat World Map Framework");
    }
}
