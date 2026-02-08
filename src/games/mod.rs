//! Game detection and management

mod proton;
mod proton_runtime;
pub mod skyrimse;

pub use proton::ProtonHelper;
pub use proton_runtime::{detect_proton_runtimes, ProtonRuntime};

use crate::config::CustomGameConfig;
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
    /// Parse from stable game ID.
    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_ascii_lowercase().as_str() {
            "skyrimse" => Some(GameType::SkyrimSE),
            "skyrimvr" => Some(GameType::SkyrimVR),
            "fallout4" => Some(GameType::Fallout4),
            "fallout4vr" => Some(GameType::Fallout4VR),
            "starfield" => Some(GameType::Starfield),
            _ => None,
        }
    }

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

/// Source platform for a detected game install.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GamePlatform {
    #[default]
    Steam,
    Gog,
    Manual,
}

impl GamePlatform {
    pub fn display_name(&self) -> &'static str {
        match self {
            GamePlatform::Steam => "Steam",
            GamePlatform::Gog => "GOG",
            GamePlatform::Manual => "Manual",
        }
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

    /// Installation source platform.
    #[serde(default)]
    pub platform: GamePlatform,
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
            platform: GamePlatform::Steam,
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

    /// Set source platform.
    pub fn with_platform(mut self, platform: GamePlatform) -> Self {
        self.platform = platform;
        self
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

        // Also scan common GOG install locations.
        for game_type in GameType::all() {
            if let Some(game) = Self::detect_gog_game(*game_type) {
                if !games.iter().any(|g| g.id == game.id && g.install_path == game.install_path) {
                    games.push(game);
                }
            }
        }

        Self::dedupe_games(games)
    }

    /// Detect Steam + custom configured entries.
    pub async fn detect_all_with_custom(custom: &[CustomGameConfig]) -> Vec<Game> {
        let mut games = Self::detect_all().await;

        for entry in custom {
            let Some(game_type) = GameType::from_id(&entry.game_id) else {
                tracing::warn!("Ignoring custom game with unknown id '{}'", entry.game_id);
                continue;
            };

            let install_path = PathBuf::from(entry.install_path.trim());
            if !install_path.exists() {
                tracing::warn!(
                    "Ignoring custom game '{}' because path does not exist: {}",
                    entry.game_id,
                    install_path.display()
                );
                continue;
            }

            let platform = match entry.platform.to_ascii_lowercase().as_str() {
                "steam" => GamePlatform::Steam,
                "gog" => GamePlatform::Gog,
                _ => GamePlatform::Manual,
            };

            let mut game = Game::new(game_type, install_path.clone()).with_platform(platform);
            if let Some(prefix) = entry
                .proton_prefix
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
            {
                let prefix = PathBuf::from(prefix);
                if prefix.exists() {
                    game = game.with_proton_prefix(prefix);
                }
            } else if let Some(prefix) = Self::infer_prefix_from_install_path(&install_path) {
                game = game.with_proton_prefix(prefix);
            }

            if !games
                .iter()
                .any(|g| g.id == game.id && g.install_path == game.install_path)
            {
                games.push(game);
            }
        }

        Self::dedupe_games(games)
    }

    fn dedupe_games(games: Vec<Game>) -> Vec<Game> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for game in games {
            let canonical = game
                .install_path
                .canonicalize()
                .unwrap_or_else(|_| game.install_path.clone());
            let key = format!("{}:{}", game.id, canonical.display());
            if seen.insert(key) {
                out.push(game);
            }
        }
        out
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

        let mut game = Game::new(game_type, install_path).with_platform(GamePlatform::Steam);

        // Check for Proton prefix
        let compatdata = steamapps.join("compatdata").join(game_type.steam_app_id().to_string());
        if compatdata.exists() {
            game = game.with_proton_prefix(compatdata);
        }

        Some(game)
    }

    /// Detect GOG installs in common Linux paths and wine prefixes.
    fn detect_gog_game(game_type: GameType) -> Option<Game> {
        let home = dirs::home_dir().unwrap_or_default();
        let title = game_type.display_name();
        let mut candidates = vec![
            home.join(format!("GOG Games/{}", title)),
            home.join(format!("Games/GOG Games/{}", title)),
            home.join(format!("Games/{}", title)),
            home.join(format!(".local/share/Steam/steamapps/compatdata/{}/pfx/drive_c/GOG Games/{}", game_type.steam_app_id(), title)),
        ];

        // Known extra aliases observed in the wild.
        if matches!(game_type, GameType::SkyrimSE) {
            candidates.push(home.join("GOG Games/Skyrim Anniversary Edition"));
            candidates.push(home.join(".local/share/Steam/steamapps/compatdata/1711230/pfx/drive_c/GOG Games/Skyrim Special Edition"));
            candidates.push(home.join(".local/share/Steam/steamapps/compatdata/1711230/pfx/drive_c/GOG Games/Skyrim Anniversary Edition"));
        }

        for install_path in candidates {
            if !install_path.exists() {
                continue;
            }
            let exe = match game_type {
                GameType::SkyrimSE => "SkyrimSE.exe",
                GameType::SkyrimVR => "SkyrimVR.exe",
                GameType::Fallout4 => "Fallout4.exe",
                GameType::Fallout4VR => "Fallout4VR.exe",
                GameType::Starfield => "Starfield.exe",
            };
            if !install_path.join(exe).exists() {
                continue;
            }

            let mut game = Game::new(game_type, install_path.clone()).with_platform(GamePlatform::Gog);
            if let Some(prefix) = Self::infer_prefix_from_install_path(&install_path) {
                game = game.with_proton_prefix(prefix);
            }
            return Some(game);
        }

        None
    }

    /// Infer Proton prefix root from an install path inside a wine prefix.
    fn infer_prefix_from_install_path(install_path: &PathBuf) -> Option<PathBuf> {
        let mut cur = Some(install_path.as_path());
        while let Some(path) = cur {
            if path.ends_with("pfx/drive_c") {
                return path.parent().map(std::path::Path::to_path_buf);
            }
            cur = path.parent();
        }
        None
    }
}

// Re-export for convenience
mod dirs {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }
}
