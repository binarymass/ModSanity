//! Proton/Wine prefix handling utilities

use std::path::{Path, PathBuf};

/// Helper for Proton prefix operations
pub struct ProtonHelper {
    prefix_path: PathBuf,
}

impl ProtonHelper {
    /// Create a new ProtonHelper for the given prefix
    pub fn new(prefix_path: PathBuf) -> Self {
        Self { prefix_path }
    }

    /// Get the pfx directory (actual Wine prefix)
    pub fn pfx_dir(&self) -> PathBuf {
        self.prefix_path.join("pfx")
    }

    /// Get the drive_c path
    pub fn drive_c(&self) -> PathBuf {
        self.pfx_dir().join("drive_c")
    }

    /// Get the users directory
    pub fn users_dir(&self) -> PathBuf {
        self.drive_c().join("users")
    }

    /// Get the steamuser home directory
    pub fn steamuser_home(&self) -> PathBuf {
        self.users_dir().join("steamuser")
    }

    /// Get the AppData/Local path
    pub fn appdata_local(&self) -> PathBuf {
        self.steamuser_home().join("AppData/Local")
    }

    /// Get the AppData/Roaming path
    pub fn appdata_roaming(&self) -> PathBuf {
        self.steamuser_home().join("AppData/Roaming")
    }

    /// Get the Documents path
    pub fn documents(&self) -> PathBuf {
        self.steamuser_home().join("Documents")
    }

    /// Get My Games path (common for Bethesda games)
    pub fn my_games(&self) -> PathBuf {
        self.documents().join("My Games")
    }

    /// Convert a Windows path to the Proton equivalent
    pub fn convert_windows_path(&self, windows_path: &str) -> PathBuf {
        let path = windows_path
            .replace('\\', "/")
            .replace("C:/", "")
            .replace("c:/", "");

        self.drive_c().join(path)
    }

    /// Check if the prefix is valid
    pub fn is_valid(&self) -> bool {
        self.pfx_dir().exists() && self.drive_c().exists()
    }

    /// Ensure the AppData structure exists for a game
    pub fn ensure_appdata_structure(&self, game_folder: &str) -> std::io::Result<PathBuf> {
        let appdata_game = self.appdata_local().join(game_folder);
        std::fs::create_dir_all(&appdata_game)?;
        Ok(appdata_game)
    }

    /// Read a file from the prefix, handling Windows line endings
    pub fn read_file(&self, path: &Path) -> std::io::Result<String> {
        let content = std::fs::read_to_string(path)?;
        // Normalize line endings
        Ok(content.replace("\r\n", "\n"))
    }

    /// Write a file to the prefix, using Windows line endings
    pub fn write_file(&self, path: &Path, content: &str) -> std::io::Result<()> {
        // Convert to Windows line endings for compatibility
        let content = content.replace('\n', "\r\n");
        std::fs::write(path, content)
    }
}
