//! Main UI rendering

use super::screens;
use crate::app::{App, AppState, InputMode, Screen};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

/// Draw the main UI
pub fn draw(f: &mut Frame, app: &App, state: &AppState) {
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
        draw_help(f);
    }

    // Draw input overlays
    match state.input_mode {
        InputMode::ModInstallPath => draw_mod_install_input(f, state),
        InputMode::ProfileNameInput => draw_profile_name_input(f, state),
        InputMode::ModDirectoryInput => draw_mod_directory_input(f, state),
        InputMode::NexusApiKeyInput => draw_nexus_api_key_input(f, state),
        InputMode::FomodComponentSelection => draw_fomod_component_selection(f, state),
        InputMode::CollectionPath => draw_collection_input(f, state),
        InputMode::PluginPositionInput => draw_plugin_position_input(f, state),
        InputMode::ModSearch => draw_mod_search_input(f, state),
        InputMode::PluginSearch => draw_plugin_search_input(f, state),
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
    let title = format!(
        " ModSanity v{}  |  {} | {}/{} mods enabled ",
        env!("CARGO_PKG_VERSION"),
        game_name,
        mod_count,
        total_mods
    );

    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(header, area);
}

/// Draw the tab bar
fn draw_tabs(f: &mut Frame, state: &AppState, area: Rect) {
    let titles = vec!["F1 Mods", "F2 Plugins", "F3 Profiles", "F4 Settings"];
    let selected = match state.current_screen {
        Screen::Dashboard | Screen::Mods | Screen::ModDetails => 0,
        Screen::Plugins => 1,
        Screen::Profiles => 2,
        Screen::Settings => 3,
        Screen::GameSelect | Screen::FomodWizard | Screen::Collection | Screen::Browse | Screen::LoadOrder => 0,
    };

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
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

    f.render_widget(list, chunks[0]);

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
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38), // Categories sidebar (wider for longer names)
            Constraint::Percentage(65), // Mod list
            Constraint::Percentage(35), // Details
        ])
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
    let (mod_dir_display, api_key_display) = if let Ok(config) = app.config.try_read() {
        let mod_dir = config.tui.default_mod_directory
            .clone()
            .unwrap_or_else(|| "Not set".to_string());

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

        (mod_dir, api_key)
    } else {
        ("Loading...".to_string(), "Loading...".to_string())
    };

    let settings = vec![
        ("NexusMods API Key", api_key_display),
        ("Deployment Method", "Symlink (recommended)".to_string()),
        ("Backup Originals", "Yes".to_string()),
        ("Default Mod Directory", mod_dir_display),
        ("Game Selection", "Change active game".to_string()),
    ];

    let items: Vec<ListItem> = settings
        .iter()
        .enumerate()
        .map(|(i, (name, value))| {
            let style = if i == state.selected_setting_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(vec![
                Line::from(Span::styled(name.to_string(), style)),
                Line::from(Span::styled(
                    format!("  {}", value),
                    Style::default().fg(Color::DarkGray),
                )),
            ])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Settings ").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
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
    for (i, category) in state.categories.iter().enumerate() {
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

    f.render_widget(list, area);
}

/// Parse hex color string to ratatui Color
fn parse_color(hex: &str) -> Option<Color> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return None;
    }

    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;

    Some(Color::Rgb(r, g, b))
}

