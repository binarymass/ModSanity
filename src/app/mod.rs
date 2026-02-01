//! Application state and orchestration

mod actions;
pub mod state;

pub use state::{AppState, ConfirmAction, ConfirmDialog, InputMode, Screen};

use crate::config::Config;
use crate::db::Database;
use crate::games::{Game, GameDetector};
use crate::mods::ModManager;
use crate::nexus::NexusClient;
use crate::profiles::ProfileManager;
use crate::tui::Tui;

use anyhow::{Context, Result};
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
        config.paths.ensure_dirs().context("Failed to create directories")?;

        // Initialize database
        let db = Database::open(&config.paths.database_file())
            .context("Failed to open database")?;
        let db = Arc::new(db);

        // Detect games
        let games = GameDetector::detect_all().await;

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
}
