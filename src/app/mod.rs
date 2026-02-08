//! Application state and orchestration

mod actions;
pub mod state;

pub use state::{AppState, ConfirmAction, ConfirmDialog, InputMode, Screen, UiMode};

use crate::config::{Config, DeploymentMethod, ExternalTool, ToolRuntimeMode};
use crate::db::Database;
use crate::games::{detect_proton_runtimes, Game, GameDetector, GamePlatform, GameType, ProtonRuntime};
use crate::mods::ModManager;
use crate::nexus::NexusClient;
use crate::profiles::ProfileManager;
use crate::tui::Tui;

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main application struct that orchestrates all components
pub struct App {
    /// Application configuration
    pub config: Arc<RwLock<Config>>,

    /// Application state
    pub state: Arc<RwLock<AppState>>,

    /// Database connection
    pub db: Arc<Database>,

    /// Mod manager
    pub mods: Arc<ModManager>,

    /// Profile manager
    pub profiles: Arc<ProfileManager>,

    /// Nexus Mods API client (optional, requires API key)
    pub nexus: Option<Arc<NexusClient>>,

    /// Detected games
    pub games: Vec<Game>,
}

impl App {
    /// Create a new App instance
    pub async fn new(config: Config) -> Result<Self> {
        // Ensure directories exist
        config.ensure_dirs().context("Failed to create directories")?;

        // Initialize database
        let db = Database::open(&config.paths.database_file())
            .context("Failed to open database")?;
        let db = Arc::new(db);

        // Detect games (Steam + GOG + user-configured custom paths).
        let games = GameDetector::detect_all_with_custom(&config.custom_games).await;

        // Find active game
        let active_game = config
            .active_game
            .as_ref()
            .and_then(|id| games.iter().find(|g| g.id == *id))
            .cloned();

        // Initialize state
        let state = AppState::new(active_game);

        // Initialize Nexus API client if API key is available
        let nexus = config
            .nexus_api_key
            .as_ref()
            .and_then(|key| {
                NexusClient::new(key.clone())
                    .map(Arc::new)
                    .map_err(|e| {
                        tracing::warn!("Failed to initialize Nexus API client: {}", e);
                        e
                    })
                    .ok()
            });

        // Wrap config
        let config = Arc::new(RwLock::new(config));

        // Initialize mod manager
        let mods = Arc::new(ModManager::new(config.clone(), db.clone()));

        // Initialize profile manager
        let profiles = Arc::new(ProfileManager::new(config.clone(), db.clone()));

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            db,
            mods,
            profiles,
            nexus,
            games,
        })
    }

    /// Run the TUI interface
    pub async fn run_tui(&mut self) -> Result<()> {
        let mut tui = Tui::new()?;
        tui.run(self).await
    }

    /// Get the currently active game
    pub async fn active_game(&self) -> Option<Game> {
        self.state.read().await.active_game.clone()
    }

    /// Set the active game
    pub async fn set_active_game(&mut self, game: Option<Game>) -> Result<()> {
        let mut state = self.state.write().await;
        state.active_game = game.clone();

        let mut config = self.config.write().await;
        config.active_game = game.map(|g| g.id);
        config.save().await?;

        Ok(())
    }

    /// Set deployment method in config
    pub async fn set_deployment_method(&self, method: DeploymentMethod) -> Result<()> {
        let mut config = self.config.write().await;
        config.deployment.method = method;
        config.save().await?;
        Ok(())
    }

    /// Set or clear downloads directory override.
    pub async fn set_downloads_dir_override(&self, path: Option<&str>) -> Result<()> {
        let mut config = self.config.write().await;
        config.downloads_dir_override = path
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToOwned::to_owned);
        config.ensure_dirs()?;
        config.save().await?;
        Ok(())
    }

    /// Set or clear staging directory override.
    pub async fn set_staging_dir_override(&self, path: Option<&str>) -> Result<()> {
        let mut config = self.config.write().await;
        config.staging_dir_override = path
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToOwned::to_owned);
        config.ensure_dirs()?;
        config.save().await?;
        Ok(())
    }

    /// Mark first-run initialization as completed.
    pub async fn mark_init_completed(&self) -> Result<()> {
        let mut config = self.config.write().await;
        config.first_run_completed = true;
        config.first_run_completed_at = Some(chrono::Utc::now().to_rfc3339());
        config.save().await?;
        Ok(())
    }

    /// Resolve configured downloads directory.
    pub async fn resolved_downloads_dir(&self) -> std::path::PathBuf {
        self.config.read().await.downloads_dir()
    }

    /// Resolve configured staging directory.
    pub async fn resolved_staging_dir(&self) -> std::path::PathBuf {
        self.config.read().await.staging_dir()
    }

    /// Validate a path string before saving to config.
    pub fn validate_directory_override(path: &str) -> Result<()> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let p = Path::new(trimmed);
        if p.as_os_str().is_empty() {
            anyhow::bail!("Directory path cannot be empty");
        }
        Ok(())
    }

    /// Set the Proton command used to launch external tools.
    pub async fn set_proton_command(&self, proton_cmd: &str) -> Result<()> {
        let mut config = self.config.write().await;
        let trimmed = proton_cmd.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Proton command cannot be empty");
        }
        // Custom command/path mode supersedes runtime selection.
        config.external_tools.proton_runtime = None;
        config.external_tools.proton_command = trimmed.to_string();
        config.save().await?;
        Ok(())
    }

    /// Detect available Steam-managed Proton runtimes.
    pub fn detect_proton_runtimes(&self) -> Vec<ProtonRuntime> {
        detect_proton_runtimes()
    }

    /// Select a Steam-managed Proton runtime by ID/name, or `auto`.
    pub async fn set_proton_runtime(&self, runtime: Option<&str>) -> Result<()> {
        let mut config = self.config.write().await;
        match runtime.map(str::trim).filter(|s| !s.is_empty()) {
            None => {
                config.external_tools.proton_runtime = None;
            }
            Some("auto") => {
                config.external_tools.proton_runtime = Some("auto".to_string());
            }
            Some(sel) => {
                let runtimes = detect_proton_runtimes();
                let Some(found) = find_runtime(&runtimes, sel) else {
                    anyhow::bail!(
                        "Proton runtime '{}' not found. Run 'modsanity tool list-proton' to see detected runtimes.",
                        sel
                    );
                };
                config.external_tools.proton_runtime = Some(found.id.clone());
            }
        }
        config.save().await?;
        Ok(())
    }

    /// Set or clear an external tool executable path.
    pub async fn set_external_tool_path(
        &self,
        tool: ExternalTool,
        path: Option<&str>,
    ) -> Result<()> {
        let mut config = self.config.write().await;
        let value = path
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToOwned::to_owned);
        config.set_external_tool_path(tool, value);
        config.save().await?;
        Ok(())
    }

    /// Set/clear per-tool runtime mode override.
    pub async fn set_external_tool_runtime_mode(
        &self,
        tool: ExternalTool,
        mode: Option<ToolRuntimeMode>,
    ) -> Result<()> {
        let mut config = self.config.write().await;
        config.set_external_tool_runtime_mode(tool, mode);
        config.save().await?;
        Ok(())
    }

    /// Launch an external tool through Proton, using active game's prefix.
    pub async fn launch_external_tool(&self, tool: ExternalTool, args: &[String]) -> Result<i32> {
        let game = self
            .active_game()
            .await
            .ok_or_else(|| anyhow::anyhow!("No game selected"))?;
        let (proton_cmd, tool_path, runtime_mode) = {
            let config = self.config.read().await;
            let tool_path = config
                .external_tool_path(tool)
                .ok_or_else(|| anyhow::anyhow!("Tool path not configured for {}", tool.display_name()))?
                .to_string();
            let mode = config.external_tool_runtime_mode(tool);
            let proton_cmd = if mode == ToolRuntimeMode::Proton {
                Some(self.resolve_proton_launcher_from_config(&config)?)
            } else {
                None
            };
            (proton_cmd, tool_path, mode)
        };

        let resolved_tool_path = expand_user_path(&tool_path);
        let mut command = if runtime_mode == ToolRuntimeMode::Proton {
            let proton_prefix = game
                .proton_prefix
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Active game has no Proton prefix detected"))?;
            let resolved_proton_cmd = expand_user_path(proton_cmd.as_deref().unwrap_or("proton"));
            let mut command = tokio::process::Command::new(&resolved_proton_cmd);
            command.arg("run").arg(&resolved_tool_path);
            // Typical Proton/Wine env for out-of-steam launches.
            command.env("STEAM_COMPAT_DATA_PATH", &proton_prefix);
            command.env("WINEPREFIX", proton_prefix.join("pfx"));
            command
        } else {
            tokio::process::Command::new(&resolved_tool_path)
        };
        for arg in args {
            command.arg(arg);
        }
        if let Some(parent) = Path::new(&resolved_tool_path).parent() {
            command.current_dir(parent);
        }

        let status = command
            .status()
            .await
            .with_context(|| format!("Failed to launch {} via Proton", tool.display_name()))?;

        Ok(status.code().unwrap_or_default())
    }

    /// Register/update a custom game install path (GOG/manual/steam override).
    pub async fn add_custom_game_path(
        &mut self,
        game_id: &str,
        install_path: &str,
        platform: GamePlatform,
        proton_prefix: Option<&str>,
    ) -> Result<()> {
        if GameType::from_id(game_id).is_none() {
            anyhow::bail!("Unknown game id '{}'", game_id);
        }
        let trimmed_path = install_path.trim();
        if trimmed_path.is_empty() {
            anyhow::bail!("Install path cannot be empty");
        }
        if !Path::new(trimmed_path).exists() {
            anyhow::bail!("Install path does not exist: {}", trimmed_path);
        }

        let mut config = self.config.write().await;
        let platform_str = match platform {
            GamePlatform::Steam => "steam".to_string(),
            GamePlatform::Gog => "gog".to_string(),
            GamePlatform::Manual => "manual".to_string(),
        };

        if let Some(existing) = config.custom_games.iter_mut().find(|entry| {
            entry.game_id.eq_ignore_ascii_case(game_id) && entry.install_path == trimmed_path
        }) {
            existing.platform = platform_str;
            existing.proton_prefix = proton_prefix
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(ToOwned::to_owned);
        } else {
            config.custom_games.push(crate::config::CustomGameConfig {
                game_id: game_id.to_string(),
                install_path: trimmed_path.to_string(),
                platform: platform_str,
                proton_prefix: proton_prefix
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                    .map(ToOwned::to_owned),
            });
        }
        config.save().await?;
        let custom_games = config.custom_games.clone();
        drop(config);

        self.games = GameDetector::detect_all_with_custom(&custom_games).await;
        Ok(())
    }

    /// Remove a custom game install path.
    pub async fn remove_custom_game_path(&mut self, game_id: &str, install_path: &str) -> Result<()> {
        let mut config = self.config.write().await;
        let before = config.custom_games.len();
        config.custom_games.retain(|entry| {
            !(entry.game_id.eq_ignore_ascii_case(game_id) && entry.install_path == install_path)
        });
        if config.custom_games.len() == before {
            anyhow::bail!("No matching custom game path found for {} at {}", game_id, install_path);
        }
        config.save().await?;
        let custom_games = config.custom_games.clone();
        drop(config);

        self.games = GameDetector::detect_all_with_custom(&custom_games).await;
        Ok(())
    }
}

