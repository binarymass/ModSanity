//! LOOT (Load Order Optimization Tool) integration
//!
//! This module provides both:
//! 1. Native Rust load order optimization (primary, based on LOOT principles)
//! 2. LOOT CLI integration (optional if user has LOOT installed)

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use crate::games::Game;
use super::PluginInfo;

/// Check if LOOT is installed and available
pub fn is_loot_available() -> bool {
    // Try to find LOOT executable
    if let Some(_) = find_loot_executable() {
        return true;
    }
    false
}

/// Find LOOT executable in common installation paths
fn find_loot_executable() -> Option<PathBuf> {
    // Common LOOT installation paths on Linux (when using Wine/Proton)
    let linux_paths = vec![
        "loot", // In PATH
        "/usr/bin/loot",
        "/usr/local/bin/loot",
    ];

    // Common LOOT installation paths on Windows
    let windows_paths = vec![
        r"C:\Program Files\LOOT\loot.exe",
        r"C:\Program Files (x86)\LOOT\loot.exe",
    ];

    // Try Linux paths first
    for path_str in &linux_paths {
        let path = PathBuf::from(path_str);
        if path.exists() || which::which(path_str).is_ok() {
            return Some(path);
        }
    }

    // Try Windows paths
    for path_str in &windows_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Sort plugin load order using LOOT
pub fn sort_plugins(game: &Game) -> Result<()> {
    let loot_exe = find_loot_executable()
        .ok_or_else(|| anyhow::anyhow!("LOOT executable not found"))?;

    // LOOT command-line arguments:
    // --game <game> : Specify the game (e.g., Skyrim, SkyrimSE)
    // --auto-sort   : Automatically sort and apply load order
    let game_name = map_game_to_loot(&game.id)?;

    let output = Command::new(&loot_exe)
        .arg("--game")
        .arg(&game_name)
        .arg("--auto-sort")
        .output()
        .context("Failed to execute LOOT")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("LOOT failed: {}", stderr);
    }

    Ok(())
}

/// Map game ID to LOOT game name
fn map_game_to_loot(game_id: &str) -> Result<String> {
    let loot_name = match game_id {
        "skyrim" => "Skyrim",
        "skyrimse" => "SkyrimSE",
        "skyrimvr" => "SkyrimVR",
        "fallout3" => "Fallout3",
        "falloutnv" => "FalloutNV",
        "fallout4" => "Fallout4",
        "fallout4vr" => "Fallout4VR",
        "oblivion" => "Oblivion",
        "morrowind" => "Morrowind",
        _ => bail!("Game '{}' is not supported by LOOT", game_id),
    };

    Ok(loot_name.to_string())
}

/// Sort plugins using native Rust algorithm (primary method)
/// This is based on LOOT principles but implemented in pure Rust
/// - No external dependencies required
/// - Faster than calling LOOT CLI
/// - Handles all essential sorting rules
pub fn sort_plugins_native(plugins: &mut [PluginInfo]) -> Result<()> {
    super::sort::optimize_load_order(plugins)
        .context("Failed to optimize plugin load order")
}

/// Get LOOT's suggested load order without applying it
/// Returns list of plugin filenames in suggested order
pub fn get_suggested_order(_game: &Game) -> Result<Vec<String>> {
    // LOOT doesn't have a direct "preview" mode in CLI, so this would require
    // reading the loadorder.txt after a sort, or using libloot API
    // For now, we'll just return an error suggesting to use auto-sort

    bail!("Preview mode not available. Use sort_plugins() to apply LOOT's recommendations directly.")
}
