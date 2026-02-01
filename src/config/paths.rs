//! XDG-compliant path management

use directories::ProjectDirs;
use std::path::PathBuf;

/// Manages all application paths using XDG base directory specification
#[derive(Debug, Clone)]
pub struct Paths {
    /// Base directories from XDG
    dirs: ProjectDirs,
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

impl Paths {
    /// Create a new Paths instance
    pub fn new() -> Self {
        let dirs = ProjectDirs::from("", "", "modsanity")
            .expect("Failed to determine project directories");
        Self { dirs }
    }

    // ========== Config Paths ==========

    /// Config directory: ~/.config/modsanity/
    pub fn config_dir(&self) -> PathBuf {
        self.dirs.config_dir().to_path_buf()
    }

    /// Main config file: ~/.config/modsanity/config.toml
    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    // ========== Data Paths ==========

    /// Data directory: ~/.local/share/modsanity/
    pub fn data_dir(&self) -> PathBuf {
        self.dirs.data_dir().to_path_buf()
    }

    /// Database file: ~/.local/share/modsanity/modsanity.db
    pub fn database_file(&self) -> PathBuf {
        self.data_dir().join("modsanity.db")
    }

    /// Mods staging directory: ~/.local/share/modsanity/mods/
    pub fn mods_dir(&self) -> PathBuf {
        self.data_dir().join("mods")
    }

    /// Staging directory for a specific game
    pub fn game_mods_dir(&self, game_id: &str) -> PathBuf {
        self.mods_dir().join(game_id)
    }

    /// Staging directory for a specific mod
    pub fn mod_dir(&self, game_id: &str, mod_id: &str) -> PathBuf {
        self.game_mods_dir(game_id).join(mod_id)
    }

    /// Downloads directory: ~/.local/share/modsanity/downloads/
    pub fn downloads_dir(&self) -> PathBuf {
        self.data_dir().join("downloads")
    }

    /// Profiles directory: ~/.local/share/modsanity/profiles/
    pub fn profiles_dir(&self) -> PathBuf {
        self.data_dir().join("profiles")
    }

    /// Profile directory for a specific game
    pub fn game_profiles_dir(&self, game_id: &str) -> PathBuf {
        self.profiles_dir().join(game_id)
    }

    /// Backups directory: ~/.local/share/modsanity/backups/
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir().join("backups")
    }

    // ========== Cache Paths ==========

    /// Cache directory: ~/.cache/modsanity/
    pub fn cache_dir(&self) -> PathBuf {
        self.dirs.cache_dir().to_path_buf()
    }

    /// NexusMods API cache: ~/.cache/modsanity/nexus/
    pub fn nexus_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("nexus")
    }

    /// LOOT masterlist cache: ~/.cache/modsanity/loot/
    pub fn loot_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("loot")
    }

    /// Archive extraction cache: ~/.cache/modsanity/extract/
    pub fn extract_cache_dir(&self) -> PathBuf {
        self.cache_dir().join("extract")
    }

    // ========== Utility Methods ==========

    /// Ensure all required directories exist
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.config_dir())?;
        std::fs::create_dir_all(self.data_dir())?;
        std::fs::create_dir_all(self.mods_dir())?;
        std::fs::create_dir_all(self.downloads_dir())?;
        std::fs::create_dir_all(self.profiles_dir())?;
        std::fs::create_dir_all(self.backups_dir())?;
        std::fs::create_dir_all(self.cache_dir())?;
        std::fs::create_dir_all(self.nexus_cache_dir())?;
        Ok(())
    }
}
