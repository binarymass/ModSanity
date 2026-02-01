//! Skyrim Special Edition specific functionality

use super::Game;
use std::path::PathBuf;

/// Skyrim SE specific constants and utilities
pub struct SkyrimSE;

impl SkyrimSE {
    /// Known plugin file extensions
    pub const PLUGIN_EXTENSIONS: &'static [&'static str] = &["esp", "esm", "esl"];

    /// Maximum number of active plugins (including light plugins)
    pub const MAX_PLUGINS: usize = 254;

    /// Maximum number of regular (non-light) plugins
    pub const MAX_REGULAR_PLUGINS: usize = 254;

    /// Base game master files (always loaded first)
    pub const BASE_MASTERS: &'static [&'static str] = &[
        "Skyrim.esm",
        "Update.esm",
        "Dawnguard.esm",
        "HearthFires.esm",
        "Dragonborn.esm",
    ];

    /// Anniversary Edition content (if owned)
    pub const AE_CONTENT: &'static [&'static str] = &[
        "ccBGSSSE001-Fish.esm",
        "ccBGSSSE025-AdvDSGS.esm",
        "ccBGSSSE037-Curios.esl",
        "ccQDRSSE001-SurvivalMode.esl",
    ];

    /// Get the SKSE plugins directory
    pub fn skse_plugins_dir(game: &Game) -> PathBuf {
        game.data_path.join("SKSE/Plugins")
    }

    /// Get the scripts directory
    pub fn scripts_dir(game: &Game) -> PathBuf {
        game.data_path.join("Scripts")
    }

    /// Get the interface directory (for SWF files)
    pub fn interface_dir(game: &Game) -> PathBuf {
        game.data_path.join("Interface")
    }

    /// Get the meshes directory
    pub fn meshes_dir(game: &Game) -> PathBuf {
        game.data_path.join("Meshes")
    }

    /// Get the textures directory
    pub fn textures_dir(game: &Game) -> PathBuf {
        game.data_path.join("Textures")
    }

    /// Check if a plugin is a base game master
    pub fn is_base_master(filename: &str) -> bool {
        Self::BASE_MASTERS
            .iter()
            .any(|m| m.eq_ignore_ascii_case(filename))
    }

    /// Check if a plugin is Anniversary Edition content
    pub fn is_ae_content(filename: &str) -> bool {
        Self::AE_CONTENT
            .iter()
            .any(|m| m.eq_ignore_ascii_case(filename))
    }

    /// Get the INI file path
    pub fn ini_path(game: &Game) -> Option<PathBuf> {
        game.appdata_path.as_ref().map(|p| p.join("Skyrim.ini"))
    }

    /// Get the Prefs INI file path
    pub fn prefs_ini_path(game: &Game) -> Option<PathBuf> {
        game.appdata_path
            .as_ref()
            .map(|p| p.join("SkyrimPrefs.ini"))
    }

    /// Get the Custom INI file path (for mod settings)
    pub fn custom_ini_path(game: &Game) -> Option<PathBuf> {
        game.appdata_path
            .as_ref()
            .map(|p| p.join("SkyrimCustom.ini"))
    }
}