/// Draw footer with status and keybindings
fn draw_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let status = state.status_message.as_deref().unwrap_or("");

    let help_hint = match state.current_screen {
        Screen::GameSelect => "Enter:select  q:quit",
        Screen::Mods | Screen::Dashboard => {
            "/:search  j/k:nav  r:refresh  i:install  b:browse  o:load-order  Space:toggle  d:delete  D:deploy  ?:help  q:quit"
        }
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
                "/:search  Enter:reorder  j/k:nav  Space:toggle  a:enable-all  n:disable-all  s:save  S:auto-sort  L:loot-sort  ?:help  q:quit"
            }
        }
        Screen::Profiles => "j/k:nav  n:new  Enter:activate  d:delete  ?:help  q:quit",
        Screen::Settings => "j/k:nav  Enter:edit  Esc:back  ?:help  q:quit",
        Screen::Collection => "j/k:nav  i:install  a:install-all  Esc:back  ?:help  q:quit",
        Screen::Browse => "s:search  f:sort  n/p:page  j/k:nav  Enter:select-file  Esc:back  ?:help  q:quit",
        Screen::ModDetails => "j/k:scroll  Esc:back  ?:help  q:quit",
        Screen::FomodWizard => "j/k:nav  Space:select  Enter:continue  b:back  Esc:cancel  ?:help",
        _ => "?:help  Esc:back  q:quit",
    };

    let footer_text = if !status.is_empty() {
        format!(" {} | {}", status, help_hint)
    } else {
        format!(" {}", help_hint)
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
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
fn draw_help(f: &mut Frame) {
    let area = centered_rect(70, 95, f.area());

    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Navigation:"),
        Line::from("  F1-F5       Switch tabs"),
        Line::from("  j/Down      Move down"),
        Line::from("  k/Up        Move up"),
        Line::from("  PgDn        Jump down 10 items"),
        Line::from("  PgUp        Jump up 10 items"),
        Line::from("  Enter       Select/confirm"),
        Line::from("  Esc         Go back"),
        Line::from("  g           Game selection"),
        Line::from(""),
        Line::from("Mod Management:"),
        Line::from("  /        Search mods by name"),
        Line::from("  Space/e  Enable/disable selected mod"),
        Line::from("  a        Enable all mods"),
        Line::from("  n        Disable all mods"),
        Line::from("  +/-      Change priority"),
        Line::from("  r        Refresh mod list"),
        Line::from("  i        Install from file path"),
        Line::from("  I        Bulk install from default directory"),
        Line::from("  l        Load from Downloads folder"),
        Line::from("  f        Reconfigure FOMOD installer for selected mod"),
        Line::from("  R        Rescan mods directory and rebuild database"),
        Line::from("  C        Load Nexus Mods collection"),
        Line::from("  b        Browse/search Nexus Mods (requires API key)"),
        Line::from("  U        Check for mod updates (requires API key)"),
        Line::from("  x        Check requirements for selected mod (requires API key)"),
        Line::from("  d/Del    Delete selected mod"),
        Line::from("  D        Deploy all enabled mods"),
        Line::from("  o        Open Load Order screen"),
        Line::from(""),
        Line::from("Categories:"),
        Line::from("  Left/Right  Navigate categories"),
        Line::from("  c           Assign category to mod (cycle)"),
        Line::from("  A           Auto-categorize uncategorized mods"),
        Line::from("  Shift+A     Force recategorize ALL mods"),
        Line::from("  s           Auto-sort by category"),
        Line::from(""),
        Line::from("Browse/Search Nexus Mods (press 'b' from Mods):"),
        Line::from("  s           Start search"),
        Line::from("  f           Cycle sort (Relevance/Downloads/Endorsements/Updated)"),
        Line::from("  n/p         Next/previous page"),
        Line::from("  PgDn/PgUp   Next/previous page"),
        Line::from("  j/k         Navigate results"),
        Line::from("  Enter       Select mod and choose file to download"),
        Line::from("  Esc         Return to Mods screen"),
        Line::from(""),
        Line::from("Load Order (press 'o' from Mods):"),
        Line::from("  Enter       Toggle reorder mode"),
        Line::from("  j/k         Move mod (reorder) / Navigate"),
        Line::from("  J/K         Jump 5 positions"),
        Line::from("  t/b         Move to top/bottom"),
        Line::from("  s           Save load order"),
        Line::from("  S           Auto-sort by category"),
        Line::from(""),
        Line::from("Plugin Management (F2):"),
        Line::from("  /           Search plugins by filename"),
        Line::from("  Enter       Toggle reorder mode"),
        Line::from("  j/k         Move plugin (in reorder mode)"),
        Line::from("  #           Go to specific position (in reorder mode)"),
        Line::from("  Space       Toggle plugin enabled"),
        Line::from("  a           Enable all plugins"),
        Line::from("  n           Disable all plugins"),
        Line::from("  s           Save plugin load order"),
        Line::from("  S           Auto-sort (native Rust)"),
        Line::from("  L           Auto-sort (LOOT CLI)"),
        Line::from(""),
        Line::from("General:"),
        Line::from("  ?        Toggle help"),
        Line::from("  q/Ctrl+C Quit"),
    ];

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

        f.render_widget(list, result_chunks[0]);
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

    f.render_widget(list, area);
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
