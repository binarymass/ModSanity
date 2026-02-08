//! Main UI rendering

use super::screens;
use crate::app::{App, AppState, InputMode, Screen, UiMode};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};
use std::sync::atomic::{AtomicBool, Ordering};

static MINIMAL_COLOR_MODE: AtomicBool = AtomicBool::new(false);

fn minimal_color_mode() -> bool {
    MINIMAL_COLOR_MODE.load(Ordering::Relaxed)
}

fn set_minimal_color_mode(enabled: bool) {
    MINIMAL_COLOR_MODE.store(enabled, Ordering::Relaxed);
}

fn map_fg_color(color: Color) -> Color {
    if !minimal_color_mode() {
        return color;
    }
    match color {
        Color::Reset => Color::Reset,
        Color::Black => Color::Black,
        Color::DarkGray | Color::Gray => Color::Gray,
        Color::White => Color::White,
        _ => Color::White,
    }
}

fn map_bg_color(color: Color) -> Color {
    if !minimal_color_mode() {
        return color;
    }
    match color {
        Color::Reset => Color::Reset,
        _ => Color::Black,
    }
}

fn themed(style: Style) -> Style {
    if !minimal_color_mode() {
        return style;
    }
    let mut mapped = style;
    mapped.fg = mapped.fg.map(map_fg_color);
    mapped.bg = mapped.bg.map(map_bg_color);
    mapped
}

fn sfg(color: Color) -> Style {
    Style::default().fg(map_fg_color(color))
}

fn pipeline_step(screen: Screen) -> Option<usize> {
    match screen {
        Screen::Mods | Screen::Dashboard => Some(0),
        Screen::ModlistEditor => Some(1),
        Screen::Import | Screen::ImportReview | Screen::ModlistReview => Some(2),
        Screen::DownloadQueue => Some(3),
        _ => None,
    }
}

/// Draw the main UI
pub fn draw(f: &mut Frame, app: &App, state: &AppState) {
    let minimal_mode = app
        .config
        .try_read()
        .map(|c| c.tui.minimal_color_mode)
        .unwrap_or(false);
    set_minimal_color_mode(minimal_mode);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(1), // Tab bar
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer/status
        ])
        .split(f.area());

    draw_header(f, state, chunks[0]);
    draw_tabs(f, state, chunks[1]);
    draw_content(f, app, state, chunks[2]);
    draw_footer(f, state, chunks[3]);

    // Draw confirmation dialog if active
    if let Some(dialog) = &state.show_confirm {
        draw_confirm_dialog(f, dialog);
    }

    // Draw requirements dialog if active
    if let Some(dialog) = &state.show_requirements {
        draw_requirements_dialog(f, dialog);
    }

    // Draw help overlay if active
    if state.show_help {
        draw_help(f, state);
    }

    // Draw input overlays
    match state.input_mode {
        InputMode::ModInstallPath => draw_mod_install_input(f, state),
        InputMode::ProfileNameInput => draw_profile_name_input(f, state),
        InputMode::ModDirectoryInput => draw_mod_directory_input(f, state),
        InputMode::DownloadsDirectoryInput => draw_downloads_directory_input(f, state),
        InputMode::StagingDirectoryInput => draw_staging_directory_input(f, state),
        InputMode::ProtonCommandInput => draw_proton_command_input(f, state),
        InputMode::ExternalToolPathInput => draw_external_tool_path_input(f, state),
        InputMode::NexusApiKeyInput => draw_nexus_api_key_input(f, state),
        InputMode::FomodComponentSelection => draw_fomod_component_selection(f, state),
        InputMode::CollectionPath => draw_collection_input(f, state),
        InputMode::PluginPositionInput => draw_plugin_position_input(f, state),
        InputMode::ModSearch => draw_mod_search_input(f, state),
        InputMode::PluginSearch => draw_plugin_search_input(f, state),
        InputMode::ImportFilePath => draw_import_file_input(f, state),
        InputMode::SaveModlistPath => draw_save_modlist_input(f, state),
        InputMode::LoadModlistPath => draw_load_modlist_input(f, state),
        InputMode::CatalogSearch => draw_catalog_search_input(f, state),
        InputMode::ModlistNameInput => draw_modlist_name_input(f, state),
        InputMode::ModlistAddCatalogInput => draw_modlist_add_catalog_input(f, state),
        InputMode::ModlistAddDirectoryInput => draw_modlist_add_directory_input(f, state),
        InputMode::QueueManualModIdInput => draw_queue_manual_mod_id_input(f, state),
        _ => {}
    }

    // Draw installation progress if active
    if let Some(progress) = &state.installation_progress {
        draw_installation_progress(f, progress);
    }

    // Draw categorization progress if active
    if let Some(progress) = &state.categorization_progress {
        draw_categorization_progress(f, progress);
    }

    // Draw import progress if active
    if let Some(progress) = &state.import_progress {
        draw_import_progress(f, progress);
    }

    // Draw file picker overlay if active
    if state.showing_file_picker {
        draw_file_picker(f, state);
    }

    // Draw download progress if active
    if let Some(progress) = &state.download_progress {
        draw_download_progress(f, progress);
    }
}

