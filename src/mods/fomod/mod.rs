//! FOMOD installer support
//!
//! FOMOD is an XML-based installer format used by many Skyrim mods.
//! It defines installation options, conditional dependencies, and file mappings.

pub mod advanced;
mod conditions;
pub mod executor;
pub mod helpers;
mod parser;
pub mod persistence;
pub mod planner;
pub mod validation;
pub mod wizard;

pub use advanced::*;
pub use conditions::*;
pub use executor::*;
pub use helpers::*;
pub use parser::*;
pub use persistence::*;
pub use planner::*;
pub use validation::*;
pub use wizard::*;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Decode XML bytes with automatic encoding detection
fn decode_xml_bytes(bytes: &[u8]) -> Result<String> {
    // Check for UTF-16 BOM
    if bytes.len() >= 2 {
        if bytes[0] == 0xFF && bytes[1] == 0xFE {
            // UTF-16 LE with BOM
            let (decoded, _, had_errors) = encoding_rs::UTF_16LE.decode(bytes);
            if had_errors {
                tracing::warn!("UTF-16LE decoding had errors, some characters may be incorrect");
            }
            return Ok(decoded.into_owned());
        } else if bytes[0] == 0xFE && bytes[1] == 0xFF {
            // UTF-16 BE with BOM
            let (decoded, _, had_errors) = encoding_rs::UTF_16BE.decode(bytes);
            if had_errors {
                tracing::warn!("UTF-16BE decoding had errors, some characters may be incorrect");
            }
            return Ok(decoded.into_owned());
        }
    }

    // Check for UTF-8 BOM
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        // UTF-8 with BOM - decode and strip BOM
        let content = String::from_utf8_lossy(&bytes[3..]);
        return Ok(content.into_owned());
    }

    // No BOM detected - try UTF-8 first (most common)
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            // Not valid UTF-8, use lossy conversion
            tracing::warn!("XML file is not valid UTF-8, using lossy conversion");
            Ok(String::from_utf8_lossy(bytes).into_owned())
        }
    }
}

/// Check if a mod archive contains a FOMOD installer
pub fn has_fomod(mod_path: &Path) -> bool {
    find_fomod_dir(mod_path).is_some() || has_numbered_folders(mod_path)
}

/// Find a file in a directory (case-insensitive)
fn find_file_case_insensitive(dir: &Path, target_name: &str) -> Option<PathBuf> {
    let target_lower = target_name.to_lowercase();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let file_name = entry.file_name();
                let file_name_lower = file_name.to_string_lossy().to_lowercase();

                if file_name_lower == target_lower {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Check if a directory contains FOMOD config files (case-insensitive)
fn has_fomod_config(fomod_dir: &Path) -> bool {
    if !fomod_dir.exists() || !fomod_dir.is_dir() {
        return false;
    }

    let mut has_module_config = false;
    let mut has_info = false;
    let mut has_script = false;

    // Check for ModuleConfig.xml (case-insensitive)
    if let Ok(entries) = std::fs::read_dir(fomod_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }

            let file_name = entry.file_name();
            let file_name_lower = file_name.to_string_lossy().to_lowercase();

            // Check for various FOMOD config files
            if file_name_lower == "moduleconfig.xml" {
                has_module_config = true;
                tracing::debug!("Found ModuleConfig.xml: {}", entry.path().display());
            } else if file_name_lower == "info.xml" {
                has_info = true;
                tracing::debug!("Found info.xml: {}", entry.path().display());
            }
            // Some FOMODs also have these files
            else if file_name_lower == "script.cs" {
                has_script = true;
                tracing::debug!("Found Script.cs: {}", entry.path().display());
            }
        }
    }

    // ModuleConfig.xml is REQUIRED for a valid FOMOD installer
    // info.xml is just metadata and doesn't make it a FOMOD by itself
    if has_module_config {
        tracing::info!("Valid FOMOD installer found in: {} (Info: {}, Script: {})",
                      fomod_dir.display(), has_info, has_script);
        true
    } else {
        if has_info {
            tracing::debug!("Found info.xml but no ModuleConfig.xml in {} - not a valid FOMOD installer",
                          fomod_dir.display());
        }
        false
    }
}

/// Find the fomod directory, scanning ALL subdirectories recursively
pub fn find_fomod_dir(mod_path: &Path) -> Option<PathBuf> {
    tracing::debug!("Starting exhaustive FOMOD search in: {}", mod_path.display());

    use std::collections::{VecDeque, HashSet};

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new(); // Track visited paths to avoid circular references

    queue.push_back((mod_path.to_path_buf(), 0));

    let mut dirs_scanned = 0;

    while let Some((current_path, depth)) = queue.pop_front() {
        // Get canonical path to handle symlinks and avoid circular references
        let canonical = match current_path.canonicalize() {
            Ok(p) => p,
            Err(_) => current_path.clone(),
        };

        // Skip if we've already visited this path
        if !visited.insert(canonical.clone()) {
            continue;
        }

        dirs_scanned += 1;

        // Read directory entries
        let entries = match std::fs::read_dir(&current_path) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::debug!("Cannot read directory {}: {}", current_path.display(), e);
                continue;
            }
        };

        for entry in entries.filter_map(|e| e.ok()) {
            // Skip non-directories
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }

            let dir_name = entry.file_name();
            let dir_name_lower = dir_name.to_string_lossy().to_lowercase();

            // Check if this is a fomod directory with valid config
            if dir_name_lower == "fomod" && has_fomod_config(&entry.path()) {
                tracing::info!("✓ Found FOMOD at depth {} after scanning {} directories: {}",
                              depth, dirs_scanned, current_path.display());
                return Some(current_path);
            }

            // Add subdirectory to queue for exploration
            queue.push_back((entry.path(), depth + 1));
        }
    }

    tracing::warn!("✗ No FOMOD found after scanning {} directories in: {}",
                  dirs_scanned, mod_path.display());

    // Print directory structure for debugging (first 3 levels)
    if tracing::enabled!(tracing::Level::DEBUG) {
        tracing::debug!("Directory structure of {}:", mod_path.display());
        print_directory_structure(mod_path, 0, 3);
    }

    None
}

