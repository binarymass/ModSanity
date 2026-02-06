//! Game detection and management

mod proton;
pub mod skyrimse;

pub use proton::ProtonHelper;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Supported games
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GameType {
    SkyrimSE,
    SkyrimVR,
    Fallout4,
    Fallout4VR,
    Starfield,
}

impl GameType {
    /// Get the Steam App ID for this game
    pub fn steam_app_id(&self) -> u32 {
        match self {
            GameType::SkyrimSE => 489830,
            GameType::SkyrimVR => 611670,
            GameType::Fallout4 => 377160,
            GameType::Fallout4VR => 611660,
            GameType::Starfield => 1716740,
        }
    }

    /// Get the NexusMods game domain
    pub fn nexus_game_id(&self) -> &'static str {
        match self {
            GameType::SkyrimSE => "skyrimspecialedition",
            GameType::SkyrimVR => "skyrimspecialedition", // Uses same mods
            GameType::Fallout4 => "fallout4",
            GameType::Fallout4VR => "fallout4", // Uses same mods
            GameType::Starfield => "starfield",
        }
    }

    /// Get the numeric NexusMods game ID (for GraphQL queries)
    pub fn nexus_numeric_id(&self) -> i64 {
        match self {
            GameType::SkyrimSE => 1704,
            GameType::SkyrimVR => 1704,
            GameType::Fallout4 => 1151,
            GameType::Fallout4VR => 1151,
            GameType::Starfield => 4187,
        }
    }

    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            GameType::SkyrimSE => "Skyrim Special Edition",
            GameType::SkyrimVR => "Skyrim VR",
            GameType::Fallout4 => "Fallout 4",
            GameType::Fallout4VR => "Fallout 4 VR",
            GameType::Starfield => "Starfield",
        }
    }

    /// Get the game ID string
    pub fn id(&self) -> &'static str {
        match self {
            GameType::SkyrimSE => "skyrimse",
            GameType::SkyrimVR => "skyrimvr",
            GameType::Fallout4 => "fallout4",
            GameType::Fallout4VR => "fallout4vr",
            GameType::Starfield => "starfield",
        }
    }

    /// Get all supported game types
    pub fn all() -> &'static [GameType] {
        &[
            GameType::SkyrimSE,
            GameType::SkyrimVR,
            GameType::Fallout4,
            GameType::Fallout4VR,
            GameType::Starfield,
        ]
    }
}

/// Represents a detected game installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    /// Game type
    pub game_type: GameType,

    /// Short identifier (e.g., "skyrimse")
    pub id: String,

    /// Display name
    pub name: String,

    /// NexusMods game domain
    pub nexus_game_id: String,

    /// Steam App ID
    pub steam_app_id: u32,

    /// Game installation path
    pub install_path: PathBuf,

    /// Data folder path (where mods go)
    pub data_path: PathBuf,

    /// Proton prefix path (if using Proton)
    pub proton_prefix: Option<PathBuf>,

    /// AppData/Local path (for plugins.txt, etc.)
    pub appdata_path: Option<PathBuf>,

    /// plugins.txt location
    pub plugins_txt_path: Option<PathBuf>,

    /// loadorder.txt location
    pub loadorder_txt_path: Option<PathBuf>,

    /// Game executable name
    pub executable: String,

    /// Is this a VR game?
    pub is_vr: bool,
}

impl Game {
    /// Create a new Game from a detected installation
    pub fn new(game_type: GameType, install_path: PathBuf) -> Self {
        let data_path = install_path.join("Data");
        let executable = match game_type {
            GameType::SkyrimSE => "SkyrimSE.exe".to_string(),
            GameType::SkyrimVR => "SkyrimVR.exe".to_string(),
            GameType::Fallout4 => "Fallout4.exe".to_string(),
            GameType::Fallout4VR => "Fallout4VR.exe".to_string(),
            GameType::Starfield => "Starfield.exe".to_string(),
        };

        Self {
            game_type,
            id: game_type.id().to_string(),
            name: game_type.display_name().to_string(),
            nexus_game_id: game_type.nexus_game_id().to_string(),
            steam_app_id: game_type.steam_app_id(),
            install_path,
            data_path,
            proton_prefix: None,
            appdata_path: None,
            plugins_txt_path: None,
            loadorder_txt_path: None,
            executable,
            is_vr: matches!(game_type, GameType::SkyrimVR | GameType::Fallout4VR),
        }
    }