/// Draw the header bar
fn draw_header(f: &mut Frame, state: &AppState, area: Rect) {
    let game_name = state
        .active_game
        .as_ref()
        .map(|g| g.name.as_str())
        .unwrap_or("No game selected");

    let mod_count = state.installed_mods.iter().filter(|m| m.enabled).count();
    let total_mods = state.installed_mods.len();

    // Note: We can't check nexus auth status here without async
    // Will show in settings screen instead
    let pipeline = if let Some(step) = pipeline_step(state.current_screen) {
        let labels = ["Mods", "Modlists", "Import", "Queue"];
        let mut rendered = String::new();
        for (i, label) in labels.iter().enumerate() {
            if i > 0 {
                rendered.push_str(" > ");
            }
            if i == step {
                rendered.push_str(&format!("[{}:{}]", i + 1, label));
            } else {
                rendered.push_str(&format!("{}:{}", i + 1, label));
            }
        }
        format!(" | Pipeline {}", rendered)
    } else {
        String::new()
    };

    let title = format!(
        " ModSanity v{}  |  {} | {}/{} mods enabled | {}{} ",
        crate::APP_VERSION,
        game_name,
        mod_count,
        total_mods,
        match state.ui_mode {
            UiMode::Guided => "Guided",
            UiMode::Advanced => "Advanced",
        },
        pipeline
    );

    let header = Paragraph::new(title)
        .style(themed(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(sfg(Color::Cyan)),
        );

    f.render_widget(header, area);
}

/// Draw the tab bar
fn draw_tabs(f: &mut Frame, state: &AppState, area: Rect) {
    let titles = vec!["F1 Mods", "F2 Plugins", "F3 Profiles", "F4 Settings", "F5 Import", "F6 Queue", "F7 Catalog", "F8 Modlists"];
    let selected = match state.current_screen {
        Screen::Dashboard | Screen::Mods | Screen::ModDetails => 0,
        Screen::Plugins => 1,
        Screen::Profiles => 2,
        Screen::Settings => 3,
        Screen::Import | Screen::ImportReview => 4,
        Screen::DownloadQueue => 5,
        Screen::NexusCatalog => 6,
        Screen::ModlistEditor => 7,
        Screen::GameSelect | Screen::FomodWizard | Screen::Collection | Screen::Browse | Screen::LoadOrder | Screen::ModlistReview => 0,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(sfg(Color::DarkGray))
        .highlight_style(
            themed(Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)),
        )
        .divider("|");

    f.render_widget(tabs, area);
}

/// Draw main content area
fn draw_content(f: &mut Frame, app: &App, state: &AppState, area: Rect) {
    match state.current_screen {
        Screen::GameSelect => draw_game_select(f, app, state, area),
        Screen::Dashboard | Screen::Mods => draw_mods_screen(f, state, area),
        Screen::ModDetails => draw_mod_details(f, state, area),
        Screen::Plugins => draw_plugins_screen(f, state, area),
        Screen::Profiles => draw_profiles_screen(f, state, area),
        Screen::Settings => draw_settings_screen(f, app, state, area),
        Screen::FomodWizard => screens::fomod_wizard::draw_fomod_wizard(f, state, area),
        Screen::Collection => draw_collection_screen(f, state, area),
        Screen::Browse => draw_browse_screen(f, state, area),
        Screen::LoadOrder => draw_load_order_screen(f, state, area),
        Screen::Import => draw_import_screen(f, state, area),
        Screen::ImportReview => draw_import_review_screen(f, state, area),
        Screen::DownloadQueue => draw_queue_screen(f, state, area),
        Screen::NexusCatalog => screens::nexus_catalog::render(f, area, state),
        Screen::ModlistReview => draw_modlist_review_screen(f, state, area),
        Screen::ModlistEditor => draw_modlist_editor_screen(f, state, area),
    }
}

/// Draw game selection screen
fn draw_game_select(f: &mut Frame, app: &App, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Game list
    let items: Vec<ListItem> = app
        .games
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let style = if i == state.selected_game_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!(" {} ({})", g.name, g.id)).style(style)
        })
        .collect();

    let title = if app.games.is_empty() {
        " No Games Found - Press 'q' to quit "
    } else {
        " Select a Game (Enter to confirm) "
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_game_index));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Game details
    if let Some(g) = app.games.get(state.selected_game_index) {
        let details = vec![
            Line::from(Span::styled(
                &g.name,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("ID:     {}", g.id)),
            Line::from(format!("Path:   {}", g.install_path.display())),
            Line::from(format!("Data:   {}", g.data_path.display())),
            Line::from(""),
            if let Some(prefix) = &g.proton_prefix {
                Line::from(format!("Proton: {}", prefix.display()))
            } else {
                Line::from("Proton: Not detected")
            },
        ];

        let details_widget = Paragraph::new(details)
            .block(Block::default().title(" Game Info ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(details_widget, chunks[1]);
    }
}

/// Draw the mods list screen
fn draw_mods_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let guided = state.ui_mode == UiMode::Guided;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if guided {
            vec![
                Constraint::Length(34), // Categories
                Constraint::Min(10),    // Mod list
            ]
        } else {
            vec![
                Constraint::Length(38),     // Categories sidebar
                Constraint::Percentage(65), // Mod list
                Constraint::Percentage(35), // Details
            ]
        })
        .split(area);

    // Draw categories sidebar
    draw_categories_sidebar(f, state, chunks[0]);

    // Filter mods by selected category and search query (moved to higher scope for use in details panel)
    let search_lower = state.mod_search_query.to_lowercase();
    let filtered_mods: Vec<(usize, &crate::mods::InstalledMod)> = state
        .installed_mods
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            // Apply category filter
            let category_match = if let Some(filter_id) = state.category_filter {
                m.category_id == Some(filter_id)
            } else {
                true // Show all if no category filter
            };

            // Apply search filter
            let search_match = if search_lower.is_empty() {
                true // Show all if no search query
            } else {
                m.name.to_lowercase().contains(&search_lower)
            };

            category_match && search_match
        })
        .collect();

    // Mod list
    if state.installed_mods.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from("No mods installed"),
            Line::from(""),
            Line::from("Press 'i' to install from file path"),
            Line::from("Press 'l' to load from Downloads folder"),
            Line::from("Press 'I' (capital) for bulk install from default directory"),
            Line::from(""),
            Line::from("Download mods from nexusmods.com manually"),
        ])
        .block(Block::default().title(" Installed Mods ").borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

        f.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = filtered_mods
            .iter()
            .enumerate()
            .map(|(display_i, (_, m))| {
                let status = if m.enabled { "[*]" } else { "[ ]" };
                let style = if display_i == state.selected_mod_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if !m.enabled {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };

                // Add category indicator if categorized
                let category_indicator = if let Some(cat_id) = m.category_id {
                    state.categories.iter()
                        .find(|c| c.id == Some(cat_id))
                        .map(|c| format!("[{}] ", &c.name[..c.name.len().min(3)]))
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                // Add update indicator if update is available
                let update_indicator = if let Some(nexus_id) = m.nexus_mod_id {
                    if state.available_updates.contains_key(&nexus_id) {
                        "✨ "
                    } else {
                        ""
                    }
                } else {
                    ""
                };

                ListItem::new(format!(" {} {}{}{} (v{})", status, category_indicator, update_indicator, m.name, m.version)).style(style)
            })
            .collect();

        let mut title = if let Some(filter_id) = state.category_filter {
            let cat_name = state.categories.iter()
                .find(|c| c.id == Some(filter_id))
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");
            format!(" Installed Mods - {} ({}) ", cat_name, filtered_mods.len())
        } else {
            format!(" Installed Mods ({}) ", filtered_mods.len())
        };

        // Add search indicator if searching
        if !state.mod_search_query.is_empty() {
            title = format!(" Installed Mods - Search: \"{}\" ({}) ", state.mod_search_query, filtered_mods.len());
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        // Use stateful rendering for proper scrolling
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_mod_index));

        f.render_stateful_widget(list, chunks[1], &mut list_state);
    }

    // Mod details panel (Advanced mode)
    if guided {
        return;
    }

    // Mod details panel
    // Get the mod from the filtered list, not the full list
    let selected_mod = filtered_mods.get(state.selected_mod_index).map(|(_, m)| *m);
    if let Some(m) = selected_mod {
        let mut details = vec![
            Line::from(Span::styled(
                &m.name,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Version:  {}", m.version)),
            Line::from(format!(
                "Status:   {}",
                if m.enabled { "Enabled" } else { "Disabled" }
            )),
            Line::from(format!("Priority: {}", m.priority)),
            Line::from(format!("Files:    {}", m.file_count)),
            Line::from(""),
            Line::from(format!(
                "Author:   {}",
                m.author.as_deref().unwrap_or("Unknown")
            )),
        ];

        // Add Nexus ID and update info
        if let Some(nexus_id) = m.nexus_mod_id {
            details.push(Line::from(format!("Nexus ID: {}", nexus_id)));

            // Show update info if available
            if let Some(update_info) = state.available_updates.get(&nexus_id) {
                details.push(Line::from(""));
                details.push(Line::from(Span::styled(
                    "✨ Update Available!",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
                details.push(Line::from(format!(
                    "Latest:   {} → {}",
                    m.version,
                    update_info.latest_version
                )));
                details.push(Line::from(format!(
                    "Updated:  {}",
                    update_info.updated_at.split('T').next().unwrap_or(&update_info.updated_at)
                )));
            }
        }

        let details_widget = Paragraph::new(details)
            .block(Block::default().title(" Mod Details ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(details_widget, chunks[2]);
    } else {
        let empty = Paragraph::new("No mod selected")
            .block(Block::default().title(" Mod Details ").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));

        f.render_widget(empty, chunks[2]);
    }
}

/// Draw mod details screen
fn draw_mod_details(f: &mut Frame, state: &AppState, area: Rect) {
    // Apply the same category and search filters as the mods screen
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

    let m = match filtered_mods.get(state.selected_mod_index) {
        Some(&m) => m,
        None => return,
    };

    let text = vec![
        Line::from(Span::styled(
            format!(" {} ", m.name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("  Version:  {}", m.version)),
        Line::from(format!(
            "  Status:   {}",
            if m.enabled { "Enabled" } else { "Disabled" }
        )),
        Line::from(format!("  Priority: {}", m.priority)),
        Line::from(format!("  Files:    {} files", m.file_count)),
        Line::from(""),
        Line::from(format!(
            "  Author:   {}",
            m.author.as_deref().unwrap_or("Unknown")
        )),
        Line::from(format!("  Path:     {}", m.install_path.display())),
    ];

    let details = Paragraph::new(text)
        .block(Block::default().title(" Mod Details ").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

    f.render_widget(details, area);
}

/// Draw plugins screen
fn draw_plugins_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Filter plugins by search query
    let search_lower = state.plugin_search_query.to_lowercase();
    let filtered_plugins: Vec<(usize, &crate::plugins::PluginInfo)> = state
        .plugins
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if search_lower.is_empty() {
                true
            } else {
                p.filename.to_lowercase().contains(&search_lower)
            }
        })
        .collect();

    if state.plugins.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from("No plugins found"),
            Line::from(""),
            Line::from("Install mods with .esp/.esm/.esl files"),
            Line::from("and deploy them to see plugins here."),
        ])
        .block(Block::default().title(" Load Order ").borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = filtered_plugins
            .iter()
            .enumerate()
            .map(|(display_i, (_, p))| {
                let status = if p.enabled { "[*]" } else { "[ ]" };
                let type_indicator = match p.plugin_type {
                    crate::plugins::PluginType::Master => "ESM",
                    crate::plugins::PluginType::Light => "ESL",
                    crate::plugins::PluginType::Plugin => "ESP",
                };

                let base_style = if display_i == state.selected_plugin_index && state.plugin_reorder_mode {
                    Style::default()
                        .bg(Color::Yellow)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                } else if display_i == state.selected_plugin_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if !p.enabled {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                };

                ListItem::new(format!(" {} [{}] {}", status, type_indicator, p.filename)).style(base_style)
            })
            .collect();

        let mode_indicator = if state.plugin_reorder_mode {
            " [REORDER MODE]"
        } else {
            ""
        };
        let dirty_indicator = if state.plugin_dirty {
            " (unsaved)"
        } else {
            ""
        };

        let mut title = format!(
            " Load Order ({}){}{}",
            filtered_plugins.len(),
            mode_indicator,
            dirty_indicator
        );

        // Add search indicator if searching
        if !state.plugin_search_query.is_empty() {
            title = format!(
                " Load Order - Search: \"{}\" ({}){}{}",
                state.plugin_search_query,
                filtered_plugins.len(),
                mode_indicator,
                dirty_indicator
            );
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        // Use stateful rendering for proper scrolling
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_plugin_index));

        f.render_stateful_widget(list, chunks[0], &mut list_state);
    }

    // Plugin details or help
    let selected_plugin = filtered_plugins.get(state.selected_plugin_index).map(|(_, p)| *p);
    if let Some(p) = selected_plugin {
        let masters_str = if p.masters.is_empty() {
            "None".to_string()
        } else {
            p.masters.join(", ")
        };

        let details = vec![
            Line::from(Span::styled(
                &p.filename,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!(
                "Type:    {}",
                match p.plugin_type {
                    crate::plugins::PluginType::Master => "Master (ESM)",
                    crate::plugins::PluginType::Light => "Light (ESL)",
                    crate::plugins::PluginType::Plugin => "Plugin (ESP)",
                }
            )),
            Line::from(format!(
                "Status:  {}",
                if p.enabled { "Enabled" } else { "Disabled" }
            )),
            Line::from(format!("Order:   {}", p.load_order)),
            Line::from(""),
            Line::from(format!("Masters: {}", masters_str)),
        ];

        let details_widget = Paragraph::new(details)
            .block(Block::default().title(" Plugin Details ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(details_widget, chunks[1]);
    } else {
        let help = Paragraph::new(vec![
            Line::from(""),
            Line::from("Plugin Management:"),
            Line::from(""),
            Line::from("  Enter    Toggle reorder mode"),
            Line::from("  j/k      Navigate (or move plugin)"),
            Line::from("  J/K      Jump 5 positions"),
            Line::from("  t/b      Move to top/bottom"),
            Line::from("  #        Go to specific position"),
            Line::from("  Space/e  Toggle enabled"),
            Line::from("  a        Enable all plugins"),
            Line::from("  n        Disable all plugins"),
            Line::from("  s        Save load order"),
            Line::from("  S        Auto-sort (native Rust)"),
            Line::from("  L        Auto-sort (LOOT CLI)"),
        ])
        .block(Block::default().title(" Help ").borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));

        f.render_widget(help, chunks[1]);
    }
}

/// Draw profiles screen
fn draw_profiles_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Profile list
    if state.profiles.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from("No profiles created"),
            Line::from(""),
            Line::from("Press 'n' to create a new profile"),
        ])
        .block(Block::default().title(" Profiles ").borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

        f.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = state
            .profiles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let style = if i == state.selected_profile_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(format!(" {}", p.name)).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(" Profiles ({}) ", state.profiles.len()))
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        // Use stateful rendering for proper scrolling
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_profile_index));

        f.render_stateful_widget(list, chunks[0], &mut list_state);
    }

    // Help panel
    let help = Paragraph::new(vec![
        Line::from(""),
        Line::from("Profile Management:"),
        Line::from(""),
        Line::from("  n        New profile"),
        Line::from("  Enter    Switch to profile"),
        Line::from("  d        Delete profile"),
        Line::from("  j/k      Navigate"),
        Line::from(""),
        Line::from("Profiles save your mod"),
        Line::from("configuration so you can"),
        Line::from("quickly switch between"),
        Line::from("different setups."),
    ])
    .block(Block::default().title(" Help ").borders(Borders::ALL))
    .style(Style::default().fg(Color::DarkGray));

    f.render_widget(help, chunks[1]);
}