/// Print directory structure for debugging (helper function)
fn print_directory_structure(path: &Path, current_depth: usize, max_depth: usize) {
    if current_depth > max_depth {
        return;
    }

    let indent = "  ".repeat(current_depth);

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                tracing::debug!("{}📁 {}/", indent, name_str);
                print_directory_structure(&entry.path(), current_depth + 1, max_depth);
            } else {
                tracing::debug!("{}📄 {}", indent, name_str);
            }
        }
    }
}

/// Check if a mod has numbered folders (simple FOMOD structure)
pub fn has_numbered_folders(mod_path: &Path) -> bool {
    if !mod_path.exists() || !mod_path.is_dir() {
        return false;
    }

    let mut numbered_count = 0;

    if let Ok(entries) = std::fs::read_dir(mod_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    // Check if folder starts with 2 digits
                    if name_str.len() >= 2
                        && name_str.chars().nth(0).map(|c| c.is_ascii_digit()).unwrap_or(false)
                        && name_str.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
                    {
                        numbered_count += 1;
                    }
                }
            }
        }
    }

    // If we have 2 or more numbered folders, it's likely a FOMOD
    numbered_count >= 2
}

/// Get list of numbered FOMOD components
pub fn get_numbered_components(mod_path: &Path) -> anyhow::Result<Vec<FomodComponent>> {
    let mut components = Vec::new();

    for entry in std::fs::read_dir(mod_path)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy().to_string();

        // Check if folder starts with 2 digits
        if name_str.len() >= 2
            && name_str.chars().nth(0).map(|c| c.is_ascii_digit()).unwrap_or(false)
            && name_str.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false)
        {
            components.push(FomodComponent {
                name: name_str.clone(),
                description: name_str.clone(),
                path: entry.path(),
                is_required: name_str.starts_with("00"),
            });
        }
    }

    // Sort by folder name
    components.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(components)
}

/// Simple FOMOD component
#[derive(Debug, Clone)]
pub struct FomodComponent {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub is_required: bool,
}

/// Get the FOMOD directory path (checks for nested structures, case-insensitive)
pub fn fomod_dir(mod_path: &Path) -> PathBuf {
    let base = find_fomod_dir(mod_path).unwrap_or_else(|| mod_path.to_path_buf());

    // Find the actual fomod directory (case-insensitive)
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let dir_name = entry.file_name();
                let dir_name_lower = dir_name.to_string_lossy().to_lowercase();

                if dir_name_lower == "fomod" {
                    return entry.path();
                }
            }
        }
    }

    // Fallback to standard path if not found
    base.join("fomod")
}

/// FOMOD installer entry point
#[derive(Debug, Clone)]
pub struct FomodInstaller {
    pub config: ModuleConfig,
    pub mod_path: PathBuf,
}

impl FomodInstaller {
    /// Load a FOMOD installer from a mod directory
    pub fn load(mod_path: &Path) -> anyhow::Result<Self> {
        // Find the actual FOMOD base directory (may be nested)
        let fomod_base = find_fomod_dir(mod_path)
            .ok_or_else(|| anyhow::anyhow!("FOMOD directory not found in {}", mod_path.display()))?;

        // Find the actual fomod directory (case-insensitive)
        let fomod = if let Ok(entries) = std::fs::read_dir(&fomod_base) {
            entries
                .filter_map(|e| e.ok())
                .find(|entry| {
                    entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        && entry.file_name().to_string_lossy().to_lowercase() == "fomod"
                })
                .map(|e| e.path())
                .ok_or_else(|| anyhow::anyhow!("fomod directory not found in {}", fomod_base.display()))?
        } else {
            anyhow::bail!("Cannot read directory: {}", fomod_base.display());
        };

        tracing::debug!("Found fomod directory: {}", fomod.display());

        // Find ModuleConfig.xml (case-insensitive)
        let config_path = if let Some(path) = find_file_case_insensitive(&fomod, "moduleconfig.xml") {
            path
        } else {
            anyhow::bail!("ModuleConfig.xml not found in {}", fomod.display());
        };

        tracing::info!("Loading FOMOD config from: {}", config_path.display());

        // Read as bytes first, then decode based on BOM/encoding
        let bytes = std::fs::read(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        // Detect encoding and decode appropriately
        let content = decode_xml_bytes(&bytes)
            .with_context(|| format!("Failed to decode XML encoding at {}", config_path.display()))?;

        let config = parse_module_config(&content)
            .with_context(|| format!("Failed to parse FOMOD at {}", config_path.display()))?;

        Ok(Self {
            config,
            mod_path: fomod_base, // Use the base path where fomod directory is located
        })
    }

    /// Get the installation steps
    pub fn steps(&self) -> &[InstallStep] {
        &self.config.install_steps.steps
    }

    /// Check if the installer requires user interaction
    pub fn requires_wizard(&self) -> bool {
        !self.config.install_steps.steps.is_empty()
    }
}
