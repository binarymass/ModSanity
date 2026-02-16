//! Configuration management for ModSanity
//!
//! Uses XDG-compliant paths:
//! - Config: ~/.config/modsanity/config.toml
//! - Data: ~/.local/share/modsanity/
//! - Cache: ~/.cache/modsanity/

mod paths;

pub use paths::Paths;

use anyhow::{bail, Context, Result};
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

    /// External tools configuration (Proton + Windows tool executables)
    pub external_tools: ExternalToolsConfig,

    /// Override for downloaded archives directory
    pub downloads_dir_override: Option<String>,

    /// Override for installed/staging mods root directory
    pub staging_dir_override: Option<String>,

    /// Additional user-defined game installations (GOG/manual paths).
    pub custom_games: Vec<CustomGameConfig>,

    /// Whether guided initialization has completed at least once.
    pub first_run_completed: bool,

    /// RFC3339 timestamp for last successful init completion.
    pub first_run_completed_at: Option<String>,

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
            external_tools: ExternalToolsConfig::default(),
            downloads_dir_override: None,
            staging_dir_override: None,
            custom_games: Vec::new(),
            first_run_completed: false,
            first_run_completed_at: None,
            paths: Paths::new(),
        }
    }
}

/// User-specified game install entry (for GOG/manual support).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomGameConfig {
    /// Game ID (e.g., "skyrimse", "fallout4")
    pub game_id: String,
    /// Install path containing the game executable and Data folder.
    pub install_path: String,
    /// Platform/source label: steam, gog, manual
    pub platform: String,
    /// Optional explicit Proton prefix path.
    pub proton_prefix: Option<String>,
}

impl Default for CustomGameConfig {
    fn default() -> Self {
        Self {
            game_id: String::new(),
            install_path: String::new(),
            platform: "manual".to_string(),
            proton_prefix: None,
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

impl DeploymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeploymentMethod::Symlink => "symlink",
            DeploymentMethod::Hardlink => "hardlink",
            DeploymentMethod::Copy => "copy",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DeploymentMethod::Symlink => "Symlink",
            DeploymentMethod::Hardlink => "Hardlink",
            DeploymentMethod::Copy => "Full Copy",
        }
    }

    pub fn from_cli(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "symlink" => Ok(DeploymentMethod::Symlink),
            "hardlink" => Ok(DeploymentMethod::Hardlink),
            "copy" | "fullcopy" | "full-copy" | "full_copy" => Ok(DeploymentMethod::Copy),
            other => bail!(
                "Invalid deployment method '{}'. Valid values: symlink, hardlink, copy",
                other
            ),
        }
    }
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

    /// Reduce heavy color usage in the TUI for accessibility/low-color terminals.
    pub minimal_color_mode: bool,
}

/// Supported external tools that can be launched via Proton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalTool {
    XEdit,
    SSEEdit,
    FNIS,
    Nemesis,
    Synthesis,
    BodySlide,
    OutfitStudio,
}

impl ExternalTool {
    pub fn as_id(&self) -> &'static str {
        match self {
            ExternalTool::XEdit => "xedit",
            ExternalTool::SSEEdit => "ssedit",
            ExternalTool::FNIS => "fnis",
            ExternalTool::Nemesis => "nemesis",
            ExternalTool::Synthesis => "symphony",
            ExternalTool::BodySlide => "bodyslide",
            ExternalTool::OutfitStudio => "outfitstudio",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ExternalTool::XEdit => "xEdit",
            ExternalTool::SSEEdit => "SSEEdit",
            ExternalTool::FNIS => "FNIS",
            ExternalTool::Nemesis => "Nemesis",
            ExternalTool::Synthesis => "Synthesis",
            ExternalTool::BodySlide => "BodySlide",
            ExternalTool::OutfitStudio => "Outfit Studio",
        }
    }

    pub fn all() -> &'static [ExternalTool] {
        &[
            ExternalTool::XEdit,
            ExternalTool::SSEEdit,
            ExternalTool::FNIS,
            ExternalTool::Nemesis,
            ExternalTool::Synthesis,
            ExternalTool::BodySlide,
            ExternalTool::OutfitStudio,
        ]
    }

    pub fn from_cli(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "xedit" | "x" => Ok(ExternalTool::XEdit),
            "ssedit" | "sseedit" | "sse" => Ok(ExternalTool::SSEEdit),
            "fnis" => Ok(ExternalTool::FNIS),
            "nemesis" => Ok(ExternalTool::Nemesis),
            "symphony" => Ok(ExternalTool::Synthesis),
            "bodyslide" | "bs" => Ok(ExternalTool::BodySlide),
            "outfitstudio" | "outfit-studio" | "os" => Ok(ExternalTool::OutfitStudio),
            other => bail!(
                "Unknown tool '{}'. Valid tools: xedit, ssedit, fnis, nemesis, symphony, bodyslide, outfitstudio",
                other
            ),
        }
    }
}

/// Runtime mode for launching external tools.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolRuntimeMode {
    /// Launch through Proton/Wine runtime.
    #[default]
    Proton,
    /// Launch executable directly on host.
    Native,
}

impl ToolRuntimeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolRuntimeMode::Proton => "proton",
            ToolRuntimeMode::Native => "native",
        }
    }

    pub fn from_cli(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "proton" => Ok(ToolRuntimeMode::Proton),
            "native" => Ok(ToolRuntimeMode::Native),
            other => bail!("Unknown runtime mode '{}'. Valid: proton, native", other),
        }
    }
}