/// Draw collection screen
fn draw_collection_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Collection mod list
    if let Some(ref collection) = state.current_collection {
        // Build list items with install status
        let items: Vec<ListItem> = collection
            .mods
            .iter()
            .enumerate()
            .map(|(i, mod_entry)| {
                let is_installed = state.collection_mod_status
                    .get(&mod_entry.source.mod_id)
                    .copied()
                    .unwrap_or(false);

                let status_icon = if is_installed { "✓" } else { "✗" };
                let status_color = if is_installed { Color::Green } else { Color::Red };

                let required_badge = if !mod_entry.optional { " [Required]" } else { "" };

                let mod_name = mod_entry.name
                    .split(" - ")
                    .next()
                    .unwrap_or(&mod_entry.name);

                let style = if i == state.selected_collection_mod_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(format!(" {} ", status_icon), Style::default().fg(status_color)),
                    Span::raw(mod_name),
                    Span::styled(required_badge, Style::default().fg(Color::Yellow)),
                ]);

                ListItem::new(line).style(style)
            })
            .collect();

        let stats = collection.stats();
        let installed_count = state.collection_mod_status.values().filter(|&&v| v).count();
        let missing_required: Vec<_> = collection.mods.iter()
            .filter(|m| !m.optional && !state.collection_mod_status.get(&m.source.mod_id).copied().unwrap_or(false))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(
                        " Collection Mods: {} / {} installed ({} required missing) ",
                        installed_count,
                        stats.total_mods,
                        missing_required.len()
                    ))
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_collection_mod_index));

        f.render_stateful_widget(list, chunks[0], &mut list_state);

        // Info panel
        let selected_mod = collection.mods.get(state.selected_collection_mod_index);

        let mut info_lines = vec![
            Line::from(Span::styled(
                collection.info.name.clone(),
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            )),
            Line::from(format!("By: {}", collection.info.author)),
            Line::from(""),
        ];

        if let Some(mod_entry) = selected_mod {
            let is_installed = state.collection_mod_status
                .get(&mod_entry.source.mod_id)
                .copied()
                .unwrap_or(false);

            info_lines.push(Line::from(Span::styled(
                "Selected Mod:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            info_lines.push(Line::from(""));
            info_lines.push(Line::from(format!("Name: {}", mod_entry.name)));
            info_lines.push(Line::from(format!("Author: {}", mod_entry.author)));
            info_lines.push(Line::from(format!("Version: {}", mod_entry.version)));
            info_lines.push(Line::from(format!("Mod ID: {}", mod_entry.source.mod_id)));
            info_lines.push(Line::from(format!("File ID: {}", mod_entry.source.file_id)));
            info_lines.push(Line::from(""));

            let status_text = if is_installed { "Installed ✓" } else { "Not Installed ✗" };
            let status_color = if is_installed { Color::Green } else { Color::Red };
            info_lines.push(Line::from(Span::styled(
                status_text,
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            )));

            if !mod_entry.optional {
                info_lines.push(Line::from(Span::styled(
                    "Required",
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        info_lines.push(Line::from(""));
        info_lines.push(Line::from(""));

        if !missing_required.is_empty() {
            info_lines.push(Line::from(Span::styled(
                format!("⚠ {} Required Mods Missing:", missing_required.len()),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            info_lines.push(Line::from(""));

            for mod_entry in missing_required.iter().take(5) {
                let mod_name = mod_entry.name
                    .split(" - ")
                    .next()
                    .unwrap_or(&mod_entry.name);
                info_lines.push(Line::from(format!("  • {}", mod_name)));
            }

            if missing_required.len() > 5 {
                info_lines.push(Line::from(format!("  ... and {} more", missing_required.len() - 5)));
            }
        }

        let info = Paragraph::new(info_lines)
            .block(Block::default().title(" Collection Info ").borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(info, chunks[1]);
    } else {
        // No collection loaded
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from("No collection loaded"),
            Line::from(""),
            Line::from("Press 'q' or Esc to go back"),
        ])
        .block(Block::default().title(" Collection ").borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

        f.render_widget(empty, area);
    }
}

/// Draw settings screen
fn draw_settings_screen(f: &mut Frame, app: &App, state: &AppState, area: Rect) {
    // Try to get config without blocking - this is a workaround for sync context
    let (
        mod_dir_display,
        downloads_dir_display,
        staging_dir_display,
        proton_cmd_display,
        proton_runtime_display,
        minimal_color_display,
        xedit_display,
        ssedit_display,
        fnis_display,
        nemesis_display,
        symphony_display,
        bodyslide_display,
        outfit_display,
        api_key_display,
        deployment_method_display,
        backup_display,
    ) = if let Ok(config) = app.config.try_read() {
        let mod_dir = config.tui.default_mod_directory
            .clone()
            .unwrap_or_else(|| "Not set".to_string());
        let downloads_dir = config.downloads_dir().display().to_string();
        let staging_dir = config.staging_dir().display().to_string();
        let proton_cmd = config.external_tools.proton_command.clone();
        let proton_runtime = config
            .external_tools
            .proton_runtime
            .clone()
            .unwrap_or_else(|| "Custom command/path".to_string());
        let minimal_color = if config.tui.minimal_color_mode { "Enabled" } else { "Disabled" }.to_string();
        let xedit = config.external_tools.xedit_path.clone().unwrap_or_else(|| "Not set".to_string());
        let ssedit = config.external_tools.ssedit_path.clone().unwrap_or_else(|| "Not set".to_string());
        let fnis = config.external_tools.fnis_path.clone().unwrap_or_else(|| "Not set".to_string());
        let nemesis = config.external_tools.nemesis_path.clone().unwrap_or_else(|| "Not set".to_string());
        let symphony = config.external_tools.symphony_path.clone().unwrap_or_else(|| "Not set".to_string());
        let bodyslide = config.external_tools.bodyslide_path.clone().unwrap_or_else(|| "Not set".to_string());
        let outfit = config.external_tools.outfitstudio_path.clone().unwrap_or_else(|| "Not set".to_string());

        let api_key = if let Some(ref key) = config.nexus_api_key {
            if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len()-4..])
            } else if !key.is_empty() {
                "****".to_string()
            } else {
                "Not set".to_string()
            }
        } else {
            "Not set".to_string()
        };

        let deployment_method = config.deployment.method.display_name().to_string();
        let backup_originals = if config.deployment.backup_originals { "Yes" } else { "No" }.to_string();

        (
            mod_dir,
            downloads_dir,
            staging_dir,
            proton_cmd,
            proton_runtime,
            minimal_color,
            xedit,
            ssedit,
            fnis,
            nemesis,
            symphony,
            bodyslide,
            outfit,
            api_key,
            deployment_method,
            backup_originals,
        )
    } else {
        (
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
            "Loading...".to_string(),
        )
    };

    let settings = vec![
        ("NexusMods API Key", api_key_display),
        ("Deployment Method", deployment_method_display),
        ("Backup Originals", backup_display),
        ("Downloads Directory", downloads_dir_display),
        ("Staging Directory", staging_dir_display),
        ("Default Mod Directory", mod_dir_display),
        ("Proton Command", proton_cmd_display),
        ("Proton Runtime", proton_runtime_display),
        ("Minimal Color Mode", minimal_color_display),
        ("xEdit Path", xedit_display),
        ("SSEEdit Path", ssedit_display),
        ("FNIS Path", fnis_display),
        ("Nemesis Path", nemesis_display),
        ("Symphony Path", symphony_display),
        ("BodySlide Path", bodyslide_display),
        ("Outfit Studio Path", outfit_display),
        ("Game Selection", "Change active game".to_string()),
    ];

    let items: Vec<ListItem> = settings
        .iter()
        .enumerate()
        .map(|(i, (name, value))| {
            let style = if i == state.selected_setting_index {
                themed(Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD))
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(Span::styled(name.to_string(), style)),
                Line::from(Span::styled(
                    format!("  {}", value),
                    sfg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Settings ").borders(Borders::ALL))
        .highlight_style(themed(Style::default().add_modifier(Modifier::BOLD)));

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_setting_index));
    f.render_stateful_widget(list, area, &mut list_state);
}

/// Draw FOMOD wizard
/// Draw categories sidebar for filtering
fn draw_categories_sidebar(f: &mut Frame, state: &AppState, area: Rect) {
    // Build category list with "All" option at the top
    let mut items = vec![ListItem::new(Line::from(Span::styled(
        if state.category_filter.is_none() { " > All Categories" } else { "   All Categories" },
        if state.category_filter.is_none() {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    )))];

    // Add each category
    for category in &state.categories {
        let is_selected = state.category_filter == category.id;
        let prefix = if is_selected { " > " } else { "   " };

        // Count mods in this category
        let mod_count = state.installed_mods.iter()
            .filter(|m| m.category_id == category.id)
            .count();

        let color = category.color.as_ref()
            .and_then(|c| parse_color(c))
            .unwrap_or(Color::White);

        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        items.push(ListItem::new(Line::from(Span::styled(
            format!("{}{} ({})", prefix, category.name, mod_count),
            style,
        ))));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Categories ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_category_index));
    f.render_stateful_widget(list, area, &mut list_state);
}

/// Parse hex color string to ratatui Color
fn parse_color(hex: &str) -> Option<Color> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return None;
    }

    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;

    Some(map_fg_color(Color::Rgb(r, g, b)))
}

/// Draw footer with status and keybindings
fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let status = state.status_message.as_deref().unwrap_or("");

    let guided = state.ui_mode == UiMode::Guided;

    let help_hint = if guided {
        match state.current_screen {
            Screen::GameSelect => "Enter:select  z:advanced  q:quit",
            Screen::Mods | Screen::Dashboard => {
                "j/k:nav  i:install  Space:toggle  d:delete  D:deploy  S:save-list  L:load-list  ?:help  z:advanced"
            }
            Screen::ModlistReview => "j/k:nav  Enter:queue-downloads  Esc:cancel  ?:help  z:advanced",
            Screen::LoadOrder => {
                if state.reorder_mode {
                    "j/k:move  Enter:done  s:save  Esc:cancel"
                } else {
                    "Enter:reorder  j/k:navigate  s:save  S:auto-sort  Esc:back  ?:help  z:advanced"
                }
            }
            Screen::Plugins => {
                if state.plugin_reorder_mode {
                    "j/k:move  Enter:done  s:save  Esc:cancel"
                } else {
                    "j/k:nav  Space:toggle  s:save  S:auto-sort  D:deploy  L:loot-sort  ?:help  z:advanced"
                }
            }
            Screen::Profiles => "j/k:nav  n:new  Enter:activate  d:delete  ?:help  z:advanced",
            Screen::Settings => "j/k:nav  Enter:edit  l:launch-tool  Esc:back  ?:help  z:advanced",
            Screen::Collection => "j/k:nav  i:install  a:install-all  Esc:back  ?:help  z:advanced",
            Screen::Browse => "s:search  j/k:nav  Enter:select-file  Esc:back  ?:help  z:advanced",
            Screen::ModDetails => "j/k:scroll  Esc:back  ?:help  z:advanced",
            Screen::FomodWizard => "j/k:nav  Space:select  Enter:continue  b:back  Esc:cancel  ?:help",
            Screen::DownloadQueue => "j/k:nav  p:process  m:choose-match  r:refresh  c:clear  ?:help  z:advanced",
            _ => "?:help  Esc:back  z:advanced  q:quit",
        }
    } else {
        match state.current_screen {
        Screen::GameSelect => "Enter:select  q:quit",
        Screen::Mods | Screen::Dashboard => {
            "/:search  j/k:nav  i:install  r:show-all  v:resolve-names  S:save  L:load(saved/file)  b:browse  o:load-order  Space:toggle  d:delete  D:deploy  ?:help  q:quit"
        },
        Screen::ModlistReview => "j/k:nav  Enter:queue-downloads  Esc:cancel  ?:help",
        Screen::LoadOrder => {
            if state.reorder_mode {
                "j/k:move  J/K:jump-5  t/b:top/bottom  Enter:stop-reorder  s:save  Esc:cancel-reorder"
            } else {
                "Enter:reorder  j/k:navigate  s:save  S:auto-sort  Esc:back  ?:help  q:quit"
            }
        }
        Screen::Plugins => {
            if state.plugin_reorder_mode {
                "j/k:move  J/K:jump-5  t/b:top/bottom  #:go-to-position  Enter:stop-reorder  s:save  Esc:cancel"
            } else {
                "/:search  Enter:reorder  j/k:nav  Space:toggle  a:enable-all  n:disable-all  s:save  S:auto-sort  D:deploy  L:loot-sort  ?:help  q:quit"
            }
        }
        Screen::Profiles => "j/k:nav  n:new  Enter:activate  d:delete  ?:help  q:quit",
        Screen::Settings => "j/k:nav  Enter:edit  l:launch-tool  Esc:back  ?:help  q:quit",
        Screen::Collection => "j/k:nav  i:install  a:install-all  Esc:back  ?:help  q:quit",
        Screen::Browse => "s:search  f:sort  n/p:page  j/k:nav  Enter:select-file  Esc:back  ?:help  q:quit",
        Screen::ModDetails => "j/k:scroll  Esc:back  ?:help  q:quit",
        Screen::FomodWizard => "j/k:nav  Space:select  Enter:continue  b:back  Esc:cancel  ?:help",
        Screen::DownloadQueue => "j/k:nav  h/l:alt  m:apply-alt  M:manual-id  p:process  r:refresh  c:clear  ?:help  q:quit",
        _ => "?:help  Esc:back  q:quit",
        }
    };

    let workflow_hint = if pipeline_step(state.current_screen).is_some() {
        if guided {
            "Pipeline: [:prev ]:next | 1:Mods 2:Modlists 3:Import 4:Queue | Tab:next"
        } else {
            "Pipeline: [:prev ]:next | 1:Mods 2:Modlists 3:Import 4:Queue 5:Plugins 6:Profiles 7:Settings 8:Catalog Tab:next"
        }
    } else {
        if guided {
            "1:Mods 2:Modlists 3:Import 4:Queue | z:advanced"
        } else {
            "1:Mods 2:Modlists 3:Import 4:Queue 5:Plugins 6:Profiles 7:Settings 8:Catalog Tab:next"
        }
    };
    let footer_text = if !status.is_empty() {
        format!(" {} | {} | {}", status, help_hint, workflow_hint)
    } else {
        format!(" {} | {}", help_hint, workflow_hint)
    };

    let footer = Paragraph::new(footer_text)
        .style(sfg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));

    f.render_widget(footer, area);
}

/// Draw confirmation dialog
fn draw_confirm_dialog(f: &mut Frame, dialog: &crate::app::state::ConfirmDialog) {
    let area = centered_rect(50, 30, f.area());

    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            &dialog.message,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!(
            "[Y] {}  [N] {}",
            dialog.confirm_text, dialog.cancel_text
        )),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(format!(" {} ", dialog.title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Center);

    f.render_widget(popup, area);
}

fn draw_requirements_dialog(f: &mut Frame, dialog: &crate::app::state::RequirementsDialog) {
    let area = centered_rect(70, 80, f.area());

    f.render_widget(Clear, area);

    let mut text_lines: Vec<Line> = vec![Line::from("")];

    // Show summary
    if dialog.missing_mods.is_empty() && dialog.dlc_requirements.is_empty() {
        text_lines.push(Line::from(Span::styled(
            "✓ All requirements satisfied!",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        )));
    } else {
        if dialog.installed_count > 0 {
            text_lines.push(Line::from(Span::styled(
                format!("✓ {} requirement(s) already installed", dialog.installed_count),
                Style::default().fg(Color::Green),
            )));
            text_lines.push(Line::from(""));
        }

        // Show missing mods
        if !dialog.missing_mods.is_empty() {
            text_lines.push(Line::from(Span::styled(
                format!("⚠ {} missing mod(s):", dialog.missing_mods.len()),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            text_lines.push(Line::from(""));

            for (i, req) in dialog.missing_mods.iter().enumerate() {
                let is_selected = i == dialog.selected_index;
                let prefix = if is_selected { "→ " } else { "  " };

                let mut style = Style::default();
                if is_selected {
                    style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
                }

                text_lines.push(Line::from(Span::styled(
                    format!("{}{}. {} (ID: {})", prefix, i + 1, req.name, req.mod_id),
                    style,
                )));

                if let Some(ref notes) = req.notes {
                    text_lines.push(Line::from(Span::styled(
                        format!("     Note: {}", notes),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }

        // Show DLC requirements
        if !dialog.dlc_requirements.is_empty() {
            text_lines.push(Line::from(""));
            text_lines.push(Line::from(Span::styled(
                format!("📦 {} DLC requirement(s):", dialog.dlc_requirements.len()),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            )));
            text_lines.push(Line::from(""));

            for (i, dlc) in dialog.dlc_requirements.iter().enumerate() {
                text_lines.push(Line::from(format!("  {}. {}", i + 1, dlc.name)));
                if let Some(ref notes) = dlc.notes {
                    text_lines.push(Line::from(Span::styled(
                        format!("     {}", notes),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }
    }

    text_lines.push(Line::from(""));
    text_lines.push(Line::from("─".repeat(60)));

    if !dialog.missing_mods.is_empty() {
        text_lines.push(Line::from(Span::styled(
            "j/k or ↑/↓: Navigate  Enter/d: Download  Esc/q: Close",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        text_lines.push(Line::from(Span::styled(
            "Esc/q: Close",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let popup = Paragraph::new(text_lines)
        .block(
            Block::default()
                .title(format!(" {} ", dialog.title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });

    f.render_widget(popup, area);
}

/// Draw help overlay
fn draw_help(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 95, f.area());

    f.render_widget(Clear, area);

    let pages: Vec<(&str, Vec<&str>)> = vec![
        (
            "Global Navigation",
            vec![
                "Tabs",
                "  F1 Mods",
                "  F2 Plugins",
                "  F3 Profiles",
                "  F4 Settings",
                "  F5 Import",
                "  F6 Queue",
                "  F7 Catalog",
                "  F8 Modlists",
                "",
                "Global",
                "  1..8        Workflow jumps (Mods->Modlists->Import->Queue->Plugins->Profiles->Settings->Catalog)",
                "  Tab         Next workflow stage",
                "  Shift+Tab   Previous workflow stage",
                "  ] / [       Next/prev install pipeline stage (Mods->Modlists->Import->Queue)",
                "  z           Toggle Guided/Advanced mode",
                "  g           Game selection screen",
                "  Esc         Back (when not in help/input)",
                "  q/Ctrl+C    Quit",
                "  ?           Open/close help",
                "",
                "Help paging",
                "  n / Right / PgDn   Next help page",
                "  p / Left  / PgUp   Previous help page",
                "  Esc or ?           Close help",
            ],
        ),
        (
            "Mods Screen (F1)",
            vec![
                "Navigation",
                "  j/k, Up/Down        Select mod",
                "  PgDn/PgUp           Jump by 10 mods",
                "  Home/End            Jump to start/end",
                "  Enter               Open mod details",
                "",
                "Actions",
                "  Space/e             Toggle enable/disable",
                "  a / n               Enable all / disable all",
                "  + / -               Adjust priority",
                "  /                   Search mods by name",
                "  i                   Install from path",
                "  I                   Bulk install from default folder",
                "  l                   Install from Downloads folder",
                "  d/Delete            Delete selected mod",
                "  D                   Deploy enabled mods",
                "  r                   Refresh + show all installed mods",
                "  v                   Resolve unresolved numeric mod names",
                "  o                   Open load order",
                "  C                   Load Nexus collection file",
                "  b                   Browse Nexus",
                "  U                   Check updates",
                "  x                   Check requirements",
            ],
        ),
        (
            "Mods Screen (F1) - Extended",
            vec![
                "FOMOD and Categorization",
                "  f                   Reconfigure selected mod FOMOD",
                "  Left/Right          Category selection pane",
                "  c                   Assign selected category to mod",
                "  A                   Auto-categorize uncategorized mods",
                "  F                   Force recategorize all mods",
                "  s                   Auto-sort by category",
                "  R                   Rescan staging and sync DB",
                "",
                "Modlist operations",
                "  S                   Save modlist",
                "  L                   Load modlist (saved or file)",
                "",
                "Notes",
                "  - Some actions require active game/API key.",
                "  - File picker overlays use j/k + Enter + Esc.",
            ],
        ),
        (
            "Plugins + Load Order",
            vec![
                "Plugins Screen (F2)",
                "  /                   Search plugins",
                "  Enter               Toggle reorder mode",
                "  j/k                 Move or navigate",
                "  J/K                 Jump by 5 (reorder mode)",
                "  t/b                 Move to top/bottom (reorder mode)",
                "  #                   Jump to absolute position",
                "  Space               Toggle plugin enabled",
                "  a / n               Enable all / disable all",
                "  s                   Save plugin order",
                "  S                   Native auto-sort",
                "  D                   Deploy mods",
                "  L                   LOOT auto-sort",
                "",
                "Load Order Screen (o from F1)",
                "  Enter               Toggle reorder mode",
                "  j/k, J/K, t/b       Reorder controls",
                "  s                   Save",
                "  S                   Auto-sort by category",
            ],
        ),
        (
            "Profiles + Settings",
            vec![
                "Profiles Screen (F3)",
                "  j/k, Up/Down        Select profile",
                "  n                   New profile",
                "  Enter               Activate profile",
                "  d/Delete            Delete profile",
                "",
                "Settings Screen (F4)",
                "  j/k, Up/Down        Select setting row",
                "  Enter               Edit/toggle selected setting",
                "  l                   Launch tool (tool path rows)",
                "",
                "Editable settings include",
                "  API key, deployment, backup",
                "  downloads/staging/default mod dir",
                "  Proton command, Proton runtime",
                "  minimal color mode, tool executable paths",
                "  game selection",
            ],
        ),
        (
            "Import + Queue + Catalog",
            vec![
                "Import Screen (F5)",
                "  i                   Import MO2 modlist file",
                "",
                "Import Review",
                "  j/k                 Navigate matches",
                "  Enter               Queue pending downloads",
                "",
                "Queue Screen (F6)",
                "  j/k                 Select entry",
                "  p                   Process selected batch",
                "  r                   Refresh queue",
                "  c                   Clear selected batch",
                "  h/l                 Cycle alternatives",
                "  m                   Apply alternative",
                "  M                   Manual Nexus mod ID",
                "",
                "Catalog Screen (F7)",
                "  /                   Search catalog",
                "  n/p                 Next/prev page",
                "  r                   Reset search",
            ],
        ),
        (
            "Modlists + Other TUI Flows",
            vec![
                "Modlists Screen (F8)",
                "  j/k                 Navigate saved modlists/entries",
                "  Enter               Open list or entry",
                "  i                   Add installed mods to open modlist",
                "  c                   Add catalog mod by ID/search",
                "  o                   Add local directory archives",
                "  n                   New modlist",
                "  l                   Load selected saved modlist for review/queue",
                "  a                   Activate selected/edited modlist",
                "  d/Delete            Delete saved modlist or entry",
                "  s                   Save/refresh editor entries",
                "  x                   Export selected/edited modlist",
                "  Esc                 Back to picker/previous screen",
                "",
                "Browse (from F1 'b')",
                "  s                   Start search",
                "  f                   Cycle sort mode",
                "  n/p, PgDn/PgUp      Next/previous page",
                "  j/k                 Navigate results",
                "  Enter               Select mod then file",
                "",
                "Collection/Requirements dialogs",
                "  j/k                 Navigate",
                "  Enter/d             Download selected requirement",
            ],
        ),
        (
            "CLI Command Map",
            vec![
                "Top-level commands",
                "  tui, game, mod, profile, import, queue, modlist",
                "  nexus, deployment, tool, deploy, status, doctor,",
                "  init, audit, getting-started",
                "",
                "Game",
                "  list, scan, select, info, add-path, remove-path",
                "Mod",
                "  list, install, enable, disable, remove, info, rescan",
                "Profile",
                "  list, create, switch, delete, export, import",
                "Import/Queue/Modlist",
                "  import modlist/status, queue list/process/retry/clear,",
                "  modlist save/load",
                "Nexus/Deployment/Tool",
                "  nexus populate/status",
                "  deployment show/set-method/set-downloads-dir/clear-downloads-dir/",
                "    set-staging-dir/clear-staging-dir",
                "  tool show/list-proton/use-proton/clear-proton-runtime/",
                "    set-proton/set-path/clear-path/run",
            ],
        ),
    ];

    let page_count = pages.len();
    let current = state.help_page.min(page_count.saturating_sub(1));
    let (title, lines) = &pages[current];
    let mut help_text = vec![
        Line::from(Span::styled(
            format!("{} ({}/{})", title, current + 1, page_count),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    help_text.extend(lines.iter().map(|line| Line::from(*line)));
    help_text.push(Line::from(""));
    help_text.push(Line::from(Span::styled(
        format!(
            "Mode: {} | n/Right/PgDn: next page  p/Left/PgUp: prev page  Esc/?: close",
            match state.ui_mode {
                UiMode::Guided => "Guided",
                UiMode::Advanced => "Advanced",
            }
        ),
        Style::default().fg(Color::DarkGray),
    )));

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(help, area);
}

/// Draw mod install path input dialog
fn draw_mod_install_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(80, 35, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter path...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter full path to mod archive or downloads folder:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  /home/user/Downloads/SkyUI-5.2SE.7z"),
        Line::from("  ~/Downloads/"),
        Line::from(""),
        Line::from("[Enter] Install  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Install Mod ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw collection path input dialog
fn draw_collection_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter path...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter path to Nexus Mods collection.json file:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  /path/to/collection.json"),
        Line::from("  ./collection.json"),
        Line::from("  ~/Downloads/my-collection.json"),
        Line::from(""),
        Line::from("[Enter] Load  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Load Collection ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw profile name input dialog
fn draw_profile_name_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 25, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter profile name...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter a name for the new profile:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("[Enter] Create  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Create New Profile ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw mod directory input dialog
fn draw_mod_directory_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "~/Downloads".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set default mod directory:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("This directory will be used for bulk installation."),
        Line::from("Leave empty to disable."),
        Line::from(""),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Default Mod Directory ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw downloads directory input dialog
fn draw_downloads_directory_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "~/.local/share/modsanity/downloads".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set downloads directory override:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Downloaded archives will be stored here."),
        Line::from("Leave empty to use default."),
        Line::from(""),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Downloads Directory ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw staging directory input dialog
fn draw_staging_directory_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "~/.local/share/modsanity/mods".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set staging/installed mods directory override:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Installed mods are extracted under this root (per game)."),
        Line::from("Leave empty to use default."),
        Line::from(""),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Staging Directory ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw proton command input dialog
fn draw_proton_command_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 28, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "proton".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set custom Proton command/path for external tools:"),
        Line::from(""),
        Line::from(Span::styled(input_text, Style::default().fg(Color::Yellow))),
        Line::from(""),
        Line::from("Examples: proton, /path/to/proton"),
        Line::from("Tip: Use 'Proton Runtime' in Settings for Steam-managed Proton"),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Proton Command ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw external tool path input dialog
fn draw_external_tool_path_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(75, 30, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "C:\\\\Path\\\\Tool.exe".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set Windows EXE path for selected tool:"),
        Line::from(""),
        Line::from(Span::styled(input_text, Style::default().fg(Color::Yellow))),
        Line::from(""),
        Line::from("This path is launched via Proton."),
        Line::from("Leave empty to clear."),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" External Tool Path ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw NexusMods API key input dialog
fn draw_nexus_api_key_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 35, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter your API key...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter your NexusMods Personal API Key:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("You can find your Personal API Key at:"),
        Line::from(Span::styled(
            "https://www.nexusmods.com/users/myaccount?tab=api",
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from("This key is required for browsing and downloading mods."),
        Line::from("Leave empty to clear."),
        Line::from(""),
        Line::from("[Enter] Save  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" NexusMods API Key ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw mod search input dialog
fn draw_mod_search_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 25, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Search installed mods by name:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} █", input_text),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Press Enter to search, Esc to cancel"),
        Line::from("Leave empty and press Enter to clear search"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Search Mods ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw plugin search input dialog
fn draw_plugin_search_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 25, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Search plugins by filename:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} █", input_text),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Press Enter to search, Esc to cancel"),
        Line::from("Leave empty and press Enter to clear search"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Search Plugins ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw plugin position input dialog
fn draw_plugin_position_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "".to_string()
    } else {
        state.input_buffer.clone()
    };

    let plugin_count = state.plugins.len();
    let current_position = state.selected_plugin_index + 1;

    let text = vec![
        Line::from(""),
        Line::from("Move plugin to position:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} █", input_text),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Current position: {}", current_position)),
        Line::from(format!("Valid range: 1-{}", plugin_count)),
        Line::from(""),
        Line::from("[Enter] Move  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Go to Position ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw FOMOD component selection dialog
fn draw_fomod_component_selection(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 80, f.area());

    f.render_widget(Clear, area);

    // Create component list
    let items: Vec<ListItem> = state
        .fomod_components
        .iter()
        .enumerate()
        .map(|(i, component)| {
            let is_selected = state.selected_fomod_components.contains(&i);
            let checkbox = if is_selected { "[X]" } else { "[ ]" };
            let required = if component.is_required { " (Required)" } else { "" };

            let style = if i == state.fomod_selection_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(format!(" {} {}{}", checkbox, component.description, required))
                .style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" FOMOD Installer - Select Components ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    // Split area for list and instructions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(4)])
        .split(area);

    // Use stateful rendering
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.fomod_selection_index));

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Instructions
    let instructions = Paragraph::new(vec![
        Line::from(""),
        Line::from("Space: Toggle   Enter: Install Selected   Esc: Cancel"),
    ])
    .block(Block::default().borders(Borders::TOP))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));

    f.render_widget(instructions, chunks[1]);
}

/// Draw installation progress dialog
fn draw_installation_progress(f: &mut Frame, progress: &crate::app::state::InstallProgress) {
    // Determine if this is a bulk install
    let is_bulk = progress.current_mod_index.is_some() && progress.total_mods.is_some();

    let area = if is_bulk {
        centered_rect(70, 35, f.area()) // Taller for bulk install
    } else {
        centered_rect(60, 25, f.area()) // Original size for single install
    };

    f.render_widget(Clear, area);

    // Build the title based on install type
    let title = if is_bulk {
        " Bulk Install Progress "
    } else {
        " Installing Mod "
    };

    // Split area for bulk install (overall + current mod progress)
    if is_bulk {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // Overall progress section
                Constraint::Length(1),  // Spacer
                Constraint::Length(6),  // Current mod progress section
                Constraint::Min(1),     // Info text
            ])
            .split(area);

        // Overall progress gauge
        if let (Some(current_idx), Some(total)) = (progress.current_mod_index, progress.total_mods) {
            let overall_percent = if total > 0 {
                ((current_idx as f64 / total as f64) * 100.0) as u16
            } else {
                0
            };

            let overall_gauge = Gauge::default()
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
                .percent(overall_percent)
                .label(format!("Mod {}/{}", current_idx, total));

            f.render_widget(overall_gauge, chunks[0]);
        }

        // Current mod progress gauge
        let mod_name = progress.current_mod_name.as_deref().unwrap_or("Unknown");
        let current_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(format!(" {} ", truncate_filename(mod_name, 50)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .percent(progress.percent)
            .label(format!(
                "{}/{} files",
                progress.processed_files,
                progress.total_files
            ));

        f.render_widget(current_gauge, chunks[2]);

        // Current file info
        let info_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Current file:",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                truncate_filename(&progress.current_file, 60),
                Style::default().fg(Color::White),
            )),
        ];

        let info = Paragraph::new(info_text)
            .alignment(Alignment::Center);

        f.render_widget(info, chunks[3]);
    } else {
        // Single mod install - original simple display
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::Black))
            .percent(progress.percent)
            .label(format!(
                "{}/{} files - {}",
                progress.processed_files,
                progress.total_files,
                truncate_filename(&progress.current_file, 40)
            ));

        f.render_widget(gauge, area);
    }
}

/// Draw categorization progress dialog
fn draw_categorization_progress(f: &mut Frame, progress: &crate::app::state::CategorizationProgress) {
    let area = centered_rect(60, 30, f.area());

    f.render_widget(Clear, area);

    // Calculate progress percentage
    let percent = if progress.total_mods > 0 {
        ((progress.current_index as f64 / progress.total_mods as f64) * 100.0) as u16
    } else {
        0
    };

    // Progress gauge
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Auto-Categorizing Mods ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .percent(percent)
        .label(format!(
            "{}/{} mods ({} categorized)",
            progress.current_index,
            progress.total_mods,
            progress.categorized_count
        ));

    // Split area for gauge and current mod info
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Gauge
            Constraint::Min(1),     // Info text
        ])
        .split(area);

    f.render_widget(gauge, chunks[0]);

    // Current mod info
    let info_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Analyzing:",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            truncate_filename(&progress.current_mod_name, 50),
            Style::default().fg(Color::White),
        )),
    ];

    let info = Paragraph::new(info_text)
        .alignment(Alignment::Center);

    f.render_widget(info, chunks[1]);
}

/// Draw import progress dialog
fn draw_import_progress(f: &mut Frame, progress: &crate::app::state::ImportProgress) {
    let area = centered_rect(70, 30, f.area());

    f.render_widget(Clear, area);

    // Calculate progress percentage
    let percent = if progress.total_plugins > 0 {
        ((progress.current_index as f64 / progress.total_plugins as f64) * 100.0) as u16
    } else {
        0
    };

    // Progress gauge
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(format!(" Importing Modlist - {} ", progress.stage))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
        .percent(percent)
        .label(format!(
            "{}/{} plugins",
            progress.current_index,
            progress.total_plugins
        ));

    // Split area for gauge and current plugin info
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Gauge
            Constraint::Min(1),     // Info text
        ])
        .split(area);

    f.render_widget(gauge, chunks[0]);

    // Current plugin info
    let info_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Matching plugin:",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            truncate_filename(&progress.current_plugin_name, 60),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Searching NexusMods for matches...",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let info = Paragraph::new(info_text)
        .alignment(Alignment::Center);

    f.render_widget(info, chunks[1]);
}

/// Draw browse/search screen
fn draw_browse_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(10),   // Results
        ])
        .split(area);

    // Search bar
    let search_text = if state.input_mode == InputMode::BrowseSearch {
        format!(" Search: {} █", state.input_buffer)
    } else if state.browse_showing_default {
        format!(" Showing: Top Mods (Press 's' to search, 'f' to filter/sort)")
    } else {
        format!(" Search: {} (Press 's' to search, 'f' to filter/sort)", state.browse_query)
    };

    let search_style = if state.input_mode == InputMode::BrowseSearch {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };

    let search_bar = Paragraph::new(search_text)
        .style(search_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if state.input_mode == InputMode::BrowseSearch {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                }),
        );

    f.render_widget(search_bar, chunks[0]);

    // Results area - split into list and details
    let result_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // Search results list
    if state.browsing {
        // Show loading indicator
        let loading = Paragraph::new(" Searching Nexus Mods...")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().title(" Results ").borders(Borders::ALL));
        f.render_widget(loading, result_chunks[0]);
    } else if state.browse_results.is_empty() {
        // Show empty state
        let empty_msg = if state.browse_query.is_empty() {
            " Press 's' to start searching for mods "
        } else {
            " No results found. Try a different search term. "
        };

        let empty = Paragraph::new(empty_msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title(" Results ").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(empty, result_chunks[0]);
    } else {
        // Show results
        let items: Vec<ListItem> = state
            .browse_results
            .iter()
            .enumerate()
            .map(|(i, result)| {
                let style = if i == state.selected_browse_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Format: Name (author) - downloads
                let text = format!(
                    " {} by {} - {} downloads",
                    result.name,
                    result.author,
                    format_number(result.downloads)
                );

                ListItem::new(text).style(style)
            })
            .collect();

        let total = state.browse_total_count;
        let limit = if state.browse_limit > 0 {
            state.browse_limit as i64
        } else {
            state.browse_results.len() as i64
        };
        let showing_start = if state.browse_results.is_empty() {
            0
        } else {
            state.browse_offset as i64 + 1
        };
        let showing_end = if state.browse_results.is_empty() {
            0
        } else {
            state.browse_offset as i64 + state.browse_results.len() as i64
        };
        let page = if total > 0 && limit > 0 {
            (state.browse_offset as i64 / limit) + 1
        } else {
            1
        };
        let pages = if total > 0 && limit > 0 {
            (total + limit - 1) / limit
        } else {
            1
        };
        let label = if state.browse_showing_default {
            "Top Mods"
        } else {
            "Results"
        };
        let title = if total > 0 {
            format!(
                " {}: {}-{} of {} | Page {}/{} | Sort: {:?} (n/p, PgUp/PgDn) ",
                label, showing_start, showing_end, total, page, pages, state.browse_sort
            )
        } else {
            format!(
                " {}: {} | Sort: {:?} (n/p, PgUp/PgDn) ",
                label,
                state.browse_results.len(),
                state.browse_sort
            )
        };

        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_browse_index));
        f.render_stateful_widget(list, result_chunks[0], &mut list_state);
    }

    // Details panel
    if let Some(result) = state.browse_results.get(state.selected_browse_index) {
        let mut details = vec![
            Line::from(Span::styled(
                &result.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Author:     {}", result.author)),
            Line::from(format!("Category:   {}", result.category)),
            Line::from(format!("Version:    {}", result.version)),
            Line::from(""),
            Line::from(Span::styled("Stats:", Style::default().add_modifier(Modifier::BOLD))),
            Line::from(format!("Downloads:  {}", format_number(result.downloads))),
            Line::from(format!("Endorsements: {}", format_number(result.endorsements))),
            Line::from(format!("Updated:    {}", result.updated_at)),
            Line::from(""),
        ];

        // Add description if available
        if !result.summary.is_empty() {
            details.push(Line::from(Span::styled(
                "Description:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            details.push(Line::from(""));

            // Wrap description text
            let max_width = result_chunks[1].width.saturating_sub(4) as usize;
            for line in wrap_text(&result.summary, max_width) {
                details.push(Line::from(line));
            }
        }

        let details_widget = Paragraph::new(details)
            .block(Block::default().title(" Mod Details ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(details_widget, result_chunks[1]);
    } else {
        let empty = Paragraph::new(" No mod selected ")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title(" Mod Details ").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(empty, result_chunks[1]);
    }
}

/// Draw file picker overlay for selecting which file to download
fn draw_file_picker(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 60, f.area());
    f.render_widget(Clear, area);

    if state.browse_mod_files.is_empty() {
        let loading = Paragraph::new(" Loading files...")
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .title(" Select File to Download ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        f.render_widget(loading, area);
        return;
    }

    let items: Vec<ListItem> = state
        .browse_mod_files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let style = if i == state.selected_file_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let size_str = format_file_size(file.size_bytes);
            let category_color = match file.category.as_str() {
                "MAIN" => Color::Green,
                "UPDATE" => Color::Yellow,
                "OPTIONAL" => Color::Cyan,
                "OLD_VERSION" => Color::DarkGray,
                _ => Color::White,
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" [{}] ", file.category),
                    Style::default().fg(category_color),
                ),
                Span::raw(&file.name),
                Span::styled(
                    format!("  v{}", file.version),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("  ({})", size_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let mod_name = state
        .download_context
        .as_ref()
        .map(|c| c.mod_name.as_str())
        .unwrap_or("Unknown");

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(
                    " Select File - {} (Enter to download, Esc to cancel) ",
                    mod_name
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_file_index));
    f.render_stateful_widget(list, area, &mut list_state);
}

/// Draw download progress overlay
fn draw_download_progress(f: &mut Frame, progress: &crate::app::state::DownloadProgress) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // Title
            Constraint::Length(1), // Spacer
            Constraint::Length(3), // Progress bar
            Constraint::Min(1),   // Info
        ])
        .split(area);

    let block = Block::default()
        .title(" Downloading ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);

    let percent = if progress.total_bytes > 0 {
        ((progress.downloaded_bytes as f64 / progress.total_bytes as f64) * 100.0) as u16
    } else {
        0
    };

    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Black),
        )
        .percent(percent)
        .label(format!(
            "{} / {} ({}%)",
            format_file_size(progress.downloaded_bytes as i64),
            format_file_size(progress.total_bytes as i64),
            percent
        ));

    f.render_widget(gauge, chunks[2]);

    let info = Paragraph::new(format!("File: {}", progress.file_name))
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(info, chunks[3]);
}

/// Format file size in human readable format
fn format_file_size(bytes: i64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1_073_741_824.0 {
        format!("{:.1} GB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.1} MB", bytes / 1_048_576.0)
    } else if bytes >= 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else {
        format!("{} B", bytes as i64)
    }
}

/// Format a number with thousands separators
fn format_number(n: i64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Wrap text to a maximum width
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current_line.chars().count();
        if current_len + word_len + 1 > max_width {
            if !current_line.is_empty() {
                lines.push(current_line);
                current_line = String::new();
            }
            if word_len > max_width {
                // Split long words safely on char boundaries
                let mut chunk = String::new();
                for (i, ch) in word.chars().enumerate() {
                    if i > 0 && i % max_width == 0 {
                        lines.push(chunk);
                        chunk = String::new();
                    }
                    chunk.push(ch);
                }
                if !chunk.is_empty() {
                    lines.push(chunk);
                }
            } else {
                current_line = word.to_string();
            }
        } else {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

/// Truncate a filename to a maximum length
fn truncate_filename(filename: &str, max_len: usize) -> String {
    if filename.len() <= max_len {
        filename.to_string()
    } else {
        let half = (max_len - 3) / 2;
        format!("{}...{}", &filename[..half], &filename[filename.len() - half..])
    }
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Draw the Load Order screen
fn draw_load_order_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // -- LEFT PANEL: Mod list in priority order --
    let mode_indicator = if state.reorder_mode {
        " [REORDER MODE]"
    } else {
        ""
    };
    let dirty_indicator = if state.load_order_dirty {
        " (unsaved)"
    } else {
        ""
    };
    let title = format!(
        " Load Order ({}){}{}",
        state.load_order_mods.len(),
        mode_indicator,
        dirty_indicator
    );

    // Build set of mod names that have conflicts
    let conflict_mod_names: std::collections::HashSet<&str> = state
        .load_order_conflicts
        .iter()
        .flat_map(|c| vec![c.mod1.as_str(), c.mod2.as_str()])
        .collect();

    let items: Vec<ListItem> = state
        .load_order_mods
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let priority_label = format!("{:>3}", i);
            let status = if m.enabled { "*" } else { " " };
            let has_conflict = conflict_mod_names.contains(m.name.as_str());
            let conflict_marker = if has_conflict { "!" } else { " " };

            let base_style = if i == state.load_order_index && state.reorder_mode {
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else if !m.enabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let conflict_style = if has_conflict {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", priority_label), Style::default().fg(Color::Cyan)),
                Span::styled(format!("[{}]", status), base_style),
                Span::styled(format!(" {} ", conflict_marker), conflict_style),
                Span::styled(m.name.clone(), base_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.load_order_index));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // -- RIGHT PANEL: Conflicts & help --
    draw_load_order_detail(f, state, chunks[1]);
}

/// Draw conflict details and keybinding help for Load Order screen
fn draw_load_order_detail(f: &mut Frame, state: &AppState, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(m) = state.load_order_mods.get(state.load_order_index) {
        lines.push(Line::from(Span::styled(
            m.name.clone(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )));
        lines.push(Line::from(format!(
            "Priority: {}  |  {}",
            state.load_order_index,
            if m.enabled { "Enabled" } else { "Disabled" }
        )));
        lines.push(Line::from(""));

        // Find conflicts involving this mod
        let relevant: Vec<_> = state
            .load_order_conflicts
            .iter()
            .filter(|c| c.mod1 == m.name || c.mod2 == m.name)
            .collect();

        if relevant.is_empty() {
            lines.push(Line::from(Span::styled(
                "No file conflicts",
                Style::default().fg(Color::Green),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("{} conflict(s):", relevant.len()),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            for conflict in &relevant {
                let other = if conflict.mod1 == m.name {
                    &conflict.mod2
                } else {
                    &conflict.mod1
                };
                let wins = &conflict.winner == &m.name;
                let win_text = if wins { " (you win)" } else { " (they win)" };
                let win_color = if wins { Color::Green } else { Color::Red };

                lines.push(Line::from(vec![
                    Span::styled(
                        format!("vs {} ", other),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("({} files{})", conflict.files.len(), win_text),
                        Style::default().fg(win_color),
                    ),
                ]));

                for file in conflict.files.iter().take(3) {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", file),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if conflict.files.len() > 3 {
                    lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", conflict.files.len() - 3),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(""));
            }
        }
    } else {
        lines.push(Line::from("No mods in load order"));
    }

    // Keybinding help at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Keys:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from("  Enter  Toggle reorder mode"));
    lines.push(Line::from("  j/k    Navigate (or move mod)"));
    lines.push(Line::from("  J/K    Jump 5 positions"));
    lines.push(Line::from("  t/b    Move to top/bottom"));
    lines.push(Line::from("  s      Save order"));
    lines.push(Line::from("  S      Auto-sort by category"));
    lines.push(Line::from("  Esc    Back to Mods"));

    let panel = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Conflicts & Help ")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(panel, area);
}

/// Draw import file path input dialog
fn draw_import_file_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 30, f.area());

    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter path...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Enter path to modlist.txt:"),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("This should be the path to your MO2 modlist.txt file."),
        Line::from("Example: ~/MO2/profiles/Default/modlist.txt"),
        Line::from(""),
        Line::from("[Enter] Confirm  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Import Modlist ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw import screen (file selection)
fn draw_import_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // Instructions
            Constraint::Length(3),  // File path input
            Constraint::Min(5),     // Recent imports
        ])
        .split(area);

    // Instructions
    let instructions = vec![
        Line::from(" Import MO2 Modlist ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("  This feature imports a Mod Organizer 2 modlist.txt file,"),
        Line::from("  automatically matches plugins to NexusMods, and creates"),
        Line::from("  a download queue for batch installation."),
    ];
    let instructions_widget = Paragraph::new(instructions)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(instructions_widget, chunks[0]);

    // File path input
    let input_text = if state.import_file_path.is_empty() {
        "Enter path to modlist.txt..."
    } else {
        &state.import_file_path
    };
    let input_widget = Paragraph::new(input_text)
        .block(Block::default()
            .title(" Modlist File Path (i to edit, Enter to import) ")
            .borders(Borders::ALL));
    f.render_widget(input_widget, chunks[1]);

    // Recent imports placeholder
    let recent = vec![
        Line::from(" Recent Imports: "),
        Line::from(""),
        Line::from("  No recent imports"),
    ];
    let recent_widget = Paragraph::new(recent)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(recent_widget, chunks[2]);
}

/// Draw import review screen (showing matched mods)
fn draw_import_review_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // Summary
            Constraint::Min(10),     // Results list
            Constraint::Length(8),   // Details/alternatives
        ])
        .split(area);

    // Summary
    let auto_matched = state.import_results.iter().filter(|r| r.confidence.is_high()).count();
    let needs_review = state.import_results.iter().filter(|r| r.confidence.needs_review()).count();
    let no_matches = state.import_results.iter().filter(|r| r.confidence.is_none()).count();

    let summary = format!(
        " Total: {} | Auto-matched: {} | Needs Review: {} | No Matches: {} ",
        state.import_results.len(), auto_matched, needs_review, no_matches
    );
    let summary_widget = Paragraph::new(summary)
        .block(Block::default()
            .title(" Import Results ")
            .borders(Borders::ALL))
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(summary_widget, chunks[0]);

    // Results list
    let items: Vec<ListItem> = state
        .import_results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let icon = if result.confidence.is_high() {
                "✓"
            } else if result.confidence.needs_review() {
                "⚠"
            } else {
                "!"
            };

            let mod_name = if let Some(m) = &result.best_match {
                &m.name
            } else {
                "<no match>"
            };

            let style = if i == state.selected_import_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(format!(
                " {} {} → {}",
                icon, result.plugin.plugin_name, mod_name
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .title(" Matches (↑/↓ to navigate, Enter to create queue) ")
            .borders(Borders::ALL));
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_import_index));
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Details for selected
    if let Some(result) = state.import_results.get(state.selected_import_index) {
        let mut details = vec![
            Line::from(format!("Plugin: {}", result.plugin.plugin_name)),
            Line::from(format!("Extracted: {}", result.mod_name)),
        ];

        if let Some(best) = &result.best_match {
            details.push(Line::from(format!("Match: {} (by {})", best.name, best.author)));
            details.push(Line::from(format!("Confidence: {:.0}%", result.confidence.score() * 100.0)));
            details.push(Line::from(format!("Downloads: {}", best.downloads)));
        } else {
            details.push(Line::from("No matches found - will need manual resolution"));
        }

        if !result.alternatives.is_empty() {
            details.push(Line::from(format!("\n{} alternative(s) available", result.alternatives.len())));
        }

        let details_widget = Paragraph::new(details)
            .block(Block::default()
                .title(" Details ")
                .borders(Borders::ALL));
        f.render_widget(details_widget, chunks[2]);
    }
}

/// Draw download queue screen
fn draw_queue_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let guided = state.ui_mode == UiMode::Guided;
    fn progress_bar(progress: f32, width: usize) -> String {
        let p = progress.clamp(0.0, 1.0);
        let filled = (p * width as f32).round() as usize;
        let filled = filled.min(width);
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "#".repeat(filled), "-".repeat(empty))
    }

    fn truncate_for_queue(text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            return text.to_string();
        }
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if guided {
            vec![
                Constraint::Length(3), // Status bar
                Constraint::Min(10),   // Queue list
            ]
        } else {
            vec![
                Constraint::Length(3), // Status bar
                Constraint::Min(10),   // Queue list
                Constraint::Length(6), // Selected item details
            ]
        })
        .split(area);

    // Status bar
    let pending = state.queue_entries.iter().filter(|e| matches!(e.status, crate::queue::QueueStatus::Pending | crate::queue::QueueStatus::Matched)).count();
    let downloading = state
        .queue_entries
        .iter()
        .filter(|e| matches!(e.status, crate::queue::QueueStatus::Downloading | crate::queue::QueueStatus::Installing))
        .count();
    let completed = state.queue_entries.iter().filter(|e| matches!(e.status, crate::queue::QueueStatus::Completed)).count();
    let failed = state.queue_entries.iter().filter(|e| matches!(e.status, crate::queue::QueueStatus::Failed)).count();

    let status_text = if guided {
        if state.queue_processing {
            format!(
                " Processing: {} pending, {} active, {} completed, {} failed ",
                pending, downloading, completed, failed
            )
        } else {
            format!(" Queue: {} pending, {} completed, {} failed ", pending, completed, failed)
        }
    } else if state.queue_processing {
        format!(
            " Processing: {} pending, {} downloading, {} completed, {} failed | ESC to stop ",
            pending, downloading, completed, failed
        )
    } else {
        format!(
            " Queue: {} pending, {} completed, {} failed | p to process, c to clear ",
            pending, completed, failed
        )
    };

    let status_widget = Paragraph::new(status_text)
        .block(Block::default()
            .title(" Download Queue ")
            .borders(Borders::ALL))
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(status_widget, chunks[0]);

    // Queue list
    let items: Vec<ListItem> = state
        .queue_entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let status_icon = match entry.status {
                crate::queue::QueueStatus::Completed => "✓",
                crate::queue::QueueStatus::Failed => "✗",
                crate::queue::QueueStatus::Downloading => "↓",
                crate::queue::QueueStatus::Installing => "↻",
                crate::queue::QueueStatus::NeedsReview => "⚠",
                crate::queue::QueueStatus::NeedsManual => "!",
                _ => "○",
            };

            let progress_bar = if entry.progress > 0.0 {
                format!(" [{:3.0}%]", entry.progress * 100.0)
            } else {
                String::new()
            };

            let style = if i == state.selected_queue_index {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(format!(
                " {} {} → {}{}",
                status_icon, entry.plugin_name, entry.mod_name, progress_bar
            ))
            .style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .title(" Queue Entries (↑/↓ to navigate) ")
            .borders(Borders::ALL));
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_queue_index));
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Selected entry details (Advanced mode)
    if guided {
        return;
    }

    // Selected entry details
    if let Some(entry) = state.queue_entries.get(state.selected_queue_index) {
        let mut details = vec![
            Line::from(format!("Plugin: {}", entry.plugin_name)),
            Line::from(format!("Mod: {}", entry.mod_name)),
            Line::from(format!("Status: {:?}", entry.status)),
        ];

        if let Some(conf) = entry.match_confidence {
            details.push(Line::from(format!("Confidence: {:.0}%", conf * 100.0)));
        }

        if let Some(err) = &entry.error {
            details.push(Line::from(format!("Error: {}", err)).style(Style::default().fg(Color::Red)));
        }

        let active_downloads: Vec<_> = state
            .queue_entries
            .iter()
            .filter(|e| matches!(e.status, crate::queue::QueueStatus::Downloading | crate::queue::QueueStatus::Installing))
            .take(3)
            .collect();
        if !active_downloads.is_empty() {
            details.push(Line::from(""));
            details.push(Line::from("Active transfers:").style(Style::default().add_modifier(Modifier::BOLD)));
            for active in active_downloads {
                let label = truncate_for_queue(&active.mod_name, 22);
                let icon = match active.status {
                    crate::queue::QueueStatus::Installing => "↻",
                    _ => "↓",
                };
                details.push(Line::from(format!(
                    " {} {:22} {} {:3.0}%",
                    icon,
                    label,
                    progress_bar(active.progress, 12),
                    active.progress * 100.0
                )));
            }
        }

        if !entry.alternatives.is_empty() {
            details.push(Line::from(format!(
                "Alternative: {}/{} (h/l to cycle, m to apply)",
                state.selected_queue_alternative_index.saturating_add(1).min(entry.alternatives.len()),
                entry.alternatives.len()
            )));
            if let Some(alt) = entry.alternatives.get(state.selected_queue_alternative_index.min(entry.alternatives.len() - 1)) {
                details.push(Line::from(format!("  {} (id: {})", alt.name, alt.mod_id)));
            }
        } else if matches!(
            entry.status,
            crate::queue::QueueStatus::NeedsManual | crate::queue::QueueStatus::NeedsReview
        ) {
            details.push(Line::from("No alternatives. Press M to enter Nexus mod ID."));
        }

        let details_widget = Paragraph::new(details)
            .block(Block::default()
                .title(" Details ")
                .borders(Borders::ALL));
        f.render_widget(details_widget, chunks[2]);
    }
}

fn draw_queue_manual_mod_id_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 28, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter Nexus mod ID...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from("Set Nexus mod ID for selected queue entry:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {}", input_text),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Enter: apply  Esc: cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Queue Manual Resolve ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(popup, area);
}

/// Draw save modlist input dialog
fn draw_save_modlist_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 40, f.area());
    f.render_widget(Clear, area);

    let format_hint = match state.modlist_save_format.as_str() {
        "mo2" => "MO2 modlist.txt (plugin list only)",
        _ => "Native JSON (mods + plugins + metadata)",
    };

    let input_text = if state.input_buffer.is_empty() {
        "Enter path...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from(" Save Modlist ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(format!("Format: {} [Tab to toggle]", format_hint)),
        Line::from(""),
        Line::from("Path:"),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Examples:"),
        Line::from("  ~/modlists/my-setup.json"),
        Line::from("  ~/backups/skyrim-2024.json"),
        Line::from("  ~/modlists/fallout4.txt (for MO2 format)"),
        Line::from(""),
        Line::from("[Enter] Save  [Tab] Toggle Format  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Save Modlist ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw load modlist input dialog
fn draw_load_modlist_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 40, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter path...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from(" Load Modlist ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Path:"),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Supports:"),
        Line::from("  • Native ModSanity JSON format"),
        Line::from("  • MO2 modlist.txt format"),
        Line::from(""),
        Line::from("Format will be auto-detected."),
        Line::from(""),
        Line::from("[Enter] Load  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Load Modlist ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Left);

    f.render_widget(popup, area);
}

/// Draw modlist review screen
fn draw_modlist_review_screen(f: &mut Frame, state: &AppState, area: Rect) {
    let review = match &state.modlist_review_data {
        Some(r) => r,
        None => {
            let p = Paragraph::new("No modlist loaded")
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
    };

    // Split into 3 sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),      // Summary
            Constraint::Percentage(50), // Needs download list
            Constraint::Percentage(50), // Already installed list
        ])
        .split(area);

    // Summary section
    let summary_text = vec![
        Line::from(format!("Modlist: {}", review.source_path)),
        Line::from(format!("Format: {}", review.format)),
        Line::from(""),
        Line::from(format!("Total mods: {}", review.total_mods)),
        Line::from(Span::styled(
            format!("  Already installed: {}", review.already_installed.len()),
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::styled(
            format!("  Needs download: {}", review.needs_download.len()),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("[Enter] Queue Downloads  [Esc] Cancel"),
    ];

    let summary = Paragraph::new(summary_text)
        .block(Block::default()
            .title(" Modlist Review ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(summary, chunks[0]);

    // Needs download list (scrollable)
    let download_items: Vec<ListItem> = review.needs_download
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let style = if idx == state.selected_modlist_entry {
                Style::default().bg(Color::DarkGray).fg(Color::Yellow)
            } else {
                Style::default()
            };

            let line = format!("  {} v{}", entry.name, entry.version);
            ListItem::new(line).style(style)
        })
        .collect();

    let download_list = List::new(download_items)
        .block(Block::default()
            .title(format!(" Mods to Download ({}) ", review.needs_download.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)));
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(state.selected_modlist_entry));
    f.render_stateful_widget(download_list, chunks[1], &mut list_state);

    // Already installed list
    let installed_items: Vec<ListItem> = review.already_installed
        .iter()
        .map(|name| {
            ListItem::new(format!("  ✓ {}", name))
                .style(Style::default().fg(Color::Green))
        })
        .collect();

    let installed_list = List::new(installed_items)
        .block(Block::default()
            .title(format!(" Already Installed ({}) ", review.already_installed.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)));
    f.render_widget(installed_list, chunks[2]);
}

/// Draw catalog search input overlay
fn draw_catalog_search_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter search query...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from(" Search Catalog ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("[Enter] Search  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Catalog Search ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);

    f.render_widget(popup, area);
}

/// Draw modlist name input overlay
fn draw_modlist_name_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);

    let input_text = if state.input_buffer.is_empty() {
        "Enter modlist name...".to_string()
    } else {
        state.input_buffer.clone()
    };

    let text = vec![
        Line::from(""),
        Line::from(" New Modlist ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(Span::styled(
            input_text,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("[Enter] Create  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Create Modlist ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);

    f.render_widget(popup, area);
}

fn draw_modlist_add_catalog_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 30, f.area());
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(" Add From Nexus Catalog ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Enter a Nexus mod ID or search term:"),
        Line::from(Span::styled(
            format!("> {}", state.input_buffer),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Examples: 266, skyui"),
        Line::from(""),
        Line::from("[Enter] Add first match  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Modlist Catalog Add ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);

    f.render_widget(popup, area);
}

fn draw_modlist_add_directory_input(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 30, f.area());
    f.render_widget(Clear, area);

    let text = vec![
        Line::from(""),
        Line::from(" Add From Local Directory ").style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Directory to scan recursively for archives (.zip/.7z/.rar):"),
        Line::from(Span::styled(
            format!("> {}", state.input_buffer),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from("Archive filenames are added as modlist entries."),
        Line::from(""),
        Line::from("[Enter] Scan and add  [Esc] Cancel"),
    ];

    let popup = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Modlist Local Add ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .alignment(Alignment::Center);

    f.render_widget(popup, area);
}

/// Draw the modlist editor screen
fn draw_modlist_editor_screen(f: &mut Frame, state: &AppState, area: Rect) {
    use crate::app::state::ModlistEditorMode;
    let guided = state.ui_mode == UiMode::Guided;

    match state.modlist_editor_mode {
        ModlistEditorMode::ListPicker => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Title
                    Constraint::Min(5),    // List
                    Constraint::Length(3),  // Help
                ])
                .split(area);

            // Title
            let title_text = if state.modlist_picker_for_loading {
                "Load Saved Modlist"
            } else {
                "Saved Modlists"
            };
            let title = Paragraph::new(title_text)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            // Modlist list
            if state.saved_modlists.is_empty() {
                let empty = Paragraph::new("No saved modlists. Import a modlist via F5 or press 'n' to create one.")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title(" Modlists "));
                f.render_widget(empty, chunks[1]);
            } else {
                let items: Vec<ListItem> = state.saved_modlists
                    .iter()
                    .enumerate()
                    .map(|(i, ml)| {
                        let style = if i == state.selected_saved_modlist_index {
                            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        let line = if guided {
                            format!(" {}", ml.name)
                        } else {
                            let desc = ml.description.as_deref().unwrap_or("");
                            let source = ml.source_file.as_deref().unwrap_or("manual");
                            format!(" {} - {} (from: {})", ml.name, desc, source)
                        };
                        ListItem::new(line).style(style)
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default()
                        .title(format!(" Modlists ({}) ", state.saved_modlists.len()))
                        .borders(Borders::ALL))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD));

                let mut list_state = ratatui::widgets::ListState::default();
                list_state.select(Some(state.selected_saved_modlist_index));
                f.render_stateful_widget(list, chunks[1], &mut list_state);
            }

            // Help
            let guided = state.ui_mode == UiMode::Guided;
            let help_text = if state.modlist_picker_for_loading {
                if guided {
                    "[Enter] Load | [l] Review/Queue | [a] Activate | [n] New | [d] Delete | [x] Export | z:Advanced"
                } else {
                    "[Enter] Load | [l] Review/Queue | [a] Activate | [x] Export | [f] File path | [n] New | [d] Delete | [r] Rename | Esc: Back | q: Quit"
                }
            } else {
                if guided {
                    "[Enter] Open | [l] Review/Queue | [a] Activate | [n] New | [d] Delete | [x] Export | z:Advanced"
                } else {
                    "[Enter] Open | [l] Review/Queue | [a] Activate | [x] Export | [n] New | [d] Delete | [r] Rename | Esc: Back | q: Quit"
                }
            };
            let help = Paragraph::new(help_text)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
        ModlistEditorMode::EntryEditor => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Title
                    Constraint::Min(5),    // Content
                    Constraint::Length(3),  // Help
                ])
                .split(area);

            // Title
            let modlist_name = state.saved_modlists
                .iter()
                .find(|ml| ml.id == state.active_modlist_id)
                .map(|ml| ml.name.as_str())
                .unwrap_or("Unknown");

            let title = Paragraph::new(format!("Editing: {}", modlist_name))
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            // Content: Guided = single list, Advanced = list + details
            let content_chunks = if guided {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(10)])
                    .split(chunks[1])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(chunks[1])
            };

            // Entry list
            let entries = &state.modlist_editor_entries;
            if entries.is_empty() {
                let empty = Paragraph::new("No entries in this modlist.")
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title(" Entries "));
                f.render_widget(empty, content_chunks[0]);
            } else {
                let items: Vec<ListItem> = entries
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| {
                        let style = if i == state.selected_modlist_editor_index {
                            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        let status = if entry.enabled { "[*]" } else { "[ ]" };
                        let nexus_id = entry.nexus_mod_id
                            .map(|id| format!(" ({})", id))
                            .unwrap_or_default();
                        ListItem::new(format!(" {} {}{}", status, entry.name, nexus_id)).style(style)
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::default()
                        .title(format!(" Entries ({}) ", entries.len()))
                        .borders(Borders::ALL))
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD));

                let mut list_state = ratatui::widgets::ListState::default();
                list_state.select(Some(state.selected_modlist_editor_index));
                f.render_stateful_widget(list, content_chunks[0], &mut list_state);
            }

            // Details panel (Advanced only)
            if !guided {
                if let Some(entry) = entries.get(state.selected_modlist_editor_index) {
                    let confidence = entry.match_confidence
                        .map(|c| format!("{:.0}%", c * 100.0))
                        .unwrap_or_else(|| "N/A".to_string());

                    let details = vec![
                        Line::from(Span::styled(&entry.name, Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))),
                        Line::from(""),
                        Line::from(format!("Enabled:    {}", if entry.enabled { "Yes" } else { "No" })),
                        Line::from(format!("Position:   {}", entry.position)),
                        Line::from(format!("Nexus ID:   {}", entry.nexus_mod_id.map(|id| id.to_string()).unwrap_or_else(|| "None".to_string()))),
                        Line::from(format!("Plugin:     {}", entry.plugin_name.as_deref().unwrap_or("None"))),
                        Line::from(format!("Confidence: {}", confidence)),
                        Line::from(format!("Author:     {}", entry.author.as_deref().unwrap_or("Unknown"))),
                        Line::from(format!("Version:    {}", entry.version.as_deref().unwrap_or("Unknown"))),
                    ];

                    let detail_widget = Paragraph::new(details)
                        .block(Block::default().title(" Entry Details ").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });
                    f.render_widget(detail_widget, content_chunks[1]);
                } else {
                    let empty_details = Paragraph::new("Select an entry to view details")
                        .alignment(Alignment::Center)
                        .block(Block::default().title(" Entry Details ").borders(Borders::ALL));
                    f.render_widget(empty_details, content_chunks[1]);
                }
            }

            // Help
            let help_text = if state.ui_mode == UiMode::Guided {
                "[Space] Toggle | [d] Delete | [J/K] Reorder | [i] Add Installed | [s] Save | [x] Export | [a] Activate | z:Advanced"
            } else {
                "[Space] Toggle | [d] Delete | [J/K] Reorder | [i] Installed | [c] Catalog | [o] Local Dir | [s] Save | [x] Export | [a] Activate | Esc: Back"
            };
            let help = Paragraph::new(help_text)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        }
    }
}