    /// Set up Proton-related paths
    pub fn with_proton_prefix(mut self, prefix: PathBuf) -> Self {
        // AppData path inside the Proton prefix
        let appdata = prefix
            .join("pfx/drive_c/users/steamuser/AppData/Local")
            .join(self.appdata_folder_name());

        self.plugins_txt_path = Some(appdata.join("plugins.txt"));
        self.loadorder_txt_path = Some(appdata.join("loadorder.txt"));
        self.appdata_path = Some(appdata);
        self.proton_prefix = Some(prefix);

        self
    }

    /// Get the AppData folder name for this game
    fn appdata_folder_name(&self) -> &str {
        match self.game_type {
            GameType::SkyrimSE | GameType::SkyrimVR => "Skyrim Special Edition",
            GameType::Fallout4 | GameType::Fallout4VR => "Fallout4",
            GameType::Starfield => "Starfield",
        }
    }

    /// Get the NexusMods game domain for API calls
    pub fn nexus_game_domain(&self) -> String {
        self.nexus_game_id.clone()
    }
}

/// Game detection utilities
pub struct GameDetector;

impl GameDetector {
    /// Detect all supported games
    pub async fn detect_all() -> Vec<Game> {
        let mut games = Vec::new();

        // Find Steam library folders
        let steam_paths = Self::find_steam_libraries();

        for steam_path in steam_paths {
            for game_type in GameType::all() {
                if let Some(game) = Self::detect_game(&steam_path, *game_type) {
                    games.push(game);
                }
            }
        }

        games
    }

    /// Find all Steam library folders
    fn find_steam_libraries() -> Vec<PathBuf> {
        let mut libraries = Vec::new();

        // Common Steam paths on Linux
        let home = dirs::home_dir().unwrap_or_default();
        let possible_paths = [
            home.join(".steam/steam"),
            home.join(".local/share/Steam"),
            PathBuf::from("/usr/share/steam"),
        ];

        for base in possible_paths {
            if !base.exists() {
                continue;
            }

            // Main steamapps
            let steamapps = base.join("steamapps");
            if steamapps.exists() {
                libraries.push(steamapps.clone());
            }

            // Parse libraryfolders.vdf for additional libraries
            let vdf_path = steamapps.join("libraryfolders.vdf");
            if let Ok(content) = std::fs::read_to_string(&vdf_path) {
                for line in content.lines() {
                    if line.contains("\"path\"") {
                        if let Some(path) = line.split('"').nth(3) {
                            let lib_path = PathBuf::from(path).join("steamapps");
                            if lib_path.exists() && !libraries.contains(&lib_path) {
                                libraries.push(lib_path);
                            }
                        }
                    }
                }
            }
        }

        libraries
    }

    /// Detect a specific game in a Steam library
    fn detect_game(steamapps: &PathBuf, game_type: GameType) -> Option<Game> {
        let common = steamapps.join("common");
        let install_path = match game_type {
            GameType::SkyrimSE => common.join("Skyrim Special Edition"),
            GameType::SkyrimVR => common.join("SkyrimVR"),
            GameType::Fallout4 => common.join("Fallout 4"),
            GameType::Fallout4VR => common.join("Fallout 4 VR"),
            GameType::Starfield => common.join("Starfield"),
        };

        if !install_path.exists() {
            return None;
        }

        let mut game = Game::new(game_type, install_path);

        // Check for Proton prefix
        let compatdata = steamapps.join("compatdata").join(game_type.steam_app_id().to_string());
        if compatdata.exists() {
            game = game.with_proton_prefix(compatdata);
        }

        Some(game)
    }
}

// Re-export for convenience
mod dirs {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }
}