/// External tool paths and Proton command configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalToolsConfig {
    /// Proton launcher command or full path (e.g., `proton`, `/path/to/proton`)
    pub proton_command: String,
    /// Optional selected Steam-managed Proton runtime ID (e.g., `steam:proton_experimental`).
    pub proton_runtime: Option<String>,
    pub xedit_path: Option<String>,
    pub ssedit_path: Option<String>,
    pub fnis_path: Option<String>,
    pub nemesis_path: Option<String>,
    pub symphony_path: Option<String>,
    pub bodyslide_path: Option<String>,
    pub outfitstudio_path: Option<String>,
    pub xedit_runtime_mode: Option<ToolRuntimeMode>,
    pub ssedit_runtime_mode: Option<ToolRuntimeMode>,
    pub fnis_runtime_mode: Option<ToolRuntimeMode>,
    pub nemesis_runtime_mode: Option<ToolRuntimeMode>,
    pub symphony_runtime_mode: Option<ToolRuntimeMode>,
    pub bodyslide_runtime_mode: Option<ToolRuntimeMode>,
    pub outfitstudio_runtime_mode: Option<ToolRuntimeMode>,
}

impl Default for ExternalToolsConfig {
    fn default() -> Self {
        Self {
            proton_command: "proton".to_string(),
            proton_runtime: None,
            xedit_path: None,
            ssedit_path: None,
            fnis_path: None,
            nemesis_path: None,
            symphony_path: None,
            bodyslide_path: None,
            outfitstudio_path: None,
            xedit_runtime_mode: None,
            ssedit_runtime_mode: None,
            fnis_runtime_mode: None,
            nemesis_runtime_mode: None,
            symphony_runtime_mode: None,
            bodyslide_runtime_mode: None,
            outfitstudio_runtime_mode: None,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            show_help: true,
            confirm_destructive: true,
            theme: "default".to_string(),
            default_mod_directory: None,
            minimal_color_mode: false,
        }
    }
}

impl Config {
    pub fn external_tool_path(&self, tool: ExternalTool) -> Option<&str> {
        match tool {
            ExternalTool::XEdit => self.external_tools.xedit_path.as_deref(),
            ExternalTool::SSEEdit => self.external_tools.ssedit_path.as_deref(),
            ExternalTool::FNIS => self.external_tools.fnis_path.as_deref(),
            ExternalTool::Nemesis => self.external_tools.nemesis_path.as_deref(),
            ExternalTool::Synthesis => self.external_tools.symphony_path.as_deref(),
            ExternalTool::BodySlide => self.external_tools.bodyslide_path.as_deref(),
            ExternalTool::OutfitStudio => self.external_tools.outfitstudio_path.as_deref(),
        }
    }

    pub fn set_external_tool_path(&mut self, tool: ExternalTool, path: Option<String>) {
        match tool {
            ExternalTool::XEdit => self.external_tools.xedit_path = path,
            ExternalTool::SSEEdit => self.external_tools.ssedit_path = path,
            ExternalTool::FNIS => self.external_tools.fnis_path = path,
            ExternalTool::Nemesis => self.external_tools.nemesis_path = path,
            ExternalTool::Synthesis => self.external_tools.symphony_path = path,
            ExternalTool::BodySlide => self.external_tools.bodyslide_path = path,
            ExternalTool::OutfitStudio => self.external_tools.outfitstudio_path = path,
        }
    }

    pub fn external_tool_runtime_mode(&self, tool: ExternalTool) -> ToolRuntimeMode {
        match tool {
            ExternalTool::XEdit => self.external_tools.xedit_runtime_mode,
            ExternalTool::SSEEdit => self.external_tools.ssedit_runtime_mode,
            ExternalTool::FNIS => self.external_tools.fnis_runtime_mode,
            ExternalTool::Nemesis => self.external_tools.nemesis_runtime_mode,
            ExternalTool::Synthesis => self.external_tools.symphony_runtime_mode,
            ExternalTool::BodySlide => self.external_tools.bodyslide_runtime_mode,
            ExternalTool::OutfitStudio => self.external_tools.outfitstudio_runtime_mode,
        }
        .unwrap_or(ToolRuntimeMode::Proton)
    }

    pub fn set_external_tool_runtime_mode(
        &mut self,
        tool: ExternalTool,
        mode: Option<ToolRuntimeMode>,
    ) {
        match tool {
            ExternalTool::XEdit => self.external_tools.xedit_runtime_mode = mode,
            ExternalTool::SSEEdit => self.external_tools.ssedit_runtime_mode = mode,
            ExternalTool::FNIS => self.external_tools.fnis_runtime_mode = mode,
            ExternalTool::Nemesis => self.external_tools.nemesis_runtime_mode = mode,
            ExternalTool::Synthesis => self.external_tools.symphony_runtime_mode = mode,
            ExternalTool::BodySlide => self.external_tools.bodyslide_runtime_mode = mode,
            ExternalTool::OutfitStudio => self.external_tools.outfitstudio_runtime_mode = mode,
        }
    }
    /// Resolve configured downloads directory (override or default XDG path)
    pub fn downloads_dir(&self) -> PathBuf {
        self.downloads_dir_override
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.paths.downloads_dir())
    }

    /// Resolve configured staging root directory (override or default XDG path)
    pub fn staging_dir(&self) -> PathBuf {
        self.staging_dir_override
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| self.paths.mods_dir())
    }

    /// Resolve staging directory for a specific game
    pub fn game_staging_dir(&self, game_id: &str) -> PathBuf {
        self.staging_dir().join(game_id)
    }

    /// Ensure required directories exist, including overrides.
    pub fn ensure_dirs(&self) -> Result<()> {
        self.paths
            .ensure_dirs()
            .context("Failed to create default application directories")?;
        std::fs::create_dir_all(self.downloads_dir())
            .context("Failed to create downloads directory")?;
        std::fs::create_dir_all(self.staging_dir())
            .context("Failed to create staging directory")?;
        Ok(())
    }

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
