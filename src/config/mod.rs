//! Configuration management for ModSanity
//!
//! Uses XDG-compliant paths:
//! - Config: ~/.config/modsanity/config.toml
//! - Data: ~/.local/share/modsanity/
//! - Cache: ~/.cache/modsanity/

mod paths;

pub use paths::Paths;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Active game identifier (e.g., "skyrimse")
    pub active_game: Option<String>,

    /// Active profile name
    pub active_profile: Option<String>,

    /// Nexus Mods API key
    pub nexus_api_key: Option<String>,

    /// Deployment settings
    pub deployment: DeploymentConfig,

    /// TUI settings
    pub tui: TuiConfig,

    /// Paths configuration
    #[serde(skip)]
    pub paths: Paths,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_game: None,
            active_profile: None,
            nexus_api_key: None,
            deployment: DeploymentConfig::default(),
            tui: TuiConfig::default(),
            paths: Paths::new(),
        }
    }
}

/// Deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeploymentConfig {
    /// Deployment method
    pub method: DeploymentMethod,

    /// Backup original files
    pub backup_originals: bool,

    /// Purge deployment on exit
    pub purge_on_exit: bool,
}

impl Default for DeploymentConfig {
    fn default() -> Self {
        Self {
            method: DeploymentMethod::Symlink,
            backup_originals: true,
            purge_on_exit: false,
        }
    }
}

/// Deployment method
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentMethod {
    #[default]
    Symlink,
    Hardlink,
    Copy,
}

/// TUI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    /// Show help panel by default
    pub show_help: bool,

    /// Confirm before destructive actions
    pub confirm_destructive: bool,

    /// Theme (future: light/dark/custom)
    pub theme: String,

    /// Default directory for bulk mod installation
    pub default_mod_directory: Option<String>,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            show_help: true,
            confirm_destructive: true,
            theme: "default".to_string(),
            default_mod_directory: None,
        }
    }
}

impl Config {
    /// Load configuration from disk or create default
    pub async fn load() -> Result<Self> {
        let paths = Paths::new();
        let config_path = paths.config_file();

        let mut config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .await
                .context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")?
        } else {
            // Create default config
            let config = Config::default();
            config.save().await?;
            config
        };

        config.paths = paths;
        Ok(config)
    }

    /// Save configuration to disk
    pub async fn save(&self) -> Result<()> {
        let config_path = self.paths.config_file();

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&config_path, content)
            .await
            .context("Failed to write config file")?;

        Ok(())
    }
}