fn find_runtime<'a>(runtimes: &'a [ProtonRuntime], selection: &str) -> Option<&'a ProtonRuntime> {
    runtimes.iter().find(|rt| {
        rt.id.eq_ignore_ascii_case(selection)
            || rt.name.eq_ignore_ascii_case(selection)
            || rt.proton_path.to_string_lossy().eq_ignore_ascii_case(selection)
    })
}

fn pick_auto_runtime(runtimes: &[ProtonRuntime]) -> Option<&ProtonRuntime> {
    if runtimes.is_empty() {
        return None;
    }

    // Prefer Proton Experimental when present; otherwise pick lexicographically last.
    if let Some(exp) = runtimes
        .iter()
        .find(|rt| rt.name.to_ascii_lowercase().contains("experimental"))
    {
        return Some(exp);
    }

    runtimes
        .iter()
        .max_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
}

impl App {
    /// Resolve configured Proton launcher executable path (runtime or command mode).
    pub fn resolve_proton_launcher_from_config(&self, config: &Config) -> Result<String> {
        if let Some(selected) = config.external_tools.proton_runtime.as_deref() {
            let runtimes = detect_proton_runtimes();
            let resolved = if selected.eq_ignore_ascii_case("auto") {
                pick_auto_runtime(&runtimes)
            } else {
                find_runtime(&runtimes, selected)
            };
            let Some(runtime) = resolved else {
                anyhow::bail!(
                    "Configured Proton runtime '{}' was not detected. Run 'modsanity tool list-proton'.",
                    selected
                );
            };
            return Ok(runtime.proton_path.display().to_string());
        }
        Ok(config.external_tools.proton_command.clone())
    }
}

fn expand_user_path(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{}/{}", home, rest);
        }
    }
    raw.to_string()
}
