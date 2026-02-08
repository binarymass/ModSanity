//! Terminal User Interface using ratatui

mod ui;
mod widgets;
pub mod screens;

use crate::app::{App, InputMode, Screen};
use crate::app::state::AppState;
use crate::config::ExternalTool;
use crate::db::Database;
use crate::plugins;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// TUI application wrapper
pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Tui {
    /// Create a new TUI instance
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Set up the terminal
    fn setup(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;
        self.terminal.hide_cursor()?;
        Ok(())
    }

    /// Restore the terminal
    fn restore(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }


    /// Run the TUI main loop
    pub async fn run(&mut self, app: &mut App) -> Result<()> {
        self.setup()?;

        // Load initial data
        self.load_initial_data(app).await?;

        let result = self.event_loop(app).await;

        self.restore()?;
        result
    }

    /// Load initial data for the TUI
    async fn load_initial_data(&self, app: &mut App) -> Result<()> {
        let mut state = app.state.write().await;
        state.is_loading = true;
        drop(state);

        // If no game selected, go to game selection
        if app.active_game().await.is_none() && !app.games.is_empty() {
            let mut state = app.state.write().await;
            state.current_screen = Screen::GameSelect;
        }

        // Load categories (game-independent)
        if let Ok(categories) = app.db.get_all_categories() {
            let mut state = app.state.write().await;
            state.categories = categories;
        }

        // Load data if a game is selected
        if let Some(game) = app.active_game().await {
            // Load mods
            if let Ok(mods) = app.mods.list_mods(&game.id).await {
                let mut state = app.state.write().await;
                state.installed_mods = mods;
            }

            // Load plugins
            if let Ok(plugins_list) = plugins::get_plugins(&game) {
                let mut state = app.state.write().await;
                state.plugins = plugins_list;
            }

            // Load profiles
            if let Ok(profiles) = app.profiles.list_profiles(&game.id).await {
                let mut state = app.state.write().await;
                state.profiles = profiles;
            }

            // Load catalog browse data if catalog is populated
            let game_domain = match game.id.as_str() {
                "skyrimse" | "skyrimvr" => "skyrimspecialedition",
                id => id,
            };

            if let Ok(sync_state) = app.db.get_sync_state(game_domain) {
                let total_mods = app.db.count_catalog_mods(game_domain).unwrap_or(0);
                let mut state = app.state.write().await;
                state.catalog_game_domain = game_domain.to_string();
                state.catalog_sync_state = Some(crate::app::state::CatalogSyncStatus {
                    current_page: sync_state.current_page,
                    completed: sync_state.completed,
                    last_sync: sync_state.last_sync,
                    last_error: sync_state.last_error,
                    total_mods,
                });
                state.catalog_total_count = total_mods;

                if sync_state.completed && total_mods > 0 {
                    if let Ok(results) = app.db.list_catalog_mods(game_domain, 0, 100) {
                        state.catalog_browse_results = results;
                        state.catalog_browse_offset = 0;
                        state.selected_catalog_index = 0;
                    }
                }
            }

            // Load saved modlists
            if let Ok(modlists) = app.db.get_modlists_for_game(&game.id) {
                let mut state = app.state.write().await;
                state.saved_modlists = modlists;
            }
        }

        let mut state = app.state.write().await;
        state.is_loading = false;
        state.show_help = false; // Don't show help by default
        Ok(())
    }

    /// Reload data for current game
    async fn reload_data(&self, app: &mut App) -> Result<()> {
        // Reload categories
        if let Ok(categories) = app.db.get_all_categories() {
            let mut state = app.state.write().await;
            state.categories = categories;
        }

        if let Some(game) = app.active_game().await {
            // Load mods
            if let Ok(mods) = app.mods.list_mods(&game.id).await {
                let mut state = app.state.write().await;
                state.installed_mods = mods;
            }

            // Load plugins
            if let Ok(plugins_list) = plugins::get_plugins(&game) {
                let mut state = app.state.write().await;
                state.plugins = plugins_list;
            }

            // Load profiles
            if let Ok(profiles) = app.profiles.list_profiles(&game.id).await {
                let mut state = app.state.write().await;
                state.profiles = profiles;
            }
        }
        Ok(())
    }

    fn settings_tool_for_index(index: usize) -> Option<ExternalTool> {
        match index {
            9 => Some(ExternalTool::XEdit),
            10 => Some(ExternalTool::SSEEdit),
            11 => Some(ExternalTool::FNIS),
            12 => Some(ExternalTool::Nemesis),
            13 => Some(ExternalTool::Symphony),
            14 => Some(ExternalTool::BodySlide),
            15 => Some(ExternalTool::OutfitStudio),
            _ => None,
        }
    }

    fn spawn_browse_search(
        state: Arc<RwLock<AppState>>,
        nexus: Arc<crate::nexus::NexusClient>,
        game_id: Option<String>,
        query: Option<String>,
        sort: crate::nexus::graphql::SortBy,
        offset: i32,
        limit: i32,
    ) {
        tokio::spawn(async move {
            if let Some(game_id) = game_id {
                let game_domain = match game_id.as_str() {
                    "skyrimse" | "skyrimvr" => "skyrimspecialedition",
                    id => id,
                };

                match nexus
                    .search_mods(crate::nexus::graphql::ModSearchParams {
                        game_domain: Some(game_domain.to_string()),
                        query: query.clone(),
                        author: None,
                        category: None,
                        sort_by: sort,
                        offset: Some(offset),
                        limit: Some(limit),
                    })
                    .await
                {
                    Ok(page) => {
                        let result_count = page.results.len() as i64;
                        let total = page.total_count;
                        let start = if result_count > 0 {
                            offset as i64 + 1
                        } else {
                            0
                        };
                        let end = if result_count > 0 {
                            offset as i64 + result_count
                        } else {
                            0
                        };

                        let mut state = state.write().await;
                        state.browse_results = page.results;
                        state.selected_browse_index = 0;
                        state.browse_offset = offset;
                        state.browse_total_count = total;
                        state.browsing = false;

                        if total > 0 {
                            if let Some(ref q) = query {
                                state.set_status(format!(
                                    "Showing {}-{} of {} results for: {}",
                                    start, end, total, q
                                ));
                            } else {
                                state.set_status(format!(
                                    "Showing {}-{} of {} top mods",
                                    start, end, total
                                ));
                            }
                        } else {
                            if let Some(ref q) = query {
                                state.set_status(format!("Found 0 results for: {}", q));
                            } else {
                                state.set_status("No mods found".to_string());
                            }
                        }
                    }
                    Err(e) => {
                        let mut state = state.write().await;
                        state.browsing = false;
                        state.set_status(format!("Search error: {}", e));
                    }
                }
            } else {
                let mut state = state.write().await;
                state.browsing = false;
                state.set_status("No active game selected".to_string());
            }
        });
    }

    fn spawn_load_modlist(
        state: Arc<RwLock<AppState>>,
        db: Arc<Database>,
        path: String,
    ) {
        tokio::spawn(async move {

            // Detect format
            let format = match crate::import::detect_format(std::path::Path::new(&path)) {
                Ok(f) => f,
                Err(e) => {
                    let mut state = state.write().await;
                    state.set_status_error(format!("Error reading file: {}", e));
                    return;
                }
            };

            match format {
                crate::import::ModlistFormat::Native => {
                    // Load native modlist
                    let modlist = match crate::import::modlist_format::load_native(std::path::Path::new(&path)) {
                        Ok(m) => m,
                        Err(e) => {
                            let mut state = state.write().await;
                            state.set_status_error(format!("Parse error: {}", e));
                            return;
                        }
                    };

                    // Get game_id
                    let game_id = {
                        let state = state.read().await;
                        state.active_game.as_ref().map(|g| g.id.clone())
                    };

                    let game_id = match game_id {
                        Some(id) => id,
                        None => {
                            let mut state = state.write().await;
                            state.set_status_error("No active game selected");
                            return;
                        }
                    };

                    // Validate game match
                    if modlist.meta.game_id != game_id {
                        let mut state = state.write().await;
                        state.set_status_error(format!(
                            "Modlist is for {} but active game is {}",
                            modlist.meta.game_id, game_id
                        ));
                        return;
                    }

                    let modlist_name = Self::modlist_name_from_path(&path, "Imported Native Modlist");
                    let db_entries: Vec<crate::db::ModlistEntryRecord> = modlist
                        .mods
                        .iter()
                        .enumerate()
                        .map(|(i, m)| crate::db::ModlistEntryRecord {
                            id: None,
                            modlist_id: 0,
                            name: m.name.clone(),
                            nexus_mod_id: m.nexus_mod_id,
                            plugin_name: None,
                            match_confidence: None,
                            position: i as i32,
                            enabled: m.enabled,
                            author: m.author.clone(),
                            version: Some(m.version.clone()),
                        })
                        .collect();

                    if let Err(e) = db.upsert_modlist_with_entries(
                        &game_id,
                        &modlist_name,
                        None,
                        Some(&path),
                        &db_entries,
                    ) {
                        let mut state = state.write().await;
                        state.set_status_error(format!("Failed to store modlist in DB: {}", e));
                        return;
                    }

                    // Library check
                    let check_result = match crate::import::library_check::check_library(
                        &db,
                        &game_id,
                        modlist.mods,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            let mut state = state.write().await;
                            state.set_status_error(format!("Library check error: {}", e));
                            return;
                        }
                    };

                    // Prepare review data
                    let review = crate::app::state::ModlistReviewData {
                        source_path: path,
                        format: "Native JSON".to_string(),
                        total_mods: check_result.already_installed.len() + check_result.needs_download.len(),
                        already_installed: check_result.already_installed
                            .iter()
                            .map(|(entry, _)| entry.name.clone())
                            .collect(),
                        needs_download: check_result.needs_download,
                        total_plugins: modlist.plugins.len(),
                    };

                    // Update state
                    let mut state = state.write().await;
                    state.modlist_review_data = Some(review);
                    state.selected_modlist_entry = 0;
                    state.goto(Screen::ModlistReview);
                    state.set_status_success(format!("Loaded and stored modlist: {}", modlist_name));
                }
                crate::import::ModlistFormat::Mo2 => {
                    // Delegate to existing import
                    let mut state = state.write().await;
                    state.import_file_path = path;
                    state.goto(Screen::Import);
                    state.set_status_info("Use Enter to import MO2 modlist");
                }
            }
        });
    }

    fn spawn_load_saved_modlist(
        state: Arc<RwLock<AppState>>,
        db: Arc<Database>,
        modlist_id: i64,
        modlist_name: String,
    ) {
        tokio::spawn(async move {
            let game_id = {
                let state = state.read().await;
                state.active_game.as_ref().map(|g| g.id.clone())
            };

            let game_id = match game_id {
                Some(id) => id,
                None => {
                    let mut state = state.write().await;
                    state.set_status_error("No active game selected");
                    return;
                }
            };

            let entries = match db.get_modlist_entries(modlist_id) {
                Ok(entries) => entries,
                Err(e) => {
                    let mut state = state.write().await;
                    state.set_status_error(format!("Failed to read saved modlist entries: {}", e));
                    return;
                }
            };

            if entries.is_empty() {
                let mut state = state.write().await;
                state.set_status_error("Saved modlist has no entries");
                return;
            }

            let plugin_count = entries.iter().filter(|e| e.plugin_name.is_some()).count();
            let mod_entries: Vec<crate::import::modlist_format::ModlistEntry> = entries
                .into_iter()
                .map(|entry| crate::import::modlist_format::ModlistEntry {
                    name: entry.name,
                    version: entry.version.unwrap_or_else(|| "unknown".to_string()),
                    nexus_mod_id: entry.nexus_mod_id,
                    nexus_file_id: None,
                    author: entry.author,
                    priority: entry.position,
                    enabled: entry.enabled,
                    category: None,
                })
                .collect();

            let check_result = match crate::import::library_check::check_library(
                &db,
                &game_id,
                mod_entries,
            ) {
                Ok(r) => r,
                Err(e) => {
                    let mut state = state.write().await;
                    state.set_status_error(format!("Library check error: {}", e));
                    return;
                }
            };

            let review = crate::app::state::ModlistReviewData {
                source_path: format!("saved: {}", modlist_name),
                format: "Saved Modlist".to_string(),
                total_mods: check_result.already_installed.len() + check_result.needs_download.len(),
                already_installed: check_result
                    .already_installed
                    .iter()
                    .map(|(entry, _)| entry.name.clone())
                    .collect(),
                needs_download: check_result.needs_download,
                total_plugins: plugin_count,
            };

            let mut state = state.write().await;
            state.modlist_picker_for_loading = false;
            state.modlist_review_data = Some(review);
            state.selected_modlist_entry = 0;
            state.goto(Screen::ModlistReview);
            state.set_status_success("Loaded saved modlist for review");
        });
    }

    fn spawn_queue_modlist_downloads(
        state: Arc<RwLock<AppState>>,
        db: Arc<Database>,
    ) {
        tokio::spawn(async move {
            // Get review data
            let (needs_download, game_id) = {
                let state = state.read().await;
                let review = match &state.modlist_review_data {
                    Some(r) => r,
                    None => return,
                };
                let game_id = match &state.active_game {
                    Some(g) => g.id.clone(),
                    None => return,
                };
                (review.needs_download.clone(), game_id)
            };

            let queue_manager = crate::queue::QueueManager::new(db);
            let batch_id = queue_manager.create_batch();

            let mut queue_position = 0;
            for entry in &needs_download {
                let nexus_mod_id = match entry.nexus_mod_id {
                    Some(id) if id > 0 => id,
                    _ => continue,
                };

                let queue_entry = crate::queue::QueueEntry {
                    id: 0,
                    batch_id: batch_id.clone(),
                    game_id: game_id.clone(),
                    queue_position,
                    plugin_name: entry.name.clone(),
                    mod_name: entry.name.clone(),
                    nexus_mod_id,
                    selected_file_id: entry.nexus_file_id,
                    auto_install: true,
                    match_confidence: Some(1.0),
                    alternatives: Vec::new(),
                    status: crate::queue::QueueStatus::Matched,
                    progress: 0.0,
                    error: None,
                };

                if let Err(e) = queue_manager.add_entry(queue_entry) {
                    let mut state = state.write().await;
                    state.set_status_error(format!("Error adding to queue: {}", e));
                    return;
                }

                queue_position += 1;
            }

            let entries = match queue_manager.get_batch(&batch_id) {
                Ok(v) => v,
                Err(e) => {
                    let mut state = state.write().await;
                    state.set_status_error(format!("Error loading queue entries: {}", e));
                    return;
                }
            };

            // Navigate to queue screen
            let mut state = state.write().await;
            state.import_batch_id = Some(batch_id);
            state.queue_entries = entries;
            state.selected_queue_index = 0;
            state.queue_processing = false;
            state.modlist_review_data = None;
            state.goto(Screen::DownloadQueue);
            state.set_status_success(format!("Queued {} downloads", queue_position));
        });
    }

    fn modlist_name_from_path(path: &str, fallback: &str) -> String {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Main event loop
    async fn event_loop(&mut self, app: &mut App) -> Result<()> {
        loop {
            // Draw UI
            {
                let state = app.state.read().await;
                self.terminal.draw(|f| ui::draw(f, app, &state))?;
            }

            // Check for quit
            if app.state.read().await.should_quit {
                break;
            }

            // Poll for events
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) => {
                        self.handle_key(app, key.code, key.modifiers).await?;
                    }
                    Event::Mouse(mouse) => {
                        self.handle_mouse(app, mouse).await?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Handle keyboard input
    async fn handle_key(&self, app: &mut App, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        let mut state = app.state.write().await;

        // Handle input mode
        if state.input_mode == InputMode::ModInstallPath {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    // Expand ~ to home directory
                    let expanded_path = if path.starts_with("~/") {
                        std::env::var("HOME")
                            .map(|h| format!("{}/{}", h, &path[2..]))
                            .unwrap_or_else(|_| path.clone())
                    } else {
                        path.clone()
                    };

                    // Check if it's a directory - if so, list archives
                    let path_obj = std::path::Path::new(&expanded_path);
                    if path_obj.is_dir() {
                        // List archive files in directory
                        if let Ok(entries) = std::fs::read_dir(path_obj) {
                            let archives: Vec<_> = entries
                                .filter_map(|e| e.ok())
                                .filter(|e| {
                                    if let Some(ext) = e.path().extension() {
                                        matches!(ext.to_str(), Some("zip" | "7z" | "rar"))
                                    } else {
                                        false
                                    }
                                })
                                .collect();

                            if archives.is_empty() {
                                let mut state = app.state.write().await;
                                state.set_status("No mod archives found in directory");
                            } else {
                                let mut state = app.state.write().await;
                                state.set_status(format!("Found {} archives - select files manually", archives.len()));
                            }
                        }
                        return Ok(());
                    }

                    // Install single mod file
                    if let Some(game) = app.active_game().await {
                        let state_clone = app.state.clone();

                        // Create progress callback
                        let progress_callback = std::sync::Arc::new(move |current_file: String, processed: usize, total: usize| {
                            if let Ok(mut state) = state_clone.try_write() {
                                let percent = if total > 0 {
                                    ((processed as f64 / total as f64) * 100.0) as u16
                                } else {
                                    0
                                };

                                state.installation_progress = Some(crate::app::state::InstallProgress {
                                    percent,
                                    current_file,
                                    total_files: total,
                                    processed_files: processed,
                                    // Single mod install - no bulk context
                                    current_mod_name: None,
                                    current_mod_index: None,
                                    total_mods: None,
                                });
                            }
                        });

                        match app.mods.install_from_archive(&game.id, &expanded_path, Some(progress_callback), None, None).await {
                            Ok(crate::mods::InstallResult::Completed(installed)) => {
                                // Clear progress FIRST to prevent UI corruption
                                {
                                    let mut state = app.state.write().await;
                                    state.installation_progress = None;
                                    state.status_message = None; // Clear any lingering status
                                }

                                self.refresh_mods(app).await?;

                                let mut state = app.state.write().await;
                                state.set_status(format!("Installed: {} (v{})", installed.name, installed.version));
                            }
                            Ok(crate::mods::InstallResult::RequiresWizard(context)) => {
                                // Clear progress
                                {
                                    let mut state = app.state.write().await;
                                    state.installation_progress = None;
                                    state.status_message = None;
                                }

                                // Initialize wizard state
                                use crate::mods::fomod::wizard::init_wizard_state;
                                use crate::app::state::{FomodWizardState, WizardPhase};

                                let wizard = init_wizard_state(&context.installer.config);
                                let wizard_state = FomodWizardState {
                                    installer: context.installer.clone(),
                                    wizard,
                                    current_step: 0,
                                    current_group: 0,
                                    selected_option: 0,
                                    validation_errors: Vec::new(),
                                    mod_name: context.mod_name.clone(),
                                    staging_path: context.staging_path.clone(),
                                    preview_files: None,
                                    phase: WizardPhase::Overview,
                                    existing_mod_id: None,
                                };

                                let mut state = app.state.write().await;
                                state.fomod_wizard_state = Some(wizard_state);
                                state.goto(crate::app::state::Screen::FomodWizard);
                            }
                            Err(e) => {
                                let mut state = app.state.write().await;
                                state.installation_progress = None;
                                state.status_message = None; // Clear any lingering status
                                state.set_status(format!("Error: {}", e));
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::CollectionPath {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    // Expand ~ to home directory
                    let expanded_path = if path.starts_with("~/") {
                        std::env::var("HOME")
                            .map(|h| format!("{}/{}", h, &path[2..]))
                            .unwrap_or_else(|_| path.clone())
                    } else {
                        path.clone()
                    };

                    // Load collection
                    self.load_collection(app, &expanded_path).await?;
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ProfileNameInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let name = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    // Create profile
                    if let Some(game) = app.active_game().await {
                        match app.profiles.create_profile(&game.id, &name).await {
                            Ok(_) => {
                                self.reload_data(app).await?;
                                let mut state = app.state.write().await;
                                state.set_status(format!("Created profile: {}", name));
                            }
                            Err(e) => {
                                let mut state = app.state.write().await;
                                state.set_status(format!("Error: {}", e));
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ModDirectoryInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let directory = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    // Save to config
                    let dir_to_save = if directory.is_empty() {
                        None
                    } else {
                        Some(directory.clone())
                    };

                    {
                        let mut config = app.config.write().await;
                        config.tui.default_mod_directory = dir_to_save.clone();

                        // Save config to disk
                        if let Err(e) = config.save().await {
                            let mut state = app.state.write().await;
                            state.set_status(format!("Error saving config: {}", e));
                            return Ok(());
                        }
                    }

                    let mut state = app.state.write().await;
                    if let Some(dir) = dir_to_save {
                        state.set_status(format!("Default mod directory set to: {}", dir));
                    } else {
                        state.set_status("Default mod directory cleared".to_string());
                    }

                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::DownloadsDirectoryInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let directory = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    if let Err(e) = App::validate_directory_override(&directory) {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Invalid directory: {}", e));
                        return Ok(());
                    }

                    if let Err(e) = app
                        .set_downloads_dir_override(if directory.trim().is_empty() {
                            None
                        } else {
                            Some(directory.as_str())
                        })
                        .await
                    {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Error saving downloads directory: {}", e));
                        return Ok(());
                    }

                    let resolved = app.resolved_downloads_dir().await;
                    let mut state = app.state.write().await;
                    if directory.trim().is_empty() {
                        state.set_status(format!(
                            "Downloads directory override cleared: {}",
                            resolved.display()
                        ));
                    } else {
                        state.set_status(format!("Downloads directory set to: {}", resolved.display()));
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::StagingDirectoryInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let directory = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    if let Err(e) = App::validate_directory_override(&directory) {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Invalid directory: {}", e));
                        return Ok(());
                    }

                    if let Err(e) = app
                        .set_staging_dir_override(if directory.trim().is_empty() {
                            None
                        } else {
                            Some(directory.as_str())
                        })
                        .await
                    {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Error saving staging directory: {}", e));
                        return Ok(());
                    }

                    let resolved = app.resolved_staging_dir().await;
                    let mut state = app.state.write().await;
                    if directory.trim().is_empty() {
                        state.set_status(format!(
                            "Staging directory override cleared: {}",
                            resolved.display()
                        ));
                    } else {
                        state.set_status(format!("Staging directory set to: {}", resolved.display()));
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ProtonCommandInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let value = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    if let Err(e) = app.set_proton_command(&value).await {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Error saving proton command: {}", e));
                        return Ok(());
                    }

                    let mut state = app.state.write().await;
                    state.set_status(format!("Proton command set to: {}", value.trim()));
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ExternalToolPathInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    let selected_idx = state.selected_setting_index;
                    state.input_buffer.clear();
                    drop(state);

                    let Some(tool) = Self::settings_tool_for_index(selected_idx) else {
                        let mut state = app.state.write().await;
                        state.set_status("Invalid settings selection for tool path".to_string());
                        return Ok(());
                    };

                    if let Err(e) = app
                        .set_external_tool_path(
                            tool,
                            if path.trim().is_empty() { None } else { Some(path.as_str()) },
                        )
                        .await
                    {
                        let mut state = app.state.write().await;
                        state.set_status(format!("Error saving {} path: {}", tool.display_name(), e));
                        return Ok(());
                    }

                    let mut state = app.state.write().await;
                    if path.trim().is_empty() {
                        state.set_status(format!("{} path cleared", tool.display_name()));
                    } else {
                        state.set_status(format!("{} path set", tool.display_name()));
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::NexusApiKeyInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let api_key = state.input_buffer.clone();
                    state.input_buffer.clear();
                    drop(state);

                    // Save to config
                    let key_to_save = if api_key.is_empty() {
                        None
                    } else {
                        Some(api_key.clone())
                    };

                    {
                        let mut config = app.config.write().await;
                        config.nexus_api_key = key_to_save.clone();

                        // Save config to disk
                        if let Err(e) = config.save().await {
                            let mut state = app.state.write().await;
                            state.set_status(format!("Error saving config: {}", e));
                            return Ok(());
                        }
                    }

                    // Reinitialize Nexus client with new API key
                    if let Some(key) = key_to_save {
                        match crate::nexus::NexusClient::new(key.clone()) {
                            Ok(client) => {
                                app.nexus = Some(Arc::new(client));
                                let mut state = app.state.write().await;
                                state.set_status("NexusMods API key saved successfully".to_string());
                            }
                            Err(e) => {
                                let mut state = app.state.write().await;
                                state.set_status(format!("Error initializing Nexus client: {}", e));
                            }
                        }
                    } else {
                        app.nexus = None;
                        let mut state = app.state.write().await;
                        state.set_status("NexusMods API key cleared".to_string());
                    }

                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::BrowseSearch {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let query = state.input_buffer.clone();
                    state.input_buffer.clear();

                    if query.is_empty() {
                        state.set_status("Search query cannot be empty".to_string());
                        return Ok(());
                    }

                    if app.nexus.is_none() {
                        state.set_status("Browse requires Nexus API key".to_string());
                        return Ok(());
                    }

                    state.browse_query = query.clone();
                    state.browse_showing_default = false;
                    state.browsing = true;
                    state.browse_offset = 0;
                    if state.browse_limit <= 0 {
                        state.browse_limit = 50;
                    }
                    state.browse_total_count = 0;
                    state.set_status(format!("Searching for: {}", query));

                    let sort = state.browse_sort;
                    let limit = state.browse_limit;
                    let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                    let nexus_clone = app.nexus.as_ref().unwrap().clone();
                    let state_clone = app.state.clone();

                    drop(state);

                    Self::spawn_browse_search(
                        state_clone,
                        nexus_clone,
                        game_id,
                        Some(query),
                        sort,
                        0,
                        limit,
                    );

                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::PluginPositionInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let position_str = state.input_buffer.clone();
                    state.input_buffer.clear();

                    if let Ok(target_position) = position_str.parse::<usize>() {
                        let plugin_count = state.plugins.len();
                        if target_position == 0 {
                            state.set_status("Position must be 1 or greater".to_string());
                        } else if target_position > plugin_count {
                            state.set_status(format!(
                                "Position {} is out of range (max: {})",
                                target_position, plugin_count
                            ));
                        } else {
                            // Convert 1-based position to 0-based index
                            let target_index = target_position - 1;
                            let current_index = state.selected_plugin_index;

                            if target_index != current_index {
                                // Remove plugin from current position
                                let plugin = state.plugins.remove(current_index);
                                // Insert at target position
                                state.plugins.insert(target_index, plugin);
                                // Update selection to follow the moved plugin
                                state.selected_plugin_index = target_index;
                                state.plugin_dirty = true;

                                // Update load_order for all plugins
                                for (i, p) in state.plugins.iter_mut().enumerate() {
                                    p.load_order = i;
                                }

                                state.set_status(format!("Moved to position {}", target_position));
                            } else {
                                state.set_status("Plugin is already at that position".to_string());
                            }
                        }
                    } else {
                        state.set_status("Please enter a valid number".to_string());
                    }

                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ModSearch {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let query = state.input_buffer.clone();
                    state.input_buffer.clear();
                    state.mod_search_query = query.clone();
                    state.selected_mod_index = 0; // Reset selection when search changes

                    if query.is_empty() {
                        state.set_status("Search cleared".to_string());
                    } else {
                        state.set_status(format!("Searching for: {}", query));
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::PluginSearch {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let query = state.input_buffer.clone();
                    state.input_buffer.clear();
                    state.plugin_search_query = query.clone();
                    state.selected_plugin_index = 0; // Reset selection when search changes

                    if query.is_empty() {
                        state.set_status("Plugin search cleared".to_string());
                    } else {
                        state.set_status(format!("Searching plugins for: {}", query));
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ImportFilePath {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    state.input_buffer.clear();

                    // Expand ~ to home directory
                    let expanded_path = if path.starts_with("~/") {
                        std::env::var("HOME")
                            .map(|h| format!("{}/{}", h, &path[2..]))
                            .unwrap_or_else(|_| path.clone())
                    } else {
                        path.clone()
                    };

                    state.import_file_path = expanded_path;
                    state.set_status("Press Enter to start import");
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::SaveModlistPath {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    let format = state.modlist_save_format.clone();
                    state.input_buffer.clear();

                    // Expand ~ to home directory
                    let expanded_path = if path.starts_with("~/") {
                        std::env::var("HOME")
                            .map(|h| format!("{}/{}", h, &path[2..]))
                            .unwrap_or_else(|_| path.clone())
                    } else {
                        path.clone()
                    };

                    // Inline the save logic since we need App context
                    let game = match app.active_game().await {
                        Some(g) => g,
                        None => {
                            let mut state = app.state.write().await;
                            state.set_status_error("No game selected");
                            return Ok(());
                        }
                    };

                    // Clone what we need for the async task
                    let state_clone = app.state.clone();
                    let db_clone = app.db.clone();
                    let mods_clone = app.mods.clone();
                    let config_clone = app.config.clone();
                    let game_id = game.id.clone();
                    let game_nexus_domain = game.nexus_game_domain();

                    drop(state);

                    // Spawn save task
                    tokio::spawn(async move {
                        use crate::import::modlist_format::{ModSanityModlist, ModlistMeta, ModlistEntry, PluginOrderEntry};
                        use crate::plugins;
                        use anyhow::Context;

                        let result: anyhow::Result<()> = async {
                            let out_path = std::path::Path::new(&expanded_path);

                            match format.as_str() {
                                "native" | "json" => {
                                    let mods = mods_clone.list_mods(&game_id).await?;

                                    // Get category names
                                    let categories = db_clone.get_all_categories()?;
                                    let cat_map: std::collections::HashMap<i64, String> = categories
                                        .into_iter()
                                        .filter_map(|c| c.id.map(|id| (id, c.name)))
                                        .collect();

                                    let mod_entries: Vec<ModlistEntry> = mods.iter().map(|m| {
                                        ModlistEntry {
                                            name: m.name.clone(),
                                            version: m.version.clone(),
                                            nexus_mod_id: m.nexus_mod_id,
                                            nexus_file_id: m.nexus_file_id,
                                            author: m.author.clone(),
                                            priority: m.priority,
                                            enabled: m.enabled,
                                            category: m.category_id.and_then(|id| cat_map.get(&id).cloned()),
                                        }
                                    }).collect();

                                    // Get plugins
                                    let plugin_entries: Vec<PluginOrderEntry> = match plugins::get_plugins(&game) {
                                        Ok(plist) => plist.iter().map(|p| {
                                            PluginOrderEntry {
                                                filename: p.filename.clone(),
                                                load_order: p.load_order as i32,
                                                enabled: p.enabled,
                                            }
                                        }).collect(),
                                        Err(_) => Vec::new(),
                                    };

                                    let profile_name = config_clone.read().await.active_profile.clone();

                                    let modlist = ModSanityModlist {
                                        meta: ModlistMeta {
                                            format_version: 1,
                                            modsanity_version: crate::APP_VERSION.to_string(),
                                            game_id: game_id.clone(),
                                            game_domain: game_nexus_domain.clone(),
                                            exported_at: chrono::Utc::now().to_rfc3339(),
                                            profile_name,
                                        },
                                        mods: mod_entries,
                                        plugins: plugin_entries,
                                    };

                                    crate::import::modlist_format::save_native(out_path, &modlist)?;
                                }
                                "mo2" => {
                                    // Write MO2 modlist.txt format (plugin list)
                                    let plugin_list = match plugins::get_plugins(&game) {
                                        Ok(plist) => plist,
                                        Err(e) => anyhow::bail!("Failed to read plugins: {}", e),
                                    };

                                    let mut lines = Vec::new();
                                    for plugin in &plugin_list {
                                        let prefix = if plugin.enabled { "*" } else { "" };
                                        lines.push(format!("{}{}", prefix, plugin.filename));
                                    }

                                    std::fs::write(out_path, lines.join("\n"))
                                        .context("Failed to write MO2 modlist file")?;
                                }
                                _ => anyhow::bail!("Unknown format '{}'", format),
                            }

                            // Persist as a DB-stored modlist snapshot as well.
                            let snapshot_mods = mods_clone.list_mods(&game_id).await?;
                            let db_entries: Vec<crate::db::ModlistEntryRecord> = snapshot_mods
                                .iter()
                                .enumerate()
                                .map(|(i, m)| crate::db::ModlistEntryRecord {
                                    id: None,
                                    modlist_id: 0,
                                    name: m.name.clone(),
                                    nexus_mod_id: m.nexus_mod_id,
                                    plugin_name: None,
                                    match_confidence: None,
                                    position: i as i32,
                                    enabled: m.enabled,
                                    author: m.author.clone(),
                                    version: Some(m.version.clone()),
                                })
                                .collect();
                            let modlist_name = Self::modlist_name_from_path(&expanded_path, "Saved Modlist");
                            db_clone.upsert_modlist_with_entries(
                                &game_id,
                                &modlist_name,
                                None,
                                None,
                                &db_entries,
                            )?;

                            Ok(())
                        }.await;

                        match result {
                            Ok(_) => {
                                let mut state = state_clone.write().await;
                                state.set_status_success(format!("Modlist saved to {}", expanded_path));
                            }
                            Err(e) => {
                                let mut state = state_clone.write().await;
                                state.set_status_error(format!("Error saving modlist: {}", e));
                            }
                        }
                    });
                    return Ok(());
                }
                KeyCode::Tab => {
                    // Toggle format between "native" and "mo2"
                    state.modlist_save_format = if state.modlist_save_format == "native" {
                        "mo2".to_string()
                    } else {
                        "native".to_string()
                    };
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::LoadModlistPath {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let path = state.input_buffer.clone();
                    state.input_buffer.clear();

                    // Expand ~ to home directory
                    let expanded_path = if path.starts_with("~/") {
                        std::env::var("HOME")
                            .map(|h| format!("{}/{}", h, &path[2..]))
                            .unwrap_or_else(|_| path.clone())
                    } else {
                        path.clone()
                    };

                    state.set_status("Loading modlist...");
                    drop(state);

                    // Spawn load task
                    let state_clone = app.state.clone();
                    let db_clone = app.db.clone();
                    Self::spawn_load_modlist(state_clone, db_clone, expanded_path);
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::CatalogSearch {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let query = state.input_buffer.clone();
                    state.input_buffer.clear();
                    state.catalog_search_query = query.clone();
                    state.selected_catalog_index = 0;
                    state.catalog_browse_offset = 0;

                    let game_domain = state.catalog_game_domain.clone();
                    drop(state);

                    if query.is_empty() {
                        // Load default page
                        screens::nexus_catalog::load_catalog_page(app, &game_domain, 0, "").await?;
                    } else {
                        screens::nexus_catalog::load_catalog_page(app, &game_domain, 0, &query).await?;
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::ModlistNameInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let name = state.input_buffer.clone();
                    state.input_buffer.clear();
                    let active_id = state.active_modlist_id;

                    if !name.is_empty() {
                        if let Some(game) = &state.active_game {
                            let game_id = game.id.clone();
                            drop(state);

                            if let Some(modlist_id) = active_id {
                                // Rename existing modlist
                                match app.db.rename_modlist(modlist_id, &name) {
                                    Ok(_) => {
                                        if let Ok(lists) = app.db.get_modlists_for_game(&game_id) {
                                            let mut state = app.state.write().await;
                                            state.saved_modlists = lists;
                                            state.active_modlist_id = None;
                                            state.set_status_success(format!("Renamed modlist to: {}", name));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.active_modlist_id = None;
                                        state.set_status_error(format!("Error: {}", e));
                                    }
                                }
                            } else {
                                // Create new modlist
                                match app.db.create_modlist(&game_id, &name, None, None) {
                                    Ok(_) => {
                                        if let Ok(lists) = app.db.get_modlists_for_game(&game_id) {
                                            let mut state = app.state.write().await;
                                            state.saved_modlists = lists;
                                            state.set_status_success(format!("Created modlist: {}", name));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.set_status_error(format!("Error: {}", e));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::QueueManualModIdInput {
            match key {
                KeyCode::Enter => {
                    state.input_mode = InputMode::Normal;
                    let value = state.input_buffer.trim().to_string();
                    let selected_idx = state.selected_queue_index;
                    let batch_id = state.import_batch_id.clone();
                    let selected_entry = state.queue_entries.get(selected_idx).cloned();
                    state.input_buffer.clear();
                    drop(state);

                    let nexus_mod_id = match value.parse::<i64>() {
                        Ok(id) if id > 0 => id,
                        _ => {
                            let mut state = app.state.write().await;
                            state.set_status_error("Enter a valid positive Nexus mod ID");
                            return Ok(());
                        }
                    };

                    let Some(entry) = selected_entry else {
                        let mut state = app.state.write().await;
                        state.set_status_error("No queue entry selected");
                        return Ok(());
                    };

                    let queue_manager = crate::queue::QueueManager::new(app.db.clone());
                    if let Err(e) = queue_manager.resolve_entry(
                        entry.id,
                        nexus_mod_id,
                        &entry.mod_name,
                        crate::queue::QueueStatus::Matched,
                    ) {
                        let mut state = app.state.write().await;
                        state.set_status_error(format!("Failed to resolve queue entry: {}", e));
                        return Ok(());
                    }

                    if let Some(batch_id) = batch_id {
                        if let Ok(entries) = queue_manager.get_batch(&batch_id) {
                            let mut state = app.state.write().await;
                            state.queue_entries = entries;
                            if !state.queue_entries.is_empty() {
                                state.selected_queue_index = selected_idx.min(state.queue_entries.len() - 1);
                            } else {
                                state.selected_queue_index = 0;
                            }
                            state.selected_queue_alternative_index = 0;
                            state.set_status_success(format!("Resolved entry with Nexus mod ID {}", nexus_mod_id));
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.input_mode = InputMode::Normal;
                    state.input_buffer.clear();
                }
                KeyCode::Backspace => {
                    state.input_buffer.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    state.input_buffer.push(c);
                }
                _ => {}
            }
            return Ok(());
        } else if state.input_mode == InputMode::FomodComponentSelection {
            match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.fomod_selection_index > 0 {
                        state.fomod_selection_index -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.fomod_selection_index < state.fomod_components.len().saturating_sub(1) {
                        state.fomod_selection_index += 1;
                    }
                }
                KeyCode::Char(' ') => {
                    // Toggle selection
                    let idx = state.fomod_selection_index;
                    if let Some(pos) = state.selected_fomod_components.iter().position(|&i| i == idx) {
                        state.selected_fomod_components.remove(pos);
                    } else {
                        state.selected_fomod_components.push(idx);
                    }
                }
                KeyCode::Enter => {
                    // Confirm selection and proceed with installation
                    state.input_mode = InputMode::Normal;
                    let archive_path = state.pending_install_archive.take();
                    let selected_indices = state.selected_fomod_components.clone();
                    let components = state.fomod_components.clone();
                    state.fomod_components.clear();
                    state.selected_fomod_components.clear();
                    state.fomod_selection_index = 0;
                    drop(state);

                    if let Some(archive_path) = archive_path {
                        self.install_fomod_components(app, &archive_path, &components, &selected_indices).await?;
                    }
                }
                KeyCode::Esc => {
                    // Cancel
                    state.input_mode = InputMode::Normal;
                    state.pending_install_archive = None;
                    state.fomod_components.clear();
                    state.selected_fomod_components.clear();
                    state.fomod_selection_index = 0;
                    state.set_status("FOMOD installation cancelled");
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle confirmation dialogs
        // Handle requirements dialog
        if let Some(ref mut dialog) = state.show_requirements {
            let missing_count = dialog.missing_mods.len();

            match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    state.show_requirements = None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if missing_count > 0 && dialog.selected_index < missing_count - 1 {
                        dialog.selected_index += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if dialog.selected_index > 0 {
                        dialog.selected_index -= 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char('d') => {
                    // Download selected requirement
                    if missing_count > 0 && dialog.selected_index < missing_count {
                        let req = dialog.missing_mods[dialog.selected_index].clone();
                        let game_domain = dialog.game_domain.clone();
                        let game_id_numeric = dialog.game_id_numeric;

                        state.show_requirements = None;
                        state.set_status(format!("Fetching files for {}...", req.name));
                        drop(state);

                        // Fetch mod files and show file picker
                        if let Some(ref nexus) = app.nexus {
                            let nexus_clone = nexus.clone();
                            let state_clone = app.state.clone();

                            tokio::spawn(async move {
                                match nexus_clone.get_mod_files(game_id_numeric, req.mod_id).await {
                                    Ok(mut files) => {
                                        let mut state = state_clone.write().await;
                                        if !files.is_empty() {
                                            // Sort: MAIN first, then UPDATE, OPTIONAL, OLD_VERSION
                                            files.sort_by(|a, b| {
                                                let order = |cat: &str| match cat {
                                                    "MAIN" => 0,
                                                    "UPDATE" => 1,
                                                    "OPTIONAL" => 2,
                                                    "OLD_VERSION" => 3,
                                                    _ => 4,
                                                };
                                                order(&a.category).cmp(&order(&b.category))
                                            });

                                            state.browse_mod_files = files;
                                            state.selected_file_index = 0;
                                            state.showing_file_picker = true;
                                            state.download_context = Some(crate::app::state::DownloadContext {
                                                mod_id: req.mod_id,
                                                mod_name: req.name.clone(),
                                                game_domain: game_domain.clone(),
                                                game_id: game_id_numeric,
                                            });
                                            state.set_status(format!("Select file to download for {}", req.name));
                                        } else {
                                            state.set_status(format!("No files found for {}", req.name));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = state_clone.write().await;
                                        state.set_status(format!("Failed to fetch files: {}", e));
                                    }
                                }
                            });
                        }
                        return Ok(());
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        if state.show_confirm.is_some() {
            match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    let action = state.show_confirm.take().unwrap().on_confirm;
                    drop(state);
                    self.handle_confirm_action(app, action).await?;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    state.show_confirm = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Help overlay navigation (modal)
        if state.show_help {
            const HELP_PAGE_COUNT: usize = 8;
            match key {
                KeyCode::Esc | KeyCode::Char('?') => {
                    state.show_help = false;
                    state.help_page = 0;
                }
                KeyCode::Char('n') | KeyCode::Right | KeyCode::PageDown => {
                    state.help_page = (state.help_page + 1) % HELP_PAGE_COUNT;
                }
                KeyCode::Char('p') | KeyCode::Left | KeyCode::PageUp => {
                    state.help_page = if state.help_page == 0 {
                        HELP_PAGE_COUNT - 1
                    } else {
                        state.help_page - 1
                    };
                }
                _ => {}
            }
            return Ok(());
        }

        // Global keys
        match (key, modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _) => {
                state.should_quit = true;
            }
            (KeyCode::F(1), _) => {
                state.goto(Screen::Mods);
            }
            (KeyCode::F(2), _) => {
                state.goto(Screen::Plugins);
            }
            (KeyCode::F(3), _) => {
                state.goto(Screen::Profiles);
            }
            (KeyCode::F(4), _) => {
                state.goto(Screen::Settings);
            }
            (KeyCode::F(5), _) => {
                state.goto(Screen::Import);
            }
            (KeyCode::F(6), _) => {
                state.goto(Screen::DownloadQueue);
            }
            (KeyCode::F(7), _) => {
                state.goto(Screen::NexusCatalog);
            }
            (KeyCode::F(8), _) => {
                // Load saved modlists before navigating
                if let Some(game) = &state.active_game {
                    let game_id = game.id.clone();
                    drop(state);
                    if let Ok(lists) = app.db.get_modlists_for_game(&game_id) {
                        let mut state = app.state.write().await;
                        state.saved_modlists = lists;
                        state.selected_saved_modlist_index = 0;
                        state.modlist_editor_mode = crate::app::state::ModlistEditorMode::ListPicker;
                        state.modlist_picker_for_loading = false;
                        state.goto(Screen::ModlistEditor);
                    }
                    return Ok(());
                } else {
                    state.set_status_error("No game selected");
                }
            }
            (KeyCode::Char('?'), _) => {
                state.show_help = !state.show_help;
                if state.show_help {
                    state.help_page = 0;
                }
            }
            (KeyCode::Esc, _) => {
                if state.show_help {
                    state.show_help = false;
                    state.help_page = 0;
                } else {
                    state.go_back();
                }
            }
            (KeyCode::Char('g'), _) => {
                // Go to game selection
                state.goto(Screen::GameSelect);
            }
            // Screen-specific keys
            _ => {
                drop(state);
                self.handle_screen_key(app, key, modifiers).await?;
            }
        }

        Ok(())
    }

    /// Handle mouse events
    async fn handle_mouse(&self, app: &mut App, mouse: MouseEvent) -> Result<()> {
        let mut state = app.state.write().await;

        // Skip mouse handling when in input mode
        if state.input_mode != InputMode::Normal {
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                // Increment appropriate selected index based on current screen
                match state.current_screen {
                    Screen::Mods | Screen::Dashboard => {
                        let count = state.installed_mods.len();
                        if count > 0 && state.selected_mod_index < count - 1 {
                            state.selected_mod_index += 1;
                        }
                    }
                    Screen::Plugins => {
                        let count = state.plugins.len();
                        if count > 0 && state.selected_plugin_index < count - 1 {
                            state.selected_plugin_index += 1;
                        }
                    }
                    Screen::Profiles => {
                        let count = state.profiles.len();
                        if count > 0 && state.selected_profile_index < count - 1 {
                            state.selected_profile_index += 1;
                        }
                    }
                    Screen::Settings => {
                        // Settings has 17 items (0-16)
                        if state.selected_setting_index < 16 {
                            state.selected_setting_index += 1;
                        }
                    }
                    Screen::Browse => {
                        let count = state.browse_results.len();
                        if count > 0 && state.selected_browse_index < count - 1 {
                            state.selected_browse_index += 1;
                        }
                    }
                    Screen::GameSelect => {
                        // Games are on the App, not state; just increment
                        state.selected_game_index += 1;
                    }
                    Screen::ImportReview => {
                        let count = state.import_results.len();
                        if count > 0 && state.selected_import_index < count - 1 {
                            state.selected_import_index += 1;
                        }
                    }
                    Screen::DownloadQueue => {
                        let count = state.queue_entries.len();
                        if count > 0 && state.selected_queue_index < count - 1 {
                            state.selected_queue_index += 1;
                            state.selected_queue_alternative_index = 0;
                        }
                    }
                    Screen::NexusCatalog => {
                        let count = state.catalog_browse_results.len();
                        if count > 0 && state.selected_catalog_index < count - 1 {
                            state.selected_catalog_index += 1;
                        }
                    }
                    Screen::ModlistEditor => {
                        match state.modlist_editor_mode {
                            crate::app::state::ModlistEditorMode::ListPicker => {
                                let count = state.saved_modlists.len();
                                if count > 0 && state.selected_saved_modlist_index < count - 1 {
                                    state.selected_saved_modlist_index += 1;
                                }
                            }
                            crate::app::state::ModlistEditorMode::EntryEditor => {
                                let count = state.modlist_editor_entries.len();
                                if count > 0 && state.selected_modlist_editor_index < count - 1 {
                                    state.selected_modlist_editor_index += 1;
                                }
                            }
                        }
                    }
                    Screen::LoadOrder => {
                        let count = state.load_order_mods.len();
                        if count > 0 && state.load_order_index < count - 1 {
                            state.load_order_index += 1;
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollUp => {
                match state.current_screen {
                    Screen::Mods | Screen::Dashboard => {
                        if state.selected_mod_index > 0 {
                            state.selected_mod_index -= 1;
                        }
                    }
                    Screen::Plugins => {
                        if state.selected_plugin_index > 0 {
                            state.selected_plugin_index -= 1;
                        }
                    }
                    Screen::Profiles => {
                        if state.selected_profile_index > 0 {
                            state.selected_profile_index -= 1;
                        }
                    }
                    Screen::Settings => {
                        if state.selected_setting_index > 0 {
                            state.selected_setting_index -= 1;
                        }
                    }
                    Screen::Browse => {
                        if state.selected_browse_index > 0 {
                            state.selected_browse_index -= 1;
                        }
                    }
                    Screen::GameSelect => {
                        if state.selected_game_index > 0 {
                            state.selected_game_index -= 1;
                        }
                    }
                    Screen::ImportReview => {
                        if state.selected_import_index > 0 {
                            state.selected_import_index -= 1;
                        }
                    }
                    Screen::DownloadQueue => {
                        if state.selected_queue_index > 0 {
                            state.selected_queue_index -= 1;
                            state.selected_queue_alternative_index = 0;
                        }
                    }
                    Screen::NexusCatalog => {
                        if state.selected_catalog_index > 0 {
                            state.selected_catalog_index -= 1;
                        }
                    }
                    Screen::ModlistEditor => {
                        match state.modlist_editor_mode {
                            crate::app::state::ModlistEditorMode::ListPicker => {
                                if state.selected_saved_modlist_index > 0 {
                                    state.selected_saved_modlist_index -= 1;
                                }
                            }
                            crate::app::state::ModlistEditorMode::EntryEditor => {
                                if state.selected_modlist_editor_index > 0 {
                                    state.selected_modlist_editor_index -= 1;
                                }
                            }
                        }
                    }
                    Screen::LoadOrder => {
                        if state.load_order_index > 0 {
                            state.load_order_index -= 1;
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if click is on tab bar row (row 3, 0-indexed)
                // Tab bar is at row 3 (after 3-line header)
                if mouse.row == 3 {
                    // Map column position to tab index
                    // Tabs: "F1 Mods|F2 Plugins|F3 Profiles|F4 Settings|F5 Import|F6 Queue|F7 Catalog|F8 Modlists"
                    let col = mouse.column as usize;
                    let screen = if col < 8 {
                        Some(Screen::Mods)
                    } else if col < 19 {
                        Some(Screen::Plugins)
                    } else if col < 31 {
                        Some(Screen::Profiles)
                    } else if col < 43 {
                        Some(Screen::Settings)
                    } else if col < 53 {
                        Some(Screen::Import)
                    } else if col < 62 {
                        Some(Screen::DownloadQueue)
                    } else if col < 73 {
                        Some(Screen::NexusCatalog)
                    } else if col < 85 {
                        Some(Screen::ModlistEditor)
                    } else {
                        None
                    };

                    if let Some(target) = screen {
                        state.goto(target);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle screen-specific keys
    async fn handle_screen_key(
        &self,
        app: &mut App,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        let mut state = app.state.write().await;
        let screen = state.current_screen;

        // Handle file picker overlay (intercepts keys when active)
        if state.showing_file_picker {
            let file_count = state.browse_mod_files.len();
            match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.selected_file_index > 0 {
                        state.selected_file_index -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if file_count > 0 && state.selected_file_index < file_count - 1 {
                        state.selected_file_index += 1;
                    }
                }
                KeyCode::Esc => {
                    state.showing_file_picker = false;
                    state.browse_mod_files.clear();
                    state.download_context = None;
                }
                KeyCode::Enter => {
                    if let Some(file) = state.browse_mod_files.get(state.selected_file_index).cloned() {
                        if let Some(ctx) = state.download_context.clone() {
                            let file_name = file.file_name.clone();
                            state.showing_file_picker = false;
                            state.set_status(format!("Getting download link for {}...", file.name));

                            let nexus_clone = app.nexus.as_ref().unwrap().clone();
                            let state_clone = app.state.clone();
                            let mods_clone = app.mods.clone();
                            let config_clone = app.config.clone();

                            drop(state);

                            tokio::spawn(async move {
                                // Get download link via REST API
                                match nexus_clone.get_download_link(
                                    &ctx.game_domain,
                                    ctx.mod_id,
                                    file.file_id,
                                ).await {
                                    Ok(links) => {
                                        if let Some(link) = links.first() {
                                            // Set up download progress
                                            {
                                                let mut state = state_clone.write().await;
                                                state.download_progress = Some(crate::app::state::DownloadProgress {
                                                    file_name: file_name.clone(),
                                                    downloaded_bytes: 0,
                                                    total_bytes: file.size_bytes as u64,
                                                });
                                                state.set_status(format!("Downloading {}...", file_name));
                                            }

                                            // Download to temp file
                                            let download_dir = std::env::var("HOME")
                                                .map(|h| std::path::PathBuf::from(h).join(".cache/modsanity/downloads"))
                                                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/modsanity"));

                                            if let Err(e) = tokio::fs::create_dir_all(&download_dir).await {
                                                let mut state = state_clone.write().await;
                                                state.download_progress = None;
                                                state.set_status(format!("Failed to create download dir: {}", e));
                                                return;
                                            }

                                            let dest_path = download_dir.join(&file_name);
                                            let url = link.url.clone();
                                            let state_for_progress = state_clone.clone();

                                            match crate::nexus::NexusClient::download_file(
                                                &url,
                                                &dest_path,
                                                move |downloaded, total| {
                                                    // Update progress in a non-blocking way
                                                    let state_ref = state_for_progress.clone();
                                                    tokio::spawn(async move {
                                                        let mut state = state_ref.write().await;
                                                        if let Some(ref mut progress) = state.download_progress {
                                                            progress.downloaded_bytes = downloaded;
                                                            if total > 0 {
                                                                progress.total_bytes = total;
                                                            }
                                                        }
                                                    });
                                                },
                                            ).await {
                                                Ok(()) => {
                                                    // Download complete - save to default mods directory if configured
                                                    let config = config_clone.read().await;
                                                    if let Some(ref default_dir) = config.tui.default_mod_directory {
                                                        let expanded_dir = if default_dir.starts_with("~/") {
                                                            std::env::var("HOME")
                                                                .map(|h| format!("{}/{}", h, &default_dir[2..]))
                                                                .unwrap_or_else(|_| default_dir.clone())
                                                        } else {
                                                            default_dir.clone()
                                                        };

                                                        // Create directory if it doesn't exist
                                                        if let Err(e) = std::fs::create_dir_all(&expanded_dir) {
                                                            tracing::warn!("Failed to create default mods directory: {}", e);
                                                        } else {
                                                            // Copy archive to default directory
                                                            let archive_dest = std::path::Path::new(&expanded_dir).join(&file_name);
                                                            if let Err(e) = std::fs::copy(&dest_path, &archive_dest) {
                                                                tracing::warn!("Failed to copy archive to default directory: {}", e);
                                                            } else {
                                                                tracing::info!("Saved archive to: {}", archive_dest.display());
                                                            }
                                                        }
                                                    }
                                                    drop(config);

                                                    // Auto install
                                                    {
                                                        let mut state = state_clone.write().await;
                                                        state.download_progress = None;
                                                        state.set_status(format!("Downloaded! Installing {}...", file_name));
                                                    }

                                                    let game_id = ctx.game_domain.clone();
                                                    let game_id_for_install = match game_id.as_str() {
                                                        "skyrimspecialedition" => "skyrimse",
                                                        "fallout4" => "fallout4",
                                                        "starfield" => "starfield",
                                                        id => id,
                                                    };

                                                    match mods_clone.install_from_archive(
                                                        game_id_for_install,
                                                        dest_path.to_str().unwrap_or(""),
                                                        None,
                                                        Some(ctx.mod_id),
                                                        None, // file_id not tracked in download context yet
                                                    ).await {
                                                        Ok(crate::mods::InstallResult::Completed(installed)) => {
                                                            let mut state = state_clone.write().await;
                                                            state.set_status(format!(
                                                                "✓ Installed: {} (v{})",
                                                                installed.name, installed.version
                                                            ));
                                                        }
                                                        Ok(crate::mods::InstallResult::RequiresWizard(context)) => {
                                                            // Launch FOMOD wizard
                                                            use crate::mods::fomod::wizard::init_wizard_state;
                                                            use crate::app::state::{FomodWizardState, WizardPhase};

                                                            let wizard = init_wizard_state(&context.installer.config);
                                                            let wizard_state = FomodWizardState {
                                                                installer: context.installer.clone(),
                                                                wizard,
                                                                current_step: 0,
                                                                current_group: 0,
                                                                selected_option: 0,
                                                                validation_errors: Vec::new(),
                                                                mod_name: context.mod_name.clone(),
                                                                staging_path: context.staging_path.clone(),
                                                                preview_files: None,
                                                                phase: WizardPhase::Overview,
                                    existing_mod_id: None,
                                                            };

                                                            let mut state = state_clone.write().await;
                                                            state.fomod_wizard_state = Some(wizard_state);
                                                            state.goto(crate::app::state::Screen::FomodWizard);
                                                            state.set_status(format!("FOMOD installer detected for {}", context.mod_name));
                                                        }
                                                        Err(e) => {
                                                            let mut state = state_clone.write().await;
                                                            state.set_status(format!(
                                                                "Downloaded to {:?} but install failed: {}",
                                                                dest_path, e
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    let mut state = state_clone.write().await;
                                                    state.download_progress = None;
                                                    state.set_status(format!("Download failed: {}", e));
                                                }
                                            }
                                        } else {
                                            let mut state = state_clone.write().await;
                                            state.set_status("No download links available".to_string());
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = state_clone.write().await;
                                        state.browse_mod_files.clear();
                                        let err_msg = e.to_string();
                                        if err_msg.contains("Premium") || err_msg.contains("403") {
                                            state.set_status("⚠ Direct download requires Nexus Premium. Visit nexusmods.com to download manually, then use 'i' to install.".to_string());
                                        } else {
                                            state.set_status(format!("Download link error: {}", err_msg));
                                        }
                                    }
                                }
                            });

                            return Ok(());
                        }
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match screen {
            Screen::GameSelect => {
                let game_count = app.games.len();
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_game_index > 0 {
                            state.selected_game_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if game_count > 0 && state.selected_game_index < game_count - 1 {
                            state.selected_game_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(game) = app.games.get(state.selected_game_index).cloned() {
                            drop(state);
                            app.set_active_game(Some(game.clone())).await?;
                            self.reload_data(app).await?;
                            let mut state = app.state.write().await;
                            state.active_game = Some(game.clone());
                            state.set_status(format!("Selected: {}", game.name));
                            state.goto(Screen::Mods);
                        }
                    }
                    _ => {}
                }
            }

            Screen::Dashboard | Screen::Mods => {
                // Build filtered mod list based on active category filter and search query
                let search_lower = state.mod_search_query.to_lowercase();
                let filtered_mods: Vec<&crate::mods::InstalledMod> = state.installed_mods.iter()
                    .filter(|m| {
                        // Apply category filter
                        let category_match = if let Some(filter_id) = state.category_filter {
                            m.category_id == Some(filter_id)
                        } else {
                            true
                        };

                        // Apply search filter
                        let search_match = if search_lower.is_empty() {
                            true
                        } else {
                            m.name.to_lowercase().contains(&search_lower)
                        };

                        category_match && search_match
                    })
                    .collect();
                let mod_count = filtered_mods.len();
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_mod_index > 0 {
                            state.selected_mod_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if mod_count > 0 && state.selected_mod_index < mod_count - 1 {
                            state.selected_mod_index += 1;
                        }
                    }
                    KeyCode::PageUp => {
                        // Move up by 10 items (or to start)
                        if state.selected_mod_index >= 10 {
                            state.selected_mod_index -= 10;
                        } else {
                            state.selected_mod_index = 0;
                        }
                    }
                    KeyCode::PageDown => {
                        // Move down by 10 items (or to end)
                        if mod_count > 0 {
                            let new_index = state.selected_mod_index + 10;
                            state.selected_mod_index = new_index.min(mod_count - 1);
                        }
                    }
                    KeyCode::Char('r') => {
                        // Refresh mod list
                        drop(state);
                        self.refresh_mods(app).await?;
                        let mut state = app.state.write().await;
                        state.set_status("Mod list refreshed".to_string());
                        return Ok(());
                    }
                    KeyCode::Char(' ') | KeyCode::Char('e') => {
                        // Enable/disable selected mod
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            let name = m.name.clone();
                            let enabled = m.enabled;
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            drop(state);

                            if let Some(game_id) = game_id {
                                if enabled {
                                    app.mods.disable_mod(&game_id, &name).await?;
                                } else {
                                    app.mods.enable_mod(&game_id, &name).await?;
                                }
                                self.refresh_mods(app).await?;
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('a') => {
                        // Enable all mods
                        let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                        let names: Vec<String> = state
                            .installed_mods
                            .iter()
                            .filter(|m| !m.enabled)
                            .map(|m| m.name.clone())
                            .collect();
                        let count = names.len();
                        drop(state);
                        if let Some(game_id) = game_id {
                            for name in &names {
                                let _ = app.mods.enable_mod(&game_id, name).await;
                            }
                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.set_status(format!("Enabled {} mods", count));
                        }
                        return Ok(());
                    }
                    KeyCode::Char('n') => {
                        // Disable all mods
                        let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                        let names: Vec<String> = state
                            .installed_mods
                            .iter()
                            .filter(|m| m.enabled)
                            .map(|m| m.name.clone())
                            .collect();
                        let count = names.len();
                        drop(state);
                        if let Some(game_id) = game_id {
                            for name in &names {
                                let _ = app.mods.disable_mod(&game_id, name).await;
                            }
                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.set_status(format!("Disabled {} mods", count));
                        }
                        return Ok(());
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        // Delete selected mod
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            use crate::app::state::{ConfirmAction, ConfirmDialog};
                            state.show_confirm = Some(ConfirmDialog {
                                title: "Delete Mod".to_string(),
                                message: format!("Delete '{}'?", m.name),
                                confirm_text: "Delete".to_string(),
                                cancel_text: "Cancel".to_string(),
                                on_confirm: ConfirmAction::DeleteMod(m.name.clone()),
                            });
                        }
                    }
                    KeyCode::Char('i') => {
                        // Install mod from file
                        state.input_mode = InputMode::ModInstallPath;
                        state.input_buffer.clear();
                    }
                    KeyCode::Char('f') => {
                        tracing::info!("'f' key pressed - checking for FOMOD installer");
                        // Re-run FOMOD installer for selected mod
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            tracing::info!("Selected mod: {}, path: {:?}", m.name, m.install_path);
                            let mod_name = m.name.clone();
                            let mod_id = m.id;
                            let staging_path = m.install_path.clone();

                            // Check if FOMOD exists (including nested structures)
                            if crate::mods::fomod::has_fomod(&staging_path) {
                                drop(state);

                                // Load FOMOD installer
                                match crate::mods::fomod::FomodInstaller::load(&staging_path) {
                                    Ok(installer) => {
                                        if installer.requires_wizard() {
                                            // Initialize wizard
                                            use crate::mods::fomod::wizard::init_wizard_state;
                                            use crate::app::state::{FomodWizardState, WizardPhase};

                                            let wizard = init_wizard_state(&installer.config);

                                            // Try to load previous choices
                                            let profile_id = None; // TODO: Get current profile ID
                                            if let Ok(previous_plan) = app.db.get_fomod_choice(mod_id, profile_id) {
                                                if let Some((config_hash, _plan_json)) = previous_plan {
                                                    // Check if config is still valid
                                                    use crate::mods::fomod::persistence::FomodChoiceManager;
                                                    let manager = FomodChoiceManager::new(&app.db);
                                                    // TODO: Restore previous selections if valid
                                                    let _ = (config_hash, manager); // Suppress unused warnings for now
                                                }
                                            }

                                            let wizard_state = FomodWizardState {
                                                installer: installer.clone(),
                                                wizard,
                                                current_step: 0,
                                                current_group: 0,
                                                selected_option: 0,
                                                validation_errors: Vec::new(),
                                                mod_name: mod_name.clone(),
                                                staging_path: staging_path.clone(),
                                                preview_files: None,
                                                phase: WizardPhase::Overview,
                                                existing_mod_id: Some(mod_id),
                                            };

                                            let mut state = app.state.write().await;
                                            state.fomod_wizard_state = Some(wizard_state);
                                            state.goto(crate::app::state::Screen::FomodWizard);
                                            state.set_status(format!("Reconfiguring FOMOD for {}", mod_name));
                                        } else {
                                            let mut state = app.state.write().await;
                                            state.set_status(format!("{} has simple FOMOD (no wizard needed)", mod_name));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.set_status(format!("Failed to load FOMOD: {:#}", e));
                                    }
                                }
                            } else {
                                state.set_status(format!("{} does not have a FOMOD installer", mod_name));
                            }
                        }
                    }
                    KeyCode::Char('l') => {
                        // Load mods from downloads folder
                        state.input_mode = InputMode::ModInstallPath;
                        state.input_buffer = std::env::var("HOME")
                            .map(|h| format!("{}/Downloads/", h))
                            .unwrap_or_else(|_| "~/Downloads/".to_string());
                    }
                    KeyCode::Char('I') => {
                        // Bulk install from default directory
                        let config = app.config.read().await;
                        let default_dir = config.tui.default_mod_directory.clone();
                        drop(config);

                        if let Some(default_dir) = default_dir {
                            let expanded_path = if default_dir.starts_with("~/") {
                                std::env::var("HOME")
                                    .map(|h| format!("{}/{}", h, &default_dir[2..]))
                                    .unwrap_or_else(|_| default_dir.clone())
                            } else {
                                default_dir.clone()
                            };

                            // Clone app components needed for background task
                            let state_clone = app.state.clone();
                            let mods_clone = app.mods.clone();
                            let path_clone = expanded_path.clone();

                            drop(state);

                            // Spawn bulk install in background so UI can continue updating
                            tokio::spawn(async move {
                                if let Err(e) = Self::run_bulk_install(
                                    state_clone.clone(),
                                    mods_clone,
                                    &path_clone
                                ).await {
                                    let mut state = state_clone.write().await;
                                    state.set_status(format!("Bulk install error: {}", e));
                                    state.installation_progress = None;
                                }
                            });

                            return Ok(());
                        } else {
                            state.set_status("No default mod directory configured. Set it in Settings (F4)".to_string());
                        }
                    }
                    KeyCode::Char('R') => {
                        // Rescan mods directory and rebuild database
                        // IMPORTANT: Get game_id from state directly, don't call active_game() while holding the lock!
                        let game_id = match state.active_game.as_ref() {
                            Some(g) => g.id.clone(),
                            None => {
                                state.set_status("No active game selected".to_string());
                                return Ok(());
                            }
                        };

                        // Set status and immediately drop the lock to avoid deadlock
                        state.set_status("Rescanning mods directory (this may take a minute)...".to_string());
                        drop(state);

                        // Clone components AFTER dropping the lock
                        let state_clone = app.state.clone();
                        let mods_clone = app.mods.clone();
                        let game_id_clone = game_id.clone();

                        // Spawn rescan in background - NO progress callbacks to avoid deadlock
                        tokio::spawn(async move {
                            tracing::info!("Starting rescan for game: {}", game_id_clone);

                            match mods_clone.rescan_mods(&game_id_clone, None).await {
                                Ok(stats) => {
                                    tracing::info!(
                                        "Rescan complete: {} added, {} updated, {} unchanged, {} failed",
                                        stats.added, stats.updated, stats.unchanged, stats.failed
                                    );

                                    // Reload mod list
                                    match mods_clone.list_mods(&game_id_clone).await {
                                        Ok(updated_mods) => {
                                            let mut st = state_clone.write().await;
                                            st.installed_mods = updated_mods;
                                            st.set_status(format!(
                                                "✓ Rescan: {} added, {} updated, {} unchanged, {} failed",
                                                stats.added, stats.updated, stats.unchanged, stats.failed
                                            ));
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to reload mod list: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Rescan error: {}", e);
                                    let mut st = state_clone.write().await;
                                    st.set_status(format!("Rescan error: {}", e));
                                }
                            }
                        });

                        return Ok(());
                    }
                    KeyCode::Char('C') => {
                        // Load collection from file
                        state.input_mode = InputMode::CollectionPath;
                        state.input_buffer = String::from("./collection.json");
                    }
                    KeyCode::Char('b') => {
                        // Browse/search Nexus Mods
                        if app.nexus.is_some() {
                            state.goto(Screen::Browse);

                            // Auto-load top mods when entering the browse screen for the first time
                            if state.browse_results.is_empty() && !state.browsing {
                                state.browsing = true;
                                state.browse_showing_default = true;
                                state.browse_query.clear();
                                state.browse_offset = 0;
                                if state.browse_limit <= 0 {
                                    state.browse_limit = 50;
                                }
                                state.browse_total_count = 0;
                                state.browse_sort = crate::nexus::graphql::SortBy::Downloads;
                                state.set_status("Loading top mods...".to_string());

                                let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                                let nexus_clone = app.nexus.as_ref().unwrap().clone();
                                let state_clone = app.state.clone();
                                let limit = state.browse_limit;

                                drop(state);

                                Self::spawn_browse_search(
                                    state_clone,
                                    nexus_clone,
                                    game_id,
                                    None,  // No query = top mods
                                    crate::nexus::graphql::SortBy::Downloads,
                                    0,
                                    limit,
                                );
                            }
                        } else {
                            state.set_status("Browse requires Nexus API key. Set it in Settings (F4)".to_string());
                        }
                    }
                    KeyCode::Char('o') => {
                        // Open Load Order screen
                        state.load_order_mods = state.installed_mods.clone();
                        state.load_order_index = state.selected_mod_index.min(
                            state.load_order_mods.len().saturating_sub(1),
                        );
                        state.load_order_dirty = false;
                        state.reorder_mode = false;
                        // Load conflicts
                        if let Some(ref game) = state.active_game {
                            if let Ok(conflicts) = crate::mods::get_conflicts_grouped(&app.db, &game.id) {
                                state.load_order_conflicts = conflicts;
                            }
                        }
                        state.goto(Screen::LoadOrder);
                    }
                    KeyCode::Char('U') => {
                        // Check for mod updates
                        if let Some(ref nexus) = app.nexus {
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            state.checking_updates = true;
                            state.set_status("Checking for mod updates...".to_string());
                            drop(state);

                            if let Some(game_id) = game_id {
                                let state_clone = app.state.clone();
                                let mods_clone = app.mods.clone();
                                let nexus_clone = nexus.clone();

                                // Spawn update check in background
                                tokio::spawn(async move {
                                    match mods_clone.check_for_updates(&game_id, &nexus_clone).await {
                                        Ok(updates) => {
                                            let mut state = state_clone.write().await;
                                            state.checking_updates = false;

                                            // Build updates map
                                            let mut updates_map = std::collections::HashMap::new();
                                            for update in &updates {
                                                updates_map.insert(update.mod_id, update.clone());
                                            }
                                            state.available_updates = updates_map;

                                            if updates.is_empty() {
                                                state.set_status("✓ All mods are up to date!".to_string());
                                            } else {
                                                state.set_status(format!(
                                                    "✨ {} mod update(s) available!",
                                                    updates.len()
                                                ));
                                            }
                                        }
                                        Err(e) => {
                                            let mut state = state_clone.write().await;
                                            state.checking_updates = false;
                                            state.set_status(format!("Update check failed: {}", e));
                                        }
                                    }
                                });
                            }
                            return Ok(());
                        } else {
                            state.set_status("Nexus API key not configured. Add it to ~/.config/modsanity/config.toml".to_string());
                        }
                    }
                    KeyCode::Char('D') => {
                        // Deploy
                        use crate::app::state::{ConfirmAction, ConfirmDialog};
                        state.show_confirm = Some(ConfirmDialog {
                            title: "Deploy Mods".to_string(),
                            message: "Deploy all enabled mods to game?".to_string(),
                            confirm_text: "Deploy".to_string(),
                            cancel_text: "Cancel".to_string(),
                            on_confirm: ConfirmAction::Deploy,
                        });
                    }
                    KeyCode::Enter => {
                        if !state.installed_mods.is_empty() {
                            state.goto(Screen::ModDetails);
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        // Increase priority (move up in load order)
                        // Higher priority = loads later = overwrites
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            let name = m.name.clone();
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            drop(state);
                            if let Some(game_id) = game_id {
                                match app.mods.change_priority(&game_id, &name, 1).await {
                                    Ok(new_priority) => {
                                        // Reload mods to reflect changes
                                        if let Ok(mods) = app.mods.list_mods(&game_id).await {
                                            let mut state = app.state.write().await;
                                            state.installed_mods = mods;
                                            state.set_status(format!(
                                                "Increased priority for {} to {}",
                                                name, new_priority
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.set_status(format!("Error: {}", e));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('-') => {
                        // Decrease priority
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            let name = m.name.clone();
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            drop(state);
                            if let Some(game_id) = game_id {
                                match app.mods.change_priority(&game_id, &name, -1).await {
                                    Ok(new_priority) => {
                                        // Reload mods to reflect changes
                                        if let Ok(mods) = app.mods.list_mods(&game_id).await {
                                            let mut state = app.state.write().await;
                                            state.installed_mods = mods;
                                            state.set_status(format!(
                                                "Decreased priority for {} to {}",
                                                name, new_priority
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.set_status(format!("Error: {}", e));
                                    }
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('/') => {
                        // Search mods by name
                        state.input_mode = InputMode::ModSearch;
                        state.input_buffer = state.mod_search_query.clone();
                    }
                    KeyCode::Char('s') => {
                        // Auto-sort by category
                        if let Some(game) = state.active_game.as_ref() {
                            let game_id = game.id.clone();
                            drop(state);

                            if let Err(e) = app.mods.auto_sort_by_category(&game_id).await {
                                let mut state = app.state.write().await;
                                state.set_status(format!("Error sorting: {}", e));
                            } else {
                                self.refresh_mods(app).await?;
                                let mut state = app.state.write().await;
                                state.set_status("Mods sorted by category order".to_string());
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('S') => {
                        // Save modlist
                        state.input_mode = InputMode::SaveModlistPath;
                        state.input_buffer = String::from("~/modlist.json");
                        state.modlist_save_format = "native".to_string();
                    }
                    KeyCode::Char('L') => {
                        // Load modlist (saved modlist picker first)
                        if let Some(game) = &state.active_game {
                            let game_id = game.id.clone();
                            drop(state);
                            match app.db.get_modlists_for_game(&game_id) {
                                Ok(lists) => {
                                    let mut state = app.state.write().await;
                                    state.saved_modlists = lists;
                                    state.selected_saved_modlist_index = 0;
                                    state.modlist_editor_mode = crate::app::state::ModlistEditorMode::ListPicker;
                                    state.modlist_picker_for_loading = true;
                                    state.goto(Screen::ModlistEditor);
                                    state.set_status_info("Select saved modlist to load, or press 'f' for file path");
                                }
                                Err(e) => {
                                    let mut state = app.state.write().await;
                                    state.set_status_error(format!("Failed to load saved modlists: {}", e));
                                }
                            }
                            return Ok(());
                        } else {
                            state.set_status_error("No game selected");
                        }
                    }
                    KeyCode::Left => {
                        // Navigate to previous category
                        if state.category_filter.is_none() {
                            // Currently showing all, can't go left
                        } else {
                            // Find current category index
                            if let Some(current_id) = state.category_filter {
                                if let Some(idx) = state.categories.iter().position(|c| c.id == Some(current_id)) {
                                    if idx == 0 {
                                        // Go to "All"
                                        state.category_filter = None;
                                    } else {
                                        // Go to previous category
                                        state.category_filter = state.categories.get(idx - 1).and_then(|c| c.id);
                                    }
                                    state.selected_mod_index = 0; // Reset selection
                                }
                            }
                        }
                    }
                    KeyCode::Right => {
                        // Navigate to next category
                        if state.category_filter.is_none() {
                            // Currently showing all, go to first category
                            state.category_filter = state.categories.first().and_then(|c| c.id);
                            state.selected_mod_index = 0; // Reset selection
                        } else {
                            // Find current category index
                            if let Some(current_id) = state.category_filter {
                                if let Some(idx) = state.categories.iter().position(|c| c.id == Some(current_id)) {
                                    // Go to next category if not at end
                                    if idx < state.categories.len() - 1 {
                                        state.category_filter = state.categories.get(idx + 1).and_then(|c| c.id);
                                        state.selected_mod_index = 0; // Reset selection
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        // Assign category to selected mod
                        if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                            let mod_id = m.id;
                            let categories = state.categories.clone();
                            drop(state);

                            // Simple category picker - cycle through categories
                            // For now, just assign the next category or None
                            let mod_rec = app.db.get_mods_for_game(&app.active_game().await.unwrap().id)?
                                .into_iter()
                                .find(|r| r.id == Some(mod_id));

                            if let Some(mod_rec) = mod_rec {
                                let next_category_id = if let Some(current_cat) = mod_rec.category_id {
                                    // Find next category
                                    if let Some(idx) = categories.iter().position(|c| c.id == Some(current_cat)) {
                                        if idx < categories.len() - 1 {
                                            categories.get(idx + 1).and_then(|c| c.id)
                                        } else {
                                            None // Cycle back to no category
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    // No category, assign first one
                                    categories.first().and_then(|c| c.id)
                                };

                                app.db.update_mod_category(mod_id, next_category_id)?;
                                self.refresh_mods(app).await?;

                                let mut state = app.state.write().await;
                                let cat_name = if let Some(cat_id) = next_category_id {
                                    categories.iter()
                                        .find(|c| c.id == Some(cat_id))
                                        .map(|c| c.name.as_str())
                                        .unwrap_or("Unknown")
                                } else {
                                    "None"
                                };
                                state.set_status(format!("Assigned category: {}", cat_name));
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('A') if modifiers.contains(KeyModifiers::SHIFT) => {
                        // Force recategorize ALL mods (even already categorized)
                        if let Some(game) = &state.active_game {
                            let game_id = game.id.clone();
                            drop(state);

                            // Get ALL mods (no filter)
                            let mods_to_categorize: Vec<_> = app.db.get_mods_for_game(&game_id)?;

                            let total = mods_to_categorize.len();
                            let mut categorized = 0;

                            // Process each mod with progress feedback
                            for (idx, mod_record) in mods_to_categorize.iter().enumerate() {
                                // Clear existing category first
                                app.db.update_mod_category(mod_record.id.unwrap(), None)?;

                                // Update progress
                                {
                                    let mut state = app.state.write().await;
                                    state.categorization_progress = Some(crate::app::state::CategorizationProgress {
                                        current_index: idx + 1,
                                        total_mods: total,
                                        current_mod_name: mod_record.name.clone(),
                                        categorized_count: categorized,
                                    });
                                }

                                // Give UI time to render
                                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                                // Categorize this mod
                                let installed_mod: crate::mods::InstalledMod = mod_record.clone().into();
                                if crate::mods::auto_categorize_mod(&app.db, &installed_mod).await.is_ok() {
                                    categorized += 1;
                                }
                            }

                            // Clear progress and refresh
                            {
                                let mut state = app.state.write().await;
                                state.categorization_progress = None;
                            }

                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.set_status(format!(
                                "✓ Force recategorized {} of {} mod(s)",
                                categorized, total
                            ));
                            return Ok(());
                        }
                    }
                    KeyCode::Char('A') => {
                        // Auto-categorize only uncategorized mods
                        if let Some(game) = &state.active_game {
                            let game_id = game.id.clone();
                            drop(state);

                            // Get mods to categorize (only uncategorized)
                            let mods_to_categorize: Vec<_> = app.db.get_mods_for_game(&game_id)?
                                .into_iter()
                                .filter(|m| m.category_id.is_none())
                                .collect();

                            let total = mods_to_categorize.len();
                            let mut categorized = 0;

                            // Process each mod with progress feedback
                            for (idx, mod_record) in mods_to_categorize.iter().enumerate() {
                                // Update progress
                                {
                                    let mut state = app.state.write().await;
                                    state.categorization_progress = Some(crate::app::state::CategorizationProgress {
                                        current_index: idx + 1,
                                        total_mods: total,
                                        current_mod_name: mod_record.name.clone(),
                                        categorized_count: categorized,
                                    });
                                }

                                // Give UI time to render
                                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                                // Categorize this mod
                                let installed_mod: crate::mods::InstalledMod = mod_record.clone().into();
                                if crate::mods::auto_categorize_mod(&app.db, &installed_mod).await.is_ok() {
                                    categorized += 1;
                                }
                            }

                            // Clear progress and refresh
                            {
                                let mut state = app.state.write().await;
                                state.categorization_progress = None;
                            }

                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.set_status(format!(
                                "✓ Auto-categorized {} of {} mod(s)",
                                categorized, total
                            ));
                            return Ok(());
                        }
                    }
                    KeyCode::Char('x') => {
                        // Check requirements for selected mod
                        if let Some(ref nexus) = app.nexus {
                            if let Some(&m) = filtered_mods.get(state.selected_mod_index) {
                                if let Some(mod_id) = m.nexus_mod_id {
                                    let mod_name = m.name.clone();
                                    let game_info = state.active_game.as_ref().map(|g| {
                                        (g.id.clone(), g.nexus_game_id.clone(), g.game_type.nexus_numeric_id())
                                    });
                                    state.set_status(format!("Checking requirements for {}...", mod_name));
                                    drop(state);

                                    if let Some((game_id, game_domain, game_id_numeric)) = game_info {
                                        let state_clone = app.state.clone();
                                        let mods_clone = app.mods.clone();
                                        let nexus_clone = nexus.clone();

                                        // Check requirements in background
                                        tokio::spawn(async move {
                                            match mods_clone.check_nexus_requirements(&game_id, mod_id, &nexus_clone).await {
                                                Ok((missing, dlcs, installed_count)) => {
                                                    let mut state = state_clone.write().await;

                                                    use crate::app::state::RequirementsDialog;
                                                    state.show_requirements = Some(RequirementsDialog {
                                                        title: format!("Requirements for {}", mod_name),
                                                        mod_name: mod_name.clone(),
                                                        missing_mods: missing,
                                                        dlc_requirements: dlcs,
                                                        installed_count,
                                                        selected_index: 0,
                                                        game_domain,
                                                        game_id_numeric,
                                                    });
                                                }
                                                Err(e) => {
                                                    let mut state = state_clone.write().await;
                                                    state.set_status(format!("Failed to check requirements: {}", e));
                                                }
                                            }
                                        });
                                    }
                                    return Ok(());
                                } else {
                                    state.set_status("No Nexus ID for this mod - requirements check not available".to_string());
                                }
                            }
                        } else {
                            state.set_status("Nexus API key not configured. Add it to ~/.config/modsanity/config.toml".to_string());
                        }
                    }
                    KeyCode::Char('u') => {
                        // Update missing Nexus IDs from mod names
                        if let Some(game_id) = state.active_game.as_ref().map(|g| g.id.clone()) {
                            state.set_status("Updating missing Nexus IDs...".to_string());

                            // Get default mods directory from config
                            let config = app.config.read().await;
                            let archive_dir = config.tui.default_mod_directory.clone();
                            drop(config);
                            drop(state);

                            let state_clone = app.state.clone();
                            let mods_clone = app.mods.clone();

                            tokio::spawn(async move {
                                match mods_clone.update_missing_nexus_ids(&game_id, archive_dir.as_deref()).await {
                                    Ok(count) => {
                                        // Reload mods list to get updated Nexus IDs
                                        if let Ok(updated_mods) = mods_clone.list_mods(&game_id).await {
                                            let mut state = state_clone.write().await;
                                            state.installed_mods = updated_mods;
                                            if count > 0 {
                                                state.set_status(format!("✓ Updated Nexus IDs for {} mod(s) and reloaded list", count));
                                            } else {
                                                state.set_status("No mods needed updating (all have Nexus IDs or couldn't parse)".to_string());
                                            }
                                        } else {
                                            let mut state = state_clone.write().await;
                                            if count > 0 {
                                                state.set_status(format!("✓ Updated Nexus IDs for {} mod(s)", count));
                                            } else {
                                                state.set_status("No mods needed updating (all have Nexus IDs or couldn't parse)".to_string());
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let mut state = state_clone.write().await;
                                        state.set_status(format!("Failed to update Nexus IDs: {}", e));
                                    }
                                }
                            });
                        }
                    }
                    _ => {}
                }
            }

            Screen::Plugins => {
                // Filter plugins by search query
                let search_lower = state.plugin_search_query.to_lowercase();
                let filtered_plugins: Vec<&crate::plugins::PluginInfo> = state.plugins.iter()
                    .filter(|p| {
                        if search_lower.is_empty() {
                            true
                        } else {
                            p.filename.to_lowercase().contains(&search_lower)
                        }
                    })
                    .collect();
                let plugin_count = filtered_plugins.len();
                match key {
                    KeyCode::Esc => {
                        if state.plugin_reorder_mode {
                            // Exit reorder mode
                            state.plugin_reorder_mode = false;
                            state.set_status("Exited reorder mode");
                        }
                    }
                    KeyCode::Enter => {
                        // Toggle reorder mode
                        if plugin_count == 0 {
                            return Ok(());
                        }
                        state.plugin_reorder_mode = !state.plugin_reorder_mode;
                        if state.plugin_reorder_mode {
                            state.set_status("REORDER MODE: j/k to move plugin, Enter/Esc to stop");
                        } else {
                            state.set_status("Navigation mode");
                        }
                    }
                    KeyCode::Char('/') => {
                        // Search plugins by name
                        state.input_mode = InputMode::PluginSearch;
                        state.input_buffer = state.plugin_search_query.clone();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.plugin_reorder_mode {
                            // Move plugin up in load order
                            let idx = state.selected_plugin_index;
                            if idx > 0 {
                                state.plugins.swap(idx, idx - 1);
                                state.selected_plugin_index = idx - 1;
                                state.plugin_dirty = true;
                                // Update load_order for all affected plugins
                                for (i, p) in state.plugins.iter_mut().enumerate() {
                                    p.load_order = i;
                                }
                            }
                        } else if state.selected_plugin_index > 0 {
                            state.selected_plugin_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.plugin_reorder_mode {
                            // Move plugin down in load order
                            let idx = state.selected_plugin_index;
                            if idx + 1 < plugin_count {
                                state.plugins.swap(idx, idx + 1);
                                state.selected_plugin_index = idx + 1;
                                state.plugin_dirty = true;
                                // Update load_order for all affected plugins
                                for (i, p) in state.plugins.iter_mut().enumerate() {
                                    p.load_order = i;
                                }
                            }
                        } else if plugin_count > 0 && state.selected_plugin_index < plugin_count - 1 {
                            state.selected_plugin_index += 1;
                        }
                    }
                    KeyCode::Char('K') => {
                        // Move up 5 positions or jump to top
                        if state.plugin_reorder_mode {
                            for _ in 0..5 {
                                let idx = state.selected_plugin_index;
                                if idx > 0 {
                                    state.plugins.swap(idx, idx - 1);
                                    state.selected_plugin_index = idx - 1;
                                }
                            }
                            if state.selected_plugin_index < plugin_count {
                                state.plugin_dirty = true;
                                for (i, p) in state.plugins.iter_mut().enumerate() {
                                    p.load_order = i;
                                }
                            }
                        } else {
                            state.selected_plugin_index = state.selected_plugin_index.saturating_sub(5);
                        }
                    }
                    KeyCode::Char('J') => {
                        // Move down 5 positions or jump to bottom
                        if state.plugin_reorder_mode {
                            for _ in 0..5 {
                                let idx = state.selected_plugin_index;
                                if idx + 1 < plugin_count {
                                    state.plugins.swap(idx, idx + 1);
                                    state.selected_plugin_index = idx + 1;
                                }
                            }
                            if state.selected_plugin_index < plugin_count {
                                state.plugin_dirty = true;
                                for (i, p) in state.plugins.iter_mut().enumerate() {
                                    p.load_order = i;
                                }
                            }
                        } else {
                            let max = plugin_count.saturating_sub(1);
                            state.selected_plugin_index = (state.selected_plugin_index + 5).min(max);
                        }
                    }
                    KeyCode::Char('t') => {
                        // Move to top
                        if state.plugin_reorder_mode && !state.plugins.is_empty() {
                            let idx = state.selected_plugin_index;
                            let p = state.plugins.remove(idx);
                            state.plugins.insert(0, p);
                            state.selected_plugin_index = 0;
                            state.plugin_dirty = true;
                            for (i, p) in state.plugins.iter_mut().enumerate() {
                                p.load_order = i;
                            }
                        } else if !state.plugin_reorder_mode {
                            state.selected_plugin_index = 0;
                        }
                    }
                    KeyCode::Char('b') => {
                        // Move to bottom
                        if state.plugin_reorder_mode && !state.plugins.is_empty() {
                            let idx = state.selected_plugin_index;
                            let p = state.plugins.remove(idx);
                            state.plugins.push(p);
                            state.selected_plugin_index = state.plugins.len() - 1;
                            state.plugin_dirty = true;
                            for (i, p) in state.plugins.iter_mut().enumerate() {
                                p.load_order = i;
                            }
                        } else if !state.plugin_reorder_mode {
                            state.selected_plugin_index = plugin_count.saturating_sub(1);
                        }
                    }
                    KeyCode::Char('#') => {
                        // Go to specific position
                        if state.plugin_reorder_mode && !state.plugins.is_empty() {
                            state.input_mode = InputMode::PluginPositionInput;
                            state.input_buffer.clear();
                        }
                    }
                    KeyCode::PageUp => {
                        // Move up by 10 items (or to start)
                        if state.selected_plugin_index >= 10 {
                            state.selected_plugin_index -= 10;
                        } else {
                            state.selected_plugin_index = 0;
                        }
                    }
                    KeyCode::PageDown => {
                        // Move down by 10 items (or to end)
                        if plugin_count > 0 {
                            let new_index = state.selected_plugin_index + 10;
                            state.selected_plugin_index = new_index.min(plugin_count - 1);
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Char('e') => {
                        // Toggle plugin enabled state
                        let index = state.selected_plugin_index;
                        if let Some(p) = state.plugins.get_mut(index) {
                            let was_enabled = p.enabled;
                            p.enabled = !p.enabled;
                            let status = if p.enabled { "Enabled" } else { "Disabled" };
                            let filename = p.filename.clone();
                            let path = p.path.clone();
                            let _ = p;

                            // Warn if enabling a non-existent plugin
                            if !was_enabled && !path.exists() {
                                state.set_status(format!(
                                    "Warning: {} not found in Data folder. Deploy mods first!",
                                    filename
                                ));
                            } else {
                                state.set_status(format!("{}: {} (press 's' to save)", status, filename));
                            }
                        }
                    }
                    KeyCode::Char('a') => {
                        // Enable all plugins
                        let count = state.plugins.len();
                        for plugin in state.plugins.iter_mut() {
                            plugin.enabled = true;
                        }
                        state.set_status(format!("Enabled all {} plugins (press 's' to save)", count));
                    }
                    KeyCode::Char('n') => {
                        // Disable all plugins
                        let count = state.plugins.len();
                        for plugin in state.plugins.iter_mut() {
                            plugin.enabled = false;
                        }
                        state.set_status(format!("Disabled all {} plugins (press 's' to save)", count));
                    }
                    KeyCode::Char('s') => {
                        // Save plugin load order
                        if let Some(game) = &state.active_game {
                            let enabled: Vec<String> = state.plugins
                                .iter()
                                .filter(|p| p.enabled)
                                .map(|p| p.filename.clone())
                                .collect();
                            let all: Vec<String> = state.plugins
                                .iter()
                                .map(|p| p.filename.clone())
                                .collect();

                            // Check if any enabled plugins are missing from Data folder
                            let mut missing_plugins = Vec::new();
                            for plugin in &enabled {
                                let plugin_path = game.data_path.join(plugin);
                                if !plugin_path.exists() {
                                    missing_plugins.push(plugin.clone());
                                }
                            }

                            if !missing_plugins.is_empty() {
                                state.set_status(format!(
                                    "Warning: {} plugin(s) not found in Data folder. Deploy mods first! Missing: {}",
                                    missing_plugins.len(),
                                    missing_plugins.join(", ")
                                ));
                            } else if let Err(e) = plugins::write_plugins_txt(game, &enabled) {
                                state.set_status(format!("Error saving plugins.txt: {}", e));
                            } else if let Err(e) = plugins::write_loadorder_txt(game, &all) {
                                state.set_status(format!("Error saving loadorder.txt: {}", e));
                            } else {
                                let skse_note = if enabled.iter().any(|p| p.to_lowercase().contains("skyui")) {
                                    " NOTE: SkyUI requires SKSE - launch through skse64_loader!"
                                } else {
                                    ""
                                };
                                state.plugin_dirty = false;
                                state.set_status(format!("Saved {} enabled plugins.{}", enabled.len(), skse_note));
                            }
                        }
                    }
                    KeyCode::Char('S') => {
                        // Native Rust auto-sort (recommended)
                        if let Some(game) = &state.active_game {
                            let game_id = game.id.clone();
                            let mut plugins_to_sort = state.plugins.clone();
                            drop(state);

                            match plugins::loot::sort_plugins_native(&game_id, &mut plugins_to_sort) {
                                Ok(_) => {
                                    // Validation
                                    let issues = plugins::sort::validate_load_order(&plugins_to_sort, &game_id);

                                    let mut state = app.state.write().await;
                                    state.plugins = plugins_to_sort;

                                    if issues.is_empty() {
                                        state.set_status("Native auto-sort complete! Press 's' to save.".to_string());
                                    } else {
                                        state.set_status(format!(
                                            "Auto-sort complete with {} warnings. Press 's' to save.",
                                            issues.len()
                                        ));
                                    }
                                }
                                Err(e) => {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("Sort error: {}", e));
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('L') => {
                        // Run LOOT CLI (requires LOOT installation)
                        if let Some(game) = &state.active_game {
                            let game_clone = game.clone();
                            drop(state);

                            // Check if LOOT is available
                            if !plugins::loot::is_loot_available() {
                                let mut state = app.state.write().await;
                                state.set_status("LOOT CLI not installed. Use 'S' for native sort instead.".to_string());
                                return Ok(());
                            }

                            // Run LOOT
                            let mut state = app.state.write().await;
                            state.set_status("Running LOOT CLI... (this may take a moment)".to_string());
                            drop(state);

                            match plugins::loot::sort_plugins(&game_clone) {
                                Ok(_) => {
                                    // Reload plugins to reflect LOOT's changes
                                    if let Ok(plugins_list) = plugins::get_plugins(&game_clone) {
                                        let mut state = app.state.write().await;
                                        state.plugins = plugins_list;
                                        state.set_status("LOOT CLI sorting complete! Plugins reloaded.".to_string());
                                    }
                                }
                                Err(e) => {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("LOOT CLI error: {}", e));
                                }
                            }
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }

            Screen::Profiles => {
                let profile_count = state.profiles.len();
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_profile_index > 0 {
                            state.selected_profile_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if profile_count > 0 && state.selected_profile_index < profile_count - 1 {
                            state.selected_profile_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Switch to selected profile
                        if let Some(p) = state.profiles.get(state.selected_profile_index) {
                            let name = p.name.clone();
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            drop(state);

                            if let Some(game_id) = game_id {
                                if let Err(e) = app.profiles.switch_profile(&game_id, &name).await {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("Error: {}", e));
                                } else {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("Switched to profile: {}", name));
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Char('n') => {
                        // New profile
                        state.input_mode = InputMode::ProfileNameInput;
                        state.input_buffer.clear();
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        // Delete profile
                        if let Some(p) = state.profiles.get(state.selected_profile_index) {
                            use crate::app::state::{ConfirmAction, ConfirmDialog};
                            state.show_confirm = Some(ConfirmDialog {
                                title: "Delete Profile".to_string(),
                                message: format!("Delete profile '{}'?", p.name),
                                confirm_text: "Delete".to_string(),
                                cancel_text: "Cancel".to_string(),
                                on_confirm: ConfirmAction::DeleteProfile(p.name.clone()),
                            });
                        }
                    }
                    _ => {}
                }
            }

            Screen::Collection => {
                // Collection screen navigation
                if let Some(ref collection) = state.current_collection {
                    let mod_count = collection.mods.len();
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            if state.selected_collection_mod_index > 0 {
                                state.selected_collection_mod_index -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if mod_count > 0 && state.selected_collection_mod_index < mod_count - 1 {
                                state.selected_collection_mod_index += 1;
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            // Go back to mods screen
                            state.goto(Screen::Mods);
                        }
                        _ => {}
                    }
                }
            }

            Screen::Settings => {
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_setting_index > 0 {
                            state.selected_setting_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.selected_setting_index < 16 {
                            state.selected_setting_index += 1;
                        }
                    }
                    KeyCode::Char('l') => {
                        if let Some(tool) = Self::settings_tool_for_index(state.selected_setting_index) {
                            state.set_status(format!("Launching {}...", tool.display_name()));
                            drop(state);
                            match app.launch_external_tool(tool, &[]).await {
                                Ok(code) => {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("{} exited with {}", tool.display_name(), code));
                                }
                                Err(e) => {
                                    let mut state = app.state.write().await;
                                    state.set_status(format!("Launch failed: {}", e));
                                }
                            }
                            return Ok(());
                        }
                    }
                    KeyCode::Enter => {
                        // Handle setting selection
                        match state.selected_setting_index {
                            0 => {
                                // NexusMods API Key setting
                                state.input_mode = InputMode::NexusApiKeyInput;
                                let config = app.config.read().await;
                                state.input_buffer = config.nexus_api_key
                                    .clone()
                                    .unwrap_or_default();
                            }
                            1 => {
                                // Cycle deployment method
                                {
                                    let mut config = app.config.write().await;
                                    config.deployment.method = match config.deployment.method {
                                        crate::config::DeploymentMethod::Symlink => crate::config::DeploymentMethod::Hardlink,
                                        crate::config::DeploymentMethod::Hardlink => crate::config::DeploymentMethod::Copy,
                                        crate::config::DeploymentMethod::Copy => crate::config::DeploymentMethod::Symlink,
                                    };
                                    if let Err(e) = config.save().await {
                                        state.set_status(format!("Error saving config: {}", e));
                                        return Ok(());
                                    }
                                    state.set_status(format!(
                                        "Deployment method: {}",
                                        config.deployment.method.display_name()
                                    ));
                                }
                            }
                            2 => {
                                // Toggle backup originals
                                {
                                    let mut config = app.config.write().await;
                                    config.deployment.backup_originals = !config.deployment.backup_originals;
                                    if let Err(e) = config.save().await {
                                        state.set_status(format!("Error saving config: {}", e));
                                        return Ok(());
                                    }
                                    state.set_status(format!(
                                        "Backup originals: {}",
                                        if config.deployment.backup_originals { "enabled" } else { "disabled" }
                                    ));
                                }
                            }
                            3 => {
                                // Downloads Directory setting
                                state.input_mode = InputMode::DownloadsDirectoryInput;
                                let config = app.config.read().await;
                                state.input_buffer = config.downloads_dir_override
                                    .clone()
                                    .unwrap_or_default();
                            }
                            4 => {
                                // Staging Directory setting
                                state.input_mode = InputMode::StagingDirectoryInput;
                                let config = app.config.read().await;
                                state.input_buffer = config.staging_dir_override
                                    .clone()
                                    .unwrap_or_default();
                            }
                            5 => {
                                // Default Mod Directory setting
                                state.input_mode = InputMode::ModDirectoryInput;
                                let config = app.config.read().await;
                                state.input_buffer = config.tui.default_mod_directory
                                    .clone()
                                    .unwrap_or_default();
                            }
                            6 => {
                                // Proton command
                                state.input_mode = InputMode::ProtonCommandInput;
                                let config = app.config.read().await;
                                state.input_buffer = config.external_tools.proton_command.clone();
                            }
                            9 | 10 | 11 | 12 | 13 | 14 | 15 => {
                                // Tool executable paths
                                let Some(tool) = Self::settings_tool_for_index(state.selected_setting_index) else {
                                    state.set_status("Invalid tool selection".to_string());
                                    return Ok(());
                                };
                                state.input_mode = InputMode::ExternalToolPathInput;
                                let config = app.config.read().await;
                                state.input_buffer = config
                                    .external_tool_path(tool)
                                    .unwrap_or("")
                                    .to_string();
                            }
                            7 => {
                                // Cycle Proton runtime (custom -> auto -> detected runtimes)
                                let runtimes = app.detect_proton_runtimes();
                                let mut options: Vec<Option<String>> = Vec::new();
                                options.push(None); // custom command/path
                                options.push(Some("auto".to_string()));
                                for runtime in runtimes {
                                    options.push(Some(runtime.id));
                                }

                                let current = {
                                    let config = app.config.read().await;
                                    config.external_tools.proton_runtime.clone()
                                };
                                let pos = options
                                    .iter()
                                    .position(|v| v.as_ref() == current.as_ref())
                                    .unwrap_or(0);
                                let next = (pos + 1) % options.len();

                                {
                                    let mut config = app.config.write().await;
                                    config.external_tools.proton_runtime = options[next].clone();
                                    if let Err(e) = config.save().await {
                                        state.set_status(format!("Error saving config: {}", e));
                                        return Ok(());
                                    }
                                    let label = config
                                        .external_tools
                                        .proton_runtime
                                        .clone()
                                        .unwrap_or_else(|| "Custom command/path".to_string());
                                    state.set_status(format!("Proton runtime: {}", label));
                                }
                            }
                            8 => {
                                // Toggle minimal color mode
                                {
                                    let mut config = app.config.write().await;
                                    config.tui.minimal_color_mode = !config.tui.minimal_color_mode;
                                    if let Err(e) = config.save().await {
                                        state.set_status(format!("Error saving config: {}", e));
                                        return Ok(());
                                    }
                                    state.set_status(format!(
                                        "Minimal color mode: {}",
                                        if config.tui.minimal_color_mode { "enabled" } else { "disabled" }
                                    ));
                                }
                            }
                            16 => {
                                // Game Selection
                                state.goto(Screen::GameSelect);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            Screen::Browse => {
                let result_count = state.browse_results.len();
                match key {
                    KeyCode::Char('s') => {
                        // Start search input
                        state.input_mode = InputMode::BrowseSearch;
                        state.input_buffer.clear();
                    }
                    KeyCode::Char('f') => {
                        // Cycle through sort options
                        use crate::nexus::graphql::SortBy;
                        state.browse_sort = match state.browse_sort {
                            SortBy::Relevance => SortBy::Downloads,
                            SortBy::Downloads => SortBy::Endorsements,
                            SortBy::Endorsements => SortBy::Updated,
                            SortBy::Updated => SortBy::Relevance,
                        };
                        let sort_mode = state.browse_sort;
                        state.set_status(format!("Sort: {:?}", sort_mode));

                        // Re-search with new sort if we have results (query or default content)
                        if (!state.browse_query.is_empty() || state.browse_showing_default) && app.nexus.is_some() {
                            let query = if state.browse_showing_default {
                                None
                            } else {
                                Some(state.browse_query.clone())
                            };
                            let sort = state.browse_sort;
                            let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                            let nexus_clone = app.nexus.as_ref().unwrap().clone();
                            let state_clone = app.state.clone();

                            state.browsing = true;
                            state.browse_offset = 0;
                            if state.browse_limit <= 0 {
                                state.browse_limit = 50;
                            }
                            let limit = state.browse_limit;
                            state.browse_total_count = 0;
                            drop(state);

                            Self::spawn_browse_search(
                                state_clone,
                                nexus_clone,
                                game_id,
                                query,
                                sort,
                                0,
                                limit,
                            );

                            return Ok(());
                        }
                    }
                    KeyCode::Char('n') | KeyCode::PageDown => {
                        if state.browsing {
                            return Ok(());
                        }
                        if state.browse_query.is_empty() && !state.browse_showing_default {
                            state.set_status("Search first to browse pages".to_string());
                            return Ok(());
                        }
                        if app.nexus.is_none() {
                            state.set_status("Browse requires Nexus API key".to_string());
                            return Ok(());
                        }
                        if state.browse_limit <= 0 {
                            state.browse_limit = 50;
                        }

                        let total = state.browse_total_count;
                        let limit = state.browse_limit;
                        let next_offset = state.browse_offset + limit;
                        if total <= 0 || next_offset as i64 >= total {
                            state.set_status("Already at last page".to_string());
                            return Ok(());
                        }

                        let query = if state.browse_showing_default {
                            None
                        } else {
                            Some(state.browse_query.clone())
                        };
                        let sort = state.browse_sort;
                        let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                        let nexus_clone = app.nexus.as_ref().unwrap().clone();
                        let state_clone = app.state.clone();

                        state.browsing = true;
                        state.set_status(format!(
                            "Loading page {}...",
                            (next_offset / limit) + 1
                        ));
                        drop(state);

                        Self::spawn_browse_search(
                            state_clone,
                            nexus_clone,
                            game_id,
                            query,
                            sort,
                            next_offset,
                            limit,
                        );
                        return Ok(());
                    }
                    KeyCode::Char('p') | KeyCode::PageUp => {
                        if state.browsing {
                            return Ok(());
                        }
                        if state.browse_query.is_empty() && !state.browse_showing_default {
                            state.set_status("Search first to browse pages".to_string());
                            return Ok(());
                        }
                        if app.nexus.is_none() {
                            state.set_status("Browse requires Nexus API key".to_string());
                            return Ok(());
                        }
                        if state.browse_limit <= 0 {
                            state.browse_limit = 50;
                        }

                        if state.browse_offset <= 0 {
                            state.set_status("Already at first page".to_string());
                            return Ok(());
                        }

                        let limit = state.browse_limit;
                        let prev_offset = (state.browse_offset - limit).max(0);
                        let query = if state.browse_showing_default {
                            None
                        } else {
                            Some(state.browse_query.clone())
                        };
                        let sort = state.browse_sort;
                        let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                        let nexus_clone = app.nexus.as_ref().unwrap().clone();
                        let state_clone = app.state.clone();

                        state.browsing = true;
                        state.set_status(format!(
                            "Loading page {}...",
                            (prev_offset / limit) + 1
                        ));
                        drop(state);

                        Self::spawn_browse_search(
                            state_clone,
                            nexus_clone,
                            game_id,
                            query,
                            sort,
                            prev_offset,
                            limit,
                        );
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_browse_index > 0 {
                            state.selected_browse_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if result_count > 0 && state.selected_browse_index < result_count - 1 {
                            state.selected_browse_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Fetch files for selected mod and show file picker
                        if let Some(result) = state.browse_results.get(state.selected_browse_index).cloned() {
                            if let Some(ref game) = state.active_game {
                                let game_numeric_id = game.game_type.nexus_numeric_id();
                                let game_domain = game.nexus_game_id.clone();
                                let game_id_numeric = game_numeric_id;
                                let mod_id = result.mod_id;
                                let mod_name = result.name.clone();

                                state.set_status(format!("Fetching files for {}...", mod_name));
                                state.download_context = Some(crate::app::state::DownloadContext {
                                    mod_id,
                                    mod_name: mod_name.clone(),
                                    game_domain: game_domain.clone(),
                                    game_id: game_id_numeric,
                                });
                                state.showing_file_picker = true;
                                state.selected_file_index = 0;
                                state.browse_mod_files.clear();

                                let nexus_clone = app.nexus.as_ref().unwrap().clone();
                                let state_clone = app.state.clone();

                                drop(state);

                                tokio::spawn(async move {
                                    match nexus_clone.get_mod_files(game_id_numeric, mod_id).await {
                                        Ok(mut files) => {
                                            // Sort: MAIN first, then UPDATE, OPTIONAL, OLD_VERSION
                                            files.sort_by(|a, b| {
                                                let order = |cat: &str| match cat {
                                                    "MAIN" => 0,
                                                    "UPDATE" => 1,
                                                    "OPTIONAL" => 2,
                                                    "MISCELLANEOUS" => 3,
                                                    "OLD_VERSION" => 4,
                                                    _ => 5,
                                                };
                                                order(&a.category).cmp(&order(&b.category))
                                            });

                                            let file_count = files.len();
                                            let mut state = state_clone.write().await;
                                            state.browse_mod_files = files;
                                            state.set_status(format!(
                                                "{} files available for {} - Select and press Enter to download",
                                                file_count, mod_name
                                            ));
                                        }
                                        Err(e) => {
                                            let mut state = state_clone.write().await;
                                            state.showing_file_picker = false;
                                            state.download_context = None;
                                            state.set_status(format!("Failed to get files: {}", e));
                                        }
                                    }
                                });

                                return Ok(());
                            }
                        }
                    }
                    _ => {}
                }
            }

            Screen::LoadOrder => {
                match key {
                    KeyCode::Esc => {
                        if state.reorder_mode {
                            // Exit reorder mode, not the screen
                            state.reorder_mode = false;
                            state.set_status("Exited reorder mode");
                        } else {
                            state.go_back();
                        }
                    }
                    KeyCode::Char('q') => {
                        if !state.reorder_mode {
                            state.go_back();
                        }
                    }
                    KeyCode::Enter => {
                        if state.load_order_mods.is_empty() {
                            return Ok(());
                        }
                        state.reorder_mode = !state.reorder_mode;
                        if state.reorder_mode {
                            state.set_status("REORDER MODE: j/k to move mod, Enter/Esc to stop");
                        } else {
                            state.set_status("Navigation mode");
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if state.load_order_mods.is_empty() {
                            return Ok(());
                        }
                        if state.reorder_mode {
                            let idx = state.load_order_index;
                            if idx + 1 < state.load_order_mods.len() {
                                state.load_order_mods.swap(idx, idx + 1);
                                state.load_order_index = idx + 1;
                                state.load_order_dirty = true;
                            }
                        } else if state.load_order_index + 1 < state.load_order_mods.len() {
                            state.load_order_index += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if state.reorder_mode {
                            let idx = state.load_order_index;
                            if idx > 0 {
                                state.load_order_mods.swap(idx, idx - 1);
                                state.load_order_index = idx - 1;
                                state.load_order_dirty = true;
                            }
                        } else if state.load_order_index > 0 {
                            state.load_order_index -= 1;
                        }
                    }
                    KeyCode::Char('J') => {
                        if state.load_order_mods.is_empty() {
                            return Ok(());
                        }
                        if state.reorder_mode {
                            for _ in 0..5 {
                                let idx = state.load_order_index;
                                if idx + 1 < state.load_order_mods.len() {
                                    state.load_order_mods.swap(idx, idx + 1);
                                    state.load_order_index = idx + 1;
                                }
                            }
                            state.load_order_dirty = true;
                        } else {
                            let max = state.load_order_mods.len().saturating_sub(1);
                            state.load_order_index = (state.load_order_index + 5).min(max);
                        }
                    }
                    KeyCode::Char('K') => {
                        if state.reorder_mode {
                            for _ in 0..5 {
                                let idx = state.load_order_index;
                                if idx > 0 {
                                    state.load_order_mods.swap(idx, idx - 1);
                                    state.load_order_index = idx - 1;
                                }
                            }
                            state.load_order_dirty = true;
                        } else {
                            state.load_order_index = state.load_order_index.saturating_sub(5);
                        }
                    }
                    KeyCode::Char('t') => {
                        if state.reorder_mode && !state.load_order_mods.is_empty() {
                            let idx = state.load_order_index;
                            let m = state.load_order_mods.remove(idx);
                            state.load_order_mods.insert(0, m);
                            state.load_order_index = 0;
                            state.load_order_dirty = true;
                        } else if !state.reorder_mode {
                            state.load_order_index = 0;
                        }
                    }
                    KeyCode::Char('b') => {
                        if state.reorder_mode && !state.load_order_mods.is_empty() {
                            let idx = state.load_order_index;
                            let m = state.load_order_mods.remove(idx);
                            state.load_order_mods.push(m);
                            state.load_order_index = state.load_order_mods.len() - 1;
                            state.load_order_dirty = true;
                        } else if !state.reorder_mode {
                            state.load_order_index = state.load_order_mods.len().saturating_sub(1);
                        }
                    }
                    KeyCode::Char('s') => {
                        // Save the current order
                        let order: Vec<(i64, i32)> = state
                            .load_order_mods
                            .iter()
                            .enumerate()
                            .map(|(i, m)| (m.id, i as i32))
                            .collect();
                        let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                        drop(state);

                        if let Err(e) = app.mods.save_priority_order(&order).await {
                            let mut state = app.state.write().await;
                            state.set_status(format!("Save error: {}", e));
                            return Ok(());
                        }

                        // Reload mods and conflicts
                        if let Some(ref gid) = game_id {
                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.load_order_mods = state.installed_mods.clone();
                            state.load_order_dirty = false;
                            if let Ok(conflicts) =
                                crate::mods::get_conflicts_grouped(&app.db, gid)
                            {
                                state.load_order_conflicts = conflicts;
                            }
                            state.set_status("Load order saved");
                        }
                        return Ok(());
                    }
                    KeyCode::Char('S') => {
                        // Auto-sort by category
                        if let Some(ref game) = state.active_game.clone() {
                            let game_id = game.id.clone();
                            drop(state);
                            if let Err(e) = app.mods.auto_sort_by_category(&game_id).await {
                                let mut state = app.state.write().await;
                                state.set_status(format!("Auto-sort error: {}", e));
                                return Ok(());
                            }
                            self.refresh_mods(app).await?;
                            let mut state = app.state.write().await;
                            state.load_order_mods = state.installed_mods.clone();
                            state.load_order_dirty = false;
                            if let Ok(conflicts) =
                                crate::mods::get_conflicts_grouped(&app.db, &game_id)
                            {
                                state.load_order_conflicts = conflicts;
                            }
                            state.set_status("Auto-sorted by category");
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }

            Screen::Import => {
                match key {
                    KeyCode::Char('i') => {
                        // Enter file path input mode
                        state.input_mode = InputMode::ImportFilePath;
                        state.input_buffer = state.import_file_path.clone();
                    }
                    KeyCode::Enter => {
                        // Start import
                        if !state.import_file_path.is_empty() {
                            let path = state.import_file_path.clone();
                            let game = state.active_game.clone();
                            let nexus = app.nexus.clone();
                            let state_clone = app.state.clone();
                            let db_clone = app.db.clone();

                            state.set_status("Importing modlist...");
                            drop(state);

                            if let Some(game) = game {
                                if let Some(nexus) = nexus {
                                    // Spawn import in background to avoid blocking UI
                                    tokio::spawn(async move {
                                        use crate::import::ModlistImporter;
                                        use crate::app::state::ImportProgress;

                                        let db_for_modlist = db_clone.clone();
                                        let importer = ModlistImporter::with_catalog(&game.id, (*nexus).clone(), Some(db_clone));

                                        // Progress callback to update UI
                                        let state_for_progress = state_clone.clone();
                                        let progress_callback = move |current: usize, total: usize, plugin_name: &str| {
                                            if let Ok(mut state) = state_for_progress.try_write() {
                                                state.import_progress = Some(ImportProgress {
                                                    current_index: current,
                                                    total_plugins: total,
                                                    current_plugin_name: plugin_name.to_string(),
                                                    stage: if current == 0 {
                                                        "Parsing".to_string()
                                                    } else {
                                                        "Matching".to_string()
                                                    },
                                                });
                                            }
                                        };

                                        match importer.import_modlist_with_progress(std::path::Path::new(&path), Some(progress_callback)).await {
                                            Ok(result) => {
                                                // Save modlist to DB for persistence
                                                let modlist_name = Self::modlist_name_from_path(&path, "Imported Modlist");
                                                let entries: Vec<crate::db::ModlistEntryRecord> = result.matches.iter().enumerate().map(|(i, m)| {
                                                    crate::db::ModlistEntryRecord {
                                                        id: None,
                                                        modlist_id: 0,
                                                        name: m.mod_name.clone(),
                                                        nexus_mod_id: m.best_match.as_ref().map(|bm| bm.mod_id),
                                                        plugin_name: Some(m.plugin.plugin_name.clone()),
                                                        match_confidence: Some(m.confidence.score()),
                                                        position: i as i32,
                                                        enabled: true,
                                                        author: m.best_match.as_ref().map(|bm| bm.author.clone()),
                                                        version: m.best_match.as_ref().map(|bm| bm.version.clone()),
                                                    }
                                                }).collect();

                                                if let Err(e) = db_for_modlist.upsert_modlist_with_entries(
                                                    &game.id,
                                                    &modlist_name,
                                                    None,
                                                    Some(&path),
                                                    &entries,
                                                ) {
                                                    tracing::warn!("Failed to persist imported modlist: {}", e);
                                                }

                                                let mut state = state_clone.write().await;
                                                let count = result.matches.len();
                                                state.import_results = result.matches;
                                                state.selected_import_index = 0;
                                                state.import_progress = None; // Clear progress
                                                state.goto(Screen::ImportReview);
                                                state.set_status(format!("Found {} plugins (saved to modlists)", count));
                                            }
                                            Err(e) => {
                                                let mut state = state_clone.write().await;
                                                state.import_progress = None; // Clear progress on error
                                                state.set_status(format!("Import failed: {}", e));
                                            }
                                        }
                                    });
                                } else {
                                    let mut state = app.state.write().await;
                                    state.set_status("NexusMods API key required");
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            Screen::ImportReview => {
                let result_count = state.import_results.len();
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_import_index > 0 {
                            state.selected_import_index -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if result_count > 0 && state.selected_import_index < result_count - 1 {
                            state.selected_import_index += 1;
                        }
                    }
                    KeyCode::Enter => {
                        // Create download queue
                        let results = state.import_results.clone();
                        let game = state.active_game.clone();
                        drop(state);

                        if let Some(game) = game {
                            use crate::queue::QueueManager;

                            let queue_manager = QueueManager::new(app.db.clone());
                            let batch_id = queue_manager.create_batch();

                            let mut queue_position = 0;
                            for result in &results {
                                let alternatives = result.alternatives.iter().map(|alt| {
                                    crate::queue::QueueAlternative {
                                        mod_id: alt.mod_id,
                                        name: alt.name.clone(),
                                        summary: alt.summary.clone(),
                                        downloads: alt.downloads,
                                        score: alt.score,
                                        thumbnail_url: None,
                                    }
                                }).collect();

                                let (mod_name, nexus_mod_id, status) = if let Some(best_match) = &result.best_match {
                                    (
                                        best_match.name.clone(),
                                        best_match.mod_id,
                                        if result.confidence.is_high() {
                                            crate::queue::QueueStatus::Matched
                                        } else {
                                            crate::queue::QueueStatus::NeedsReview
                                        }
                                    )
                                } else {
                                    // No match found - add entry with NeedsManual status
                                    (
                                        result.mod_name.clone(),
                                        0,
                                        crate::queue::QueueStatus::NeedsManual
                                    )
                                };

                                let entry = crate::queue::QueueEntry {
                                    id: 0,
                                    batch_id: batch_id.clone(),
                                    game_id: game.id.clone(),
                                    queue_position,
                                    plugin_name: result.plugin.plugin_name.clone(),
                                    mod_name,
                                    nexus_mod_id,
                                    selected_file_id: None,
                                    auto_install: true,
                                    match_confidence: Some(result.confidence.score()),
                                    alternatives,
                                    status,
                                    progress: 0.0,
                                    error: None,
                                };

                                if let Ok(_) = queue_manager.add_entry(entry) {
                                    queue_position += 1;
                                }
                            }

                            let mut state = app.state.write().await;
                            state.import_batch_id = Some(batch_id.clone());
                            state.goto(Screen::DownloadQueue);
                            state.selected_queue_index = 0;
                            state.selected_queue_alternative_index = 0;
                            state.queue_processing = false;
                            state.set_status(format!("Created queue with {} entries (batch: {})", queue_position, batch_id));

                            // Load queue entries
                            if let Ok(entries) = queue_manager.get_batch(&batch_id) {
                                state.queue_entries = entries;
                            }
                        }
                    }
                    _ => {}
                }
            }

            Screen::ModlistReview => {
                match key {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(review) = &state.modlist_review_data {
                            state.selected_modlist_entry =
                                (state.selected_modlist_entry + 1).min(review.needs_download.len().saturating_sub(1));
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        state.selected_modlist_entry = state.selected_modlist_entry.saturating_sub(1);
                    }
                    KeyCode::Enter => {
                        // Confirm and queue downloads
                        if let Some(review) = &state.modlist_review_data {
                            if review.needs_download.is_empty() {
                                state.set_status_success("All mods already installed!");
                                state.modlist_review_data = None;
                                state.go_back();
                            } else {
                                state.set_status("Queueing downloads...");
                                drop(state);
                                Self::spawn_queue_modlist_downloads(app.state.clone(), app.db.clone());
                            }
                        }
                    }
                    KeyCode::Esc => {
                        state.modlist_review_data = None;
                        state.go_back();
                    }
                    _ => {}
                }
            }

            Screen::DownloadQueue => {
                let entry_count = state.queue_entries.len();
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.selected_queue_index > 0 {
                            state.selected_queue_index -= 1;
                            state.selected_queue_alternative_index = 0;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if entry_count > 0 && state.selected_queue_index < entry_count - 1 {
                            state.selected_queue_index += 1;
                            state.selected_queue_alternative_index = 0;
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if let Some(entry) = state.queue_entries.get(state.selected_queue_index) {
                            let count = entry.alternatives.len();
                            if count > 0 {
                                if state.selected_queue_alternative_index > 0 {
                                    state.selected_queue_alternative_index -= 1;
                                } else {
                                    state.selected_queue_alternative_index = count - 1;
                                }
                            }
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if let Some(entry) = state.queue_entries.get(state.selected_queue_index) {
                            let count = entry.alternatives.len();
                            if count > 0 {
                                state.selected_queue_alternative_index =
                                    (state.selected_queue_alternative_index + 1) % count;
                            }
                        }
                    }
                    KeyCode::Char('m') => {
                        let selected = state.queue_entries.get(state.selected_queue_index).cloned();
                        let Some(entry) = selected else {
                            state.set_status("No queue entry selected");
                            return Ok(());
                        };

                        // Apply currently selected alternative when available.
                        let alt_idx = state.selected_queue_alternative_index;
                        if let Some(alternative) = entry.alternatives.get(alt_idx).cloned() {
                            let batch_id = state.import_batch_id.clone();
                            let selected_idx = state.selected_queue_index;
                            drop(state);

                            use crate::queue::{QueueManager, QueueStatus};
                            let queue_manager = QueueManager::new(app.db.clone());
                            if let Err(e) = queue_manager.resolve_entry(
                                entry.id,
                                alternative.mod_id,
                                &alternative.name,
                                QueueStatus::Matched,
                            ) {
                                let mut state = app.state.write().await;
                                state.set_status_error(format!("Failed to apply alternative: {}", e));
                                return Ok(());
                            }

                            if let Some(batch_id) = batch_id {
                                if let Ok(entries) = queue_manager.get_batch(&batch_id) {
                                    let mut state = app.state.write().await;
                                    state.queue_entries = entries;
                                    if !state.queue_entries.is_empty() {
                                        state.selected_queue_index = selected_idx.min(state.queue_entries.len() - 1);
                                    } else {
                                        state.selected_queue_index = 0;
                                    }
                                    state.selected_queue_alternative_index = 0;
                                    state.set_status_success(format!(
                                        "Resolved '{}' -> '{}'",
                                        entry.plugin_name, alternative.name
                                    ));
                                }
                            }
                            return Ok(());
                        }

                        // No alternatives: prompt manual Nexus ID entry.
                        state.input_mode = InputMode::QueueManualModIdInput;
                        state.input_buffer.clear();
                    }
                    KeyCode::Char('M') => {
                        state.input_mode = InputMode::QueueManualModIdInput;
                        state.input_buffer.clear();
                    }
                    KeyCode::Char('p') => {
                        // Process queue
                        if let Some(batch_id) = state.import_batch_id.clone() {
                            if let Some(game) = state.active_game.clone() {
                                if let Some(nexus) = app.nexus.clone() {
                                    state.queue_processing = true;
                                    drop(state);

                                    use crate::queue::QueueProcessor;
                                    let config = app.config.read().await;
                                    let download_dir = config.downloads_dir();
                                    drop(config);

                                    let processor = QueueProcessor::new(
                                        app.db.clone(),
                                        (*nexus).clone(),
                                        game.nexus_game_domain(),
                                        game.id.clone(),
                                        download_dir,
                                        app.mods.clone(),
                                    );
                                    let state_for_task = app.state.clone();
                                    let db_for_task = app.db.clone();
                                    let batch_for_task = batch_id.clone();

                                    tokio::spawn(async move {
                                        let result = processor.process_batch(&batch_for_task, false).await;
                                        let queue_manager = crate::queue::QueueManager::new(db_for_task);
                                        let refreshed = queue_manager.get_batch(&batch_for_task).unwrap_or_default();
                                        let mut state = state_for_task.write().await;
                                        state.queue_processing = false;
                                        state.queue_entries = refreshed;
                                        match result {
                                            Ok(_) => state.set_status_success("Queue processing complete"),
                                            Err(e) => {
                                                tracing::error!("Queue processing error: {}", e);
                                                state.set_status_error(format!("Queue processing failed: {}", e));
                                            }
                                        }
                                    });

                                    let mut state = app.state.write().await;
                                    state.set_status("Processing queue...");
                                } else {
                                    state.set_status("NexusMods API key required");
                                }
                            } else {
                                state.set_status("No active game selected");
                            }
                        } else {
                            state.set_status("No queue batch selected");
                        }
                    }
                    KeyCode::Char('c') => {
                        // Clear queue
                        if let Some(batch_id) = state.import_batch_id.clone() {
                            drop(state);

                            use crate::queue::QueueManager;
                            let queue_manager = QueueManager::new(app.db.clone());

                            if let Ok(_) = queue_manager.clear_batch(&batch_id) {
                                let mut state = app.state.write().await;
                                state.queue_entries.clear();
                                state.import_batch_id = None;
                                state.queue_processing = false;
                                state.set_status("Queue cleared");
                            }
                        } else {
                            state.set_status("No queue batch selected");
                        }
                    }
                    KeyCode::Char('r') => {
                        // Refresh queue
                        if let Some(batch_id) = state.import_batch_id.clone() {
                            drop(state);

                            use crate::queue::QueueManager;
                            let queue_manager = QueueManager::new(app.db.clone());

                            if let Ok(entries) = queue_manager.get_batch(&batch_id) {
                                let mut state = app.state.write().await;
                                state.queue_entries = entries;
                                state.set_status("Queue refreshed");
                            }
                        } else {
                            state.set_status("No queue batch selected");
                        }
                    }
                    _ => {}
                }
            }

            Screen::NexusCatalog => {
                drop(state);
                screens::nexus_catalog::handle_input(app, key).await?;
            }

            Screen::ModlistEditor => {
                use crate::app::state::ModlistEditorMode;

                match state.modlist_editor_mode {
                    ModlistEditorMode::ListPicker => {
                        let list_count = state.saved_modlists.len();
                        match key {
                            KeyCode::Down | KeyCode::Char('j') => {
                                if list_count > 0 && state.selected_saved_modlist_index < list_count - 1 {
                                    state.selected_saved_modlist_index += 1;
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if state.selected_saved_modlist_index > 0 {
                                    state.selected_saved_modlist_index -= 1;
                                }
                            }
                            KeyCode::Enter => {
                                // Open selected modlist (editor mode) or load it (load mode)
                                if let Some(ml) = state.saved_modlists.get(state.selected_saved_modlist_index) {
                                    let ml_id = ml.id.unwrap();
                                    let ml_name = ml.name.clone();
                                    let load_mode = state.modlist_picker_for_loading;
                                    drop(state);
                                    if load_mode {
                                        Self::spawn_load_saved_modlist(
                                            app.state.clone(),
                                            app.db.clone(),
                                            ml_id,
                                            ml_name,
                                        );
                                    } else if let Ok(entries) = app.db.get_modlist_entries(ml_id) {
                                        let mut state = app.state.write().await;
                                        state.modlist_editor_entries = entries;
                                        state.selected_modlist_editor_index = 0;
                                        state.active_modlist_id = Some(ml_id);
                                        state.modlist_editor_mode = ModlistEditorMode::EntryEditor;
                                    }
                                    return Ok(());
                                }
                            }
                            KeyCode::Char('f') => {
                                // Fallback to file-path modlist loading
                                state.modlist_picker_for_loading = false;
                                state.input_mode = InputMode::LoadModlistPath;
                                state.input_buffer = String::from("~/modlist.json");
                            }
                            KeyCode::Char('n') => {
                                // Create new modlist
                                state.input_mode = InputMode::ModlistNameInput;
                                state.input_buffer.clear();
                            }
                            KeyCode::Char('d') => {
                                // Delete selected modlist
                                if let Some(ml) = state.saved_modlists.get(state.selected_saved_modlist_index) {
                                    let ml_id = ml.id.unwrap();
                                    let ml_name = ml.name.clone();
                                    let game_id = state.active_game.as_ref().map(|g| g.id.clone());
                                    drop(state);
                                    if let Err(e) = app.db.delete_modlist(ml_id) {
                                        let mut state = app.state.write().await;
                                        state.set_status_error(format!("Delete failed: {}", e));
                                    } else if let Some(game_id) = game_id {
                                        if let Ok(lists) = app.db.get_modlists_for_game(&game_id) {
                                            let mut state = app.state.write().await;
                                            state.saved_modlists = lists;
                                            state.selected_saved_modlist_index = 0;
                                            state.set_status_success(format!("Deleted modlist: {}", ml_name));
                                        }
                                    }
                                    return Ok(());
                                }
                            }
                            KeyCode::Char('r') => {
                                // Rename selected modlist
                                if let Some(ml) = state.saved_modlists.get(state.selected_saved_modlist_index) {
                                    let name = ml.name.clone();
                                    let id = ml.id;
                                    state.input_mode = InputMode::ModlistNameInput;
                                    state.input_buffer = name;
                                    state.active_modlist_id = id;
                                }
                            }
                            KeyCode::Esc => {
                                state.modlist_picker_for_loading = false;
                                state.go_back();
                            }
                            _ => {}
                        }
                    }
                    ModlistEditorMode::EntryEditor => {
                        let entry_count = state.modlist_editor_entries.len();
                        match key {
                            KeyCode::Down | KeyCode::Char('j') => {
                                if entry_count > 0 && state.selected_modlist_editor_index < entry_count - 1 {
                                    state.selected_modlist_editor_index += 1;
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if state.selected_modlist_editor_index > 0 {
                                    state.selected_modlist_editor_index -= 1;
                                }
                            }
                            KeyCode::Char(' ') => {
                                // Toggle enabled
                                let idx = state.selected_modlist_editor_index;
                                if idx < state.modlist_editor_entries.len() {
                                    let entry = &state.modlist_editor_entries[idx];
                                    let entry_id = entry.id;
                                    let new_enabled = !entry.enabled;
                                    state.modlist_editor_entries[idx].enabled = new_enabled;
                                    if let Some(eid) = entry_id {
                                        drop(state);
                                        let _ = app.db.update_modlist_entry_enabled(eid, new_enabled);
                                        return Ok(());
                                    }
                                }
                            }
                            KeyCode::Char('d') => {
                                // Delete entry
                                if let Some(entry) = state.modlist_editor_entries.get(state.selected_modlist_editor_index) {
                                    if let Some(entry_id) = entry.id {
                                        let idx = state.selected_modlist_editor_index;
                                        drop(state);
                                        let _ = app.db.delete_modlist_entry(entry_id);
                                        let mut state = app.state.write().await;
                                        state.modlist_editor_entries.remove(idx);
                                        if state.selected_modlist_editor_index >= state.modlist_editor_entries.len() && state.selected_modlist_editor_index > 0 {
                                            state.selected_modlist_editor_index -= 1;
                                        }
                                        return Ok(());
                                    }
                                }
                            }
                            KeyCode::Char('J') => {
                                // Move entry down
                                if entry_count > 0 && state.selected_modlist_editor_index < entry_count - 1 {
                                    let idx = state.selected_modlist_editor_index;
                                    state.modlist_editor_entries.swap(idx, idx + 1);
                                    // Update positions in DB
                                    if let (Some(id_a), Some(id_b)) = (
                                        state.modlist_editor_entries[idx].id,
                                        state.modlist_editor_entries[idx + 1].id,
                                    ) {
                                        state.modlist_editor_entries[idx].position = idx as i32;
                                        state.modlist_editor_entries[idx + 1].position = (idx + 1) as i32;
                                        drop(state);
                                        let _ = app.db.update_modlist_entry_position(id_a, idx as i32);
                                        let _ = app.db.update_modlist_entry_position(id_b, (idx + 1) as i32);
                                        let mut state = app.state.write().await;
                                        state.selected_modlist_editor_index = idx + 1;
                                        return Ok(());
                                    }
                                    state.selected_modlist_editor_index = idx + 1;
                                }
                            }
                            KeyCode::Char('K') => {
                                // Move entry up
                                if state.selected_modlist_editor_index > 0 {
                                    let idx = state.selected_modlist_editor_index;
                                    state.modlist_editor_entries.swap(idx, idx - 1);
                                    // Update positions in DB
                                    if let (Some(id_a), Some(id_b)) = (
                                        state.modlist_editor_entries[idx].id,
                                        state.modlist_editor_entries[idx - 1].id,
                                    ) {
                                        state.modlist_editor_entries[idx].position = idx as i32;
                                        state.modlist_editor_entries[idx - 1].position = (idx - 1) as i32;
                                        drop(state);
                                        let _ = app.db.update_modlist_entry_position(id_a, idx as i32);
                                        let _ = app.db.update_modlist_entry_position(id_b, (idx - 1) as i32);
                                        let mut state = app.state.write().await;
                                        state.selected_modlist_editor_index = idx - 1;
                                        return Ok(());
                                    }
                                    state.selected_modlist_editor_index = idx - 1;
                                }
                            }
                            KeyCode::Esc => {
                                // Back to list picker
                                state.modlist_editor_mode = ModlistEditorMode::ListPicker;
                                state.modlist_editor_entries.clear();
                                state.active_modlist_id = None;
                            }
                            _ => {}
                        }
                    }
                }
            }

            Screen::FomodWizard => {
                use crate::app::state::WizardPhase;
                use crate::mods::fomod::validation::validate_group;

                let wizard_state = state.fomod_wizard_state.as_ref();
                if wizard_state.is_none() {
                    state.go_back();
                    return Ok(());
                }

                match key {
                    KeyCode::Esc => {
                        // Cancel wizard
                        state.fomod_wizard_state = None;
                        state.go_back();
                        state.set_status("FOMOD installation cancelled");
                    }
                    KeyCode::Char('q') => {
                        // Cancel wizard
                        state.fomod_wizard_state = None;
                        state.go_back();
                        state.set_status("FOMOD installation cancelled");
                    }
                    KeyCode::Char('?') => {
                        // Show help
                        state.set_status("j/k=navigate, Space=select, Enter=next, b=back, p=preview, Esc=cancel");
                    }
                    KeyCode::Enter => {
                        // Handle phase-specific enter action
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        match wizard_state.phase {
                            WizardPhase::Overview => {
                                wizard_state.phase = WizardPhase::StepNavigation;
                            }
                            WizardPhase::StepNavigation => {
                                let config = &wizard_state.installer.config;

                                // Validate current step
                                let current_step = wizard_state.current_step;
                                if current_step < config.install_steps.steps.len() {
                                    let step = &config.install_steps.steps[current_step];
                                    let mut all_valid = true;

                                    for (group_idx, group) in step.groups.groups.iter().enumerate() {
                                        let key = (current_step, group_idx);
                                        let selections = wizard_state.wizard.selections.get(&key)
                                            .cloned()
                                            .unwrap_or_default();

                                        if validate_group(group, &selections, current_step, group_idx).is_err() {
                                            all_valid = false;
                                            break;
                                        }
                                    }

                                    if !all_valid {
                                        state.set_status("Please complete all required selections");
                                        return Ok(());
                                    }
                                }

                                // Move to next step or summary
                                if wizard_state.current_step + 1 < config.install_steps.steps.len() {
                                    wizard_state.current_step += 1;
                                    wizard_state.current_group = 0;
                                    wizard_state.selected_option = 0;
                                } else {
                                    wizard_state.phase = WizardPhase::Summary;
                                }
                            }
                            WizardPhase::Summary => {
                                wizard_state.phase = WizardPhase::Confirm;
                            }
                            WizardPhase::Confirm => {
                                // Execute installation
                                let context = wizard_state.installer.clone();
                                let wizard = wizard_state.wizard.clone();
                                let staging_path = wizard_state.staging_path.clone();
                                let mod_name = wizard_state.mod_name.clone();
                                let existing_mod_id = wizard_state.existing_mod_id;

                                // Get nexus IDs from existing mod if reconfiguring
                                let (nexus_mod_id, nexus_file_id) = if let Some(mod_id) = existing_mod_id {
                                    if let Ok(Some(existing_mod)) = app.db.get_mod_by_id(mod_id) {
                                        (existing_mod.nexus_mod_id, existing_mod.nexus_file_id)
                                    } else {
                                        (None, None)
                                    }
                                } else {
                                    (None, None)
                                };

                                // Create context from wizard state
                                let fomod_context = crate::mods::FomodInstallContext {
                                    game_id: state.active_game.as_ref().unwrap().id.clone(),
                                    mod_name: mod_name.clone(),
                                    version: "1.0".to_string(), // TODO: Get actual version
                                    staging_path,
                                    installer: context,
                                    priority: 0, // TODO: Get actual priority
                                    existing_mod_id,
                                    nexus_mod_id,
                                    nexus_file_id,
                                };

                                state.fomod_wizard_state = None;
                                drop(state);

                                // Execute FOMOD installation
                                match app.mods.complete_fomod_install(&fomod_context, &wizard, None).await {
                                    Ok(installed) => {
                                        self.refresh_mods(app).await?;
                                        let mut state = app.state.write().await;
                                        state.goto(Screen::Mods);
                                        state.set_status(format!("Successfully installed: {}", installed.name));
                                    }
                                    Err(e) => {
                                        let mut state = app.state.write().await;
                                        state.goto(Screen::Mods);
                                        state.set_status(format!("Installation failed: {}", e));
                                    }
                                }
                                return Ok(());
                            }
                        }
                    }
                    KeyCode::Char('b') => {
                        // Go back
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        match wizard_state.phase {
                            WizardPhase::Overview => {
                                // Can't go back from overview
                            }
                            WizardPhase::StepNavigation => {
                                if wizard_state.current_step > 0 {
                                    wizard_state.current_step -= 1;
                                    wizard_state.current_group = 0;
                                    wizard_state.selected_option = 0;
                                } else {
                                    wizard_state.phase = WizardPhase::Overview;
                                }
                            }
                            WizardPhase::Summary => {
                                wizard_state.phase = WizardPhase::StepNavigation;
                                wizard_state.current_step = wizard_state.installer.config.install_steps.steps.len() - 1;
                            }
                            WizardPhase::Confirm => {
                                wizard_state.phase = WizardPhase::Summary;
                            }
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        // Navigate down
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        if let WizardPhase::StepNavigation = wizard_state.phase {
                            let config = &wizard_state.installer.config;
                            let current_step = wizard_state.current_step;

                            if current_step < config.install_steps.steps.len() {
                                let step = &config.install_steps.steps[current_step];
                                let current_group = wizard_state.current_group;

                                if current_group < step.groups.groups.len() {
                                    let group = &step.groups.groups[current_group];
                                    if wizard_state.selected_option + 1 < group.plugins.plugins.len() {
                                        wizard_state.selected_option += 1;
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        // Navigate up
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        if let WizardPhase::StepNavigation = wizard_state.phase {
                            if wizard_state.selected_option > 0 {
                                wizard_state.selected_option -= 1;
                            }
                        }
                    }
                    KeyCode::Tab => {
                        // Next group
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        if let WizardPhase::StepNavigation = wizard_state.phase {
                            let config = &wizard_state.installer.config;
                            let current_step = wizard_state.current_step;

                            if current_step < config.install_steps.steps.len() {
                                let step = &config.install_steps.steps[current_step];
                                if wizard_state.current_group + 1 < step.groups.groups.len() {
                                    wizard_state.current_group += 1;
                                    wizard_state.selected_option = 0;
                                }
                            }
                        }
                    }
                    KeyCode::BackTab => {
                        // Previous group
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        if let WizardPhase::StepNavigation = wizard_state.phase {
                            if wizard_state.current_group > 0 {
                                wizard_state.current_group -= 1;
                                wizard_state.selected_option = 0;
                            }
                        }
                    }
                    KeyCode::Char(' ') => {
                        // Toggle selection
                        let wizard_state = state.fomod_wizard_state.as_mut().unwrap();

                        if let WizardPhase::StepNavigation = wizard_state.phase {
                            let config = &wizard_state.installer.config;
                            let current_step = wizard_state.current_step;
                            let current_group = wizard_state.current_group;
                            let selected_option = wizard_state.selected_option;

                            if current_step < config.install_steps.steps.len() {
                                let step = &config.install_steps.steps[current_step];
                                if current_group < step.groups.groups.len() {
                                    let group = &step.groups.groups[current_group];
                                    let group_type = group.group_type.as_str();

                                    if selected_option < group.plugins.plugins.len() {
                                        let plugin = &group.plugins.plugins[selected_option];
                                        wizard_state.wizard.toggle_selection(
                                            current_step,
                                            current_group,
                                            selected_option,
                                            group_type,
                                            plugin,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('p') => {
                        // Preview (currently just show status)
                        let wizard_state = state.fomod_wizard_state.as_ref().unwrap();
                        let selection_count: usize = wizard_state.wizard.selections.values()
                            .map(|s| s.len())
                            .sum();
                        state.set_status(format!("{} options selected", selection_count));
                    }
                    _ => {}
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Handle confirmation actions
    async fn handle_confirm_action(
        &self,
        app: &mut App,
        action: crate::app::state::ConfirmAction,
    ) -> Result<()> {
        use crate::app::state::ConfirmAction;

        match action {
            ConfirmAction::DeleteMod(name) => {
                if let Some(game) = app.active_game().await {
                    app.mods.remove_mod(&game.id, &name).await?;
                    self.refresh_mods(app).await?;
                    // Note: Deployed files remain until next deploy/purge
                    let mut state = app.state.write().await;
                    state.set_status(format!("Deleted: {} (redeploy to update game files)", name));
                }
            }
            ConfirmAction::Deploy => {
                if let Some(game) = app.active_game().await {
                    // Check if there are any enabled mods
                    let enabled_count = {
                        let state = app.state.read().await;
                        state.installed_mods.iter().filter(|m| m.enabled).count()
                    };

                    let mut state = app.state.write().await;
                    if enabled_count == 0 {
                        state.set_status("No enabled mods - restoring game to factory state...");
                    } else {
                        state.set_status("Deploying mods...");
                    }
                    drop(state);

                    let stats = app.mods.deploy(&game).await?;

                    // Refresh plugins list to pick up newly deployed .esp/.esm/.esl files
                    self.refresh_plugins(app).await?;

                    let mut state = app.state.write().await;
                    if stats.mods_deployed == 0 {
                        state.set_status("✓ Game restored to factory state (all mod files removed)");
                    } else {
                        state.set_status(format!(
                            "Deployed {} files from {} mods",
                            stats.files_deployed, stats.mods_deployed
                        ));
                    }
                }
            }
            ConfirmAction::Purge => {
                if let Some(game) = app.active_game().await {
                    app.mods.purge(&game).await?;

                    // Refresh plugins list since purge removes all deployed files
                    self.refresh_plugins(app).await?;

                    let mut state = app.state.write().await;
                    state.set_status("Purged all deployed mods");
                }
            }
            ConfirmAction::DeleteProfile(name) => {
                if let Some(game) = app.active_game().await {
                    app.profiles.delete_profile(&game.id, &name).await?;
                    self.reload_data(app).await?;
                    let mut state = app.state.write().await;
                    state.set_status(format!("Deleted profile: {}", name));
                }
            }
            ConfirmAction::ClearQueue => {
                // Clear download queue
                let state = app.state.read().await;
                if let Some(batch_id) = state.import_batch_id.clone() {
                    drop(state);
                    let queue_manager = crate::queue::QueueManager::new(app.db.clone());
                    let _ = queue_manager.clear_batch(&batch_id);
                    let mut state = app.state.write().await;
                    state.queue_entries.clear();
                    state.import_batch_id = None;
                    state.set_status("Queue cleared");
                } else {
                    drop(state);
                    let mut state = app.state.write().await;
                    state.set_status("No active queue");
                }
            }
            ConfirmAction::LoadModlist(path) => {
                // This is handled in the load flow, so just acknowledge
                let mut state = app.state.write().await;
                state.set_status(format!("Loading modlist from {}", path));
            }
        }

        Ok(())
    }

    /// Refresh mods list
    async fn refresh_mods(&self, app: &mut App) -> Result<()> {
        if let Some(game) = app.active_game().await {
            let mods = app.mods.list_mods(&game.id).await?;
            let mut state = app.state.write().await;
            state.installed_mods = mods;
            if !state.installed_mods.is_empty() {
                state.selected_mod_index = state
                    .selected_mod_index
                    .min(state.installed_mods.len() - 1);
            } else {
                state.selected_mod_index = 0;
            }
        }
        Ok(())
    }

    async fn refresh_plugins(&self, app: &mut App) -> Result<()> {
        if let Some(game) = app.active_game().await {
            if let Ok(plugins_list) = plugins::get_plugins(&game) {
                let mut state = app.state.write().await;
                state.plugins = plugins_list;
                if !state.plugins.is_empty() {
                    state.selected_plugin_index = state
                        .selected_plugin_index
                        .min(state.plugins.len() - 1);
                } else {
                    state.selected_plugin_index = 0;
                }
            }
        }
        Ok(())
    }

    /// Install selected FOMOD components
    async fn install_fomod_components(
        &self,
        app: &mut App,
        archive_path: &str,
        components: &[crate::mods::fomod::FomodComponent],
        selected_indices: &[usize],
    ) -> Result<()> {
        if selected_indices.is_empty() {
            let mut state = app.state.write().await;
            state.set_status("No components selected");
            return Ok(());
        }

        // Get selected components
        let selected_components: Vec<_> = selected_indices
            .iter()
            .filter_map(|&i| components.get(i))
            .collect();

        if app.active_game().await.is_some() {
            let archive_path_obj = std::path::Path::new(archive_path);
            let mod_name = archive_path_obj
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown");

            {
                let mut state = app.state.write().await;
                state.set_status(format!("Installing {} components from {}...", selected_components.len(), mod_name));
            }

            // For now, we'll copy selected component files to a temporary staging directory
            // then install from there
            let temp_staging = std::env::temp_dir().join(format!("modsanity_fomod_{}", mod_name));
            if temp_staging.exists() {
                tokio::fs::remove_dir_all(&temp_staging).await?;
            }
            tokio::fs::create_dir_all(&temp_staging).await?;

            // Copy selected components
            for component in &selected_components {
                if let Ok(entries) = walkdir::WalkDir::new(&component.path).into_iter().collect::<std::result::Result<Vec<_>, _>>() {
                    for entry in entries {
                        if entry.file_type().is_file() {
                            if let Ok(rel_path) = entry.path().strip_prefix(&component.path) {
                                let dest = temp_staging.join(rel_path);
                                if let Some(parent) = dest.parent() {
                                    tokio::fs::create_dir_all(parent).await?;
                                }
                                tokio::fs::copy(entry.path(), &dest).await?;
                            }
                        }
                    }
                }
            }

            // For now, just notify user that FOMOD components were selected
            // The actual selective installation will be implemented in the next iteration
            let mut state = app.state.write().await;
            state.set_status(format!(
                "FOMOD support is work-in-progress. Selected {} components. For now, delete the mod and reinstall after manually extracting '00 Required' folder contents.",
                selected_components.len()
            ));

            // Clean up temp directory
            tokio::fs::remove_dir_all(&temp_staging).await.ok();
        }

        Ok(())
    }

    /// Run bulk install (static method for background task)
    async fn run_bulk_install(
        state: Arc<RwLock<AppState>>,
        mods: Arc<crate::mods::ModManager>,
        directory: &str,
    ) -> Result<()> {
        // Get active game
        let game_id = {
            let state = state.read().await;
            state.active_game.as_ref().map(|g| g.id.clone())
        };

        let game_id = match game_id {
            Some(id) => id,
            None => {
                let mut state = state.write().await;
                state.set_status("No active game selected".to_string());
                return Ok(());
            }
        };

        Self::bulk_install_from_directory_impl(state, mods, &game_id, directory).await
    }

    /// Bulk install all mod archives from a directory (implementation)
    async fn bulk_install_from_directory_impl(
        state: Arc<RwLock<AppState>>,
        mods: Arc<crate::mods::ModManager>,
        game_id: &str,
        directory: &str,
    ) -> Result<()> {
        let path = std::path::Path::new(directory);

        if !path.exists() {
            let mut st = state.write().await;
            st.set_status(format!("ERROR: Directory does not exist: {}", directory));
            return Ok(());
        }

        if !path.is_dir() {
            let mut st = state.write().await;
            st.set_status(format!("ERROR: Not a directory: {}", directory));
            return Ok(());
        }

        // Find all archive files
        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => {
                let mut st = state.write().await;
                st.set_status(format!("ERROR: Cannot read directory: {}", e));
                return Ok(());
            }
        };

        let archives: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                if let Some(ext) = e.path().extension() {
                    matches!(ext.to_str(), Some("zip" | "7z" | "rar"))
                } else {
                    false
                }
            })
            .collect();

        if archives.is_empty() {
            let mut st = state.write().await;
            st.set_status(format!("No mod archives (.zip, .7z, .rar) found in: {}", directory));
            return Ok(());
        }

        // Show starting message
        {
            let mut st = state.write().await;
            st.set_status(format!("Starting bulk install: {} archives found in {}", archives.len(), directory));
        }

        let total = archives.len();
        let mut installed = 0;
        let mut failed = 0;
        let mut skipped = 0;

        // Install each archive
        for (idx, entry) in archives.iter().enumerate() {
            let archive_path = entry.path();
            let filename = archive_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Create progress callback with bulk install context
            let state_clone = state.clone();
            let filename_clone = filename.to_string();
            let current_index = idx + 1;
            let total_mods = total;

            let progress_callback = std::sync::Arc::new(move |current_file: String, processed: usize, total_files: usize| {
                // Use try_write to avoid blocking within the async runtime
                if let Ok(mut st) = state_clone.try_write() {
                    let percent = if total_files > 0 {
                        ((processed as f64 / total_files as f64) * 100.0) as u16
                    } else {
                        0
                    };

                    st.installation_progress = Some(crate::app::state::InstallProgress {
                        percent,
                        current_file,
                        total_files,
                        processed_files: processed,
                        // Bulk install context
                        current_mod_name: Some(filename_clone.clone()),
                        current_mod_index: Some(current_index),
                        total_mods: Some(total_mods),
                    });
                }
            });

            // Install
            match mods.install_from_archive(
                game_id,
                archive_path.to_str().unwrap(),
                Some(progress_callback),
                None, // No Nexus ID for bulk installs
                None,
            ).await {
                Ok(crate::mods::InstallResult::Completed(installed_mod)) => {
                    installed += 1;
                    tracing::info!("[{}/{}] Installed: {}", idx + 1, total, installed_mod.name);

                    // Check for missing requirements
                    match mods.check_requirements(game_id, &installed_mod.name).await {
                        Ok(missing) if !missing.is_empty() => {
                            let req_list: Vec<String> = missing.iter()
                                .map(|(req, plugin)| format!("{} (required by {})", req, plugin))
                                .collect();
                            tracing::warn!(
                                "[{}/{}] {} installed but missing requirements: {}",
                                idx + 1, total, installed_mod.name, req_list.join(", ")
                            );

                            // Show warning briefly
                            {
                                let mut st = state.write().await;
                                if let Some(ref mut progress) = st.installation_progress {
                                    progress.percent = 100;
                                    progress.current_file = format!(
                                        "⚠ {}: Missing {} requirement(s)",
                                        installed_mod.name,
                                        missing.len()
                                    );
                                }
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                        }
                        Ok(_) => {
                            // No missing requirements - show success
                            {
                                let mut st = state.write().await;
                                if let Some(ref mut progress) = st.installation_progress {
                                    progress.percent = 100;
                                    progress.current_file = format!("✓ Completed: {}", installed_mod.name);
                                }
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        }
                        Err(e) => {
                            tracing::debug!("Could not check requirements: {}", e);
                            // Show success anyway
                            {
                                let mut st = state.write().await;
                                if let Some(ref mut progress) = st.installation_progress {
                                    progress.percent = 100;
                                    progress.current_file = format!("✓ Completed: {}", installed_mod.name);
                                }
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        }
                    }
                }
                Ok(crate::mods::InstallResult::RequiresWizard(_context)) => {
                    // Skip FOMOD wizards in bulk install
                    skipped += 1;
                    tracing::warn!("[{}/{}] Skipped: {} requires FOMOD wizard", idx + 1, total, filename);

                    {
                        let mut st = state.write().await;
                        if let Some(ref mut progress) = st.installation_progress {
                            progress.current_file = format!("⊘ Skipped: {} (needs wizard)", filename);
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    failed += 1;
                    let error_msg = format!("{}", e);
                    tracing::error!("[{}/{}] Failed to install {}: {}", idx + 1, total, filename, error_msg);

                    // Show error briefly before moving to next mod
                    {
                        let mut st = state.write().await;
                        if let Some(ref mut progress) = st.installation_progress {
                            progress.current_file = format!("✗ Failed: {}", error_msg);
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                }
            }
        }

        // Final cleanup and summary
        {
            let mut st = state.write().await;
            st.installation_progress = None;

            // Reload mods list
            if let Ok(updated_mods) = mods.list_mods(game_id).await {
                st.installed_mods = updated_mods;
            }

            let summary = if failed > 0 || skipped > 0 {
                format!(
                    "✓ Bulk install complete: {} installed, {} skipped, {} failed (check logs for details)",
                    installed, skipped, failed
                )
            } else {
                format!("✓ Bulk install complete: {} mods installed successfully!", installed)
            };

            st.set_status(summary);
        }

        Ok(())
    }

    /// Load and process a Nexus Mods collection
    async fn load_collection(&self, app: &mut App, path: &str) -> Result<()> {
        use crate::collections::load_collection;
        use std::path::Path;

        let path = Path::new(path);

        // Load collection
        let collection = match load_collection(path) {
            Ok(c) => c,
            Err(e) => {
                let mut state = app.state.write().await;
                state.set_status(format!("Failed to load collection: {}", e));
                return Ok(());
            }
        };

        // Get current game
        let game_id = {
            let state = app.state.read().await;
            state.active_game.as_ref().map(|g| g.id.clone())
        };

        let game_id = match game_id {
            Some(id) => id,
            None => {
                let mut state = app.state.write().await;
                state.set_status("No active game selected".to_string());
                return Ok(());
            }
        };

        // Get currently installed mods with nexus IDs
        let installed_mods: Vec<_> = match app.db.get_mods_for_game(&game_id) {
            Ok(mods) => mods,
            Err(_) => Vec::new(),
        };

        let installed_mod_ids: Vec<i64> = installed_mods.iter()
            .filter_map(|m| m.nexus_mod_id)
            .collect();

        // Build install status map
        let mut mod_status = std::collections::HashMap::new();
        for collection_mod in &collection.mods {
            let is_installed = installed_mod_ids.contains(&collection_mod.source.mod_id);
            mod_status.insert(collection_mod.source.mod_id, is_installed);
        }

        // Check which mods are installed
        let (installed_count, missing_required) = collection.check_installed(&installed_mod_ids);
        let stats = collection.stats();

        // Create a profile for this collection
        let profile_name = format!("{} Collection", collection.info.name);
        if let Err(e) = app.profiles.create_profile(&game_id, &profile_name).await {
            tracing::warn!("Failed to create profile for collection: {}", e);
        } else {
            tracing::info!("Created profile: {}", profile_name);
        }

        // Enable all installed mods that are in the collection
        let mut enabled_count = 0;
        for installed_mod in &installed_mods {
            if let Some(nexus_id) = installed_mod.nexus_mod_id {
                // Check if this mod is in the collection
                if collection.mods.iter().any(|cm| cm.source.mod_id == nexus_id) {
                    if let Err(e) = app.mods.enable_mod(&game_id, &installed_mod.name).await {
                        tracing::warn!("Failed to enable mod {}: {}", installed_mod.name, e);
                    } else {
                        enabled_count += 1;
                    }
                }
            }
        }

        // Store collection in state and navigate to collection screen
        {
            let mut state = app.state.write().await;
            state.current_collection = Some(collection.clone());
            state.collection_mod_status = mod_status;
            state.selected_collection_mod_index = 0;

            // Reload profiles to show the new one
            if let Ok(profiles) = app.profiles.list_profiles(&game_id).await {
                state.profiles = profiles;
            }

            state.set_status(format!(
                "Loaded collection '{}' | Profile created | {} mods enabled | {} / {} required mods installed",
                collection.info.name,
                enabled_count,
                installed_count,
                stats.required_mods
            ));

            // Navigate to collection screen
            state.goto(crate::app::state::Screen::Collection);
        }

        tracing::info!(
            "Loaded collection '{}': {}/{} mods installed, {} mods enabled, {} required mods missing",
            collection.info.name,
            installed_count,
            stats.total_mods,
            enabled_count,
            missing_required.len()
        );

        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
