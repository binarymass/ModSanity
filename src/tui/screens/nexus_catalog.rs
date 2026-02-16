//! Nexus Catalog TUI screen

use crate::app::state::{AppState, CatalogProgress, CatalogSyncStatus, InputMode};
use crate::app::App;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Render the Nexus Catalog screen
pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    // If catalog is populated and we have browse results, show browse view
    let has_catalog = state
        .catalog_sync_state
        .as_ref()
        .map(|s| s.completed && s.total_mods > 0)
        .unwrap_or(false);

    if state.catalog_populating {
        render_status_view(f, area, state);
    } else if has_catalog || !state.catalog_browse_results.is_empty() {
        render_browse_view(f, area, state);
    } else {
        render_status_view(f, area, state);
    }
}

/// Original status/population view
fn render_status_view(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(10), // Status info
            Constraint::Min(5),     // Main content
            Constraint::Length(3),  // Help
        ])
        .split(area);

    // Title
    let title = Paragraph::new("Nexus Mods Catalog")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Status info
    render_status(f, chunks[1], state);

    // Main content
    if state.catalog_populating {
        render_progress(f, chunks[2], state);
    } else {
        render_info(f, chunks[2], state);
    }

    // Help text
    let help_text = if state.catalog_populating {
        "Population in progress... | Esc: Back"
    } else {
        "p: Populate | r: Reset | s: Refresh | Esc: Back | q: Quit"
    };

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[3]);
}

/// Browse view with searchable/scrollable mod list
fn render_browse_view(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Help
        ])
        .split(area);

    // Search bar
    let search_display = if state.catalog_search_query.is_empty() {
        format!(
            " Catalog: {} mods | /: Search | p: Populate | r: Reset",
            state.catalog_total_count
        )
    } else {
        format!(
            " Search: \"{}\" | {} results | /: New search | Esc: Clear search",
            state.catalog_search_query,
            state.catalog_browse_results.len()
        )
    };

    let search_bar = Paragraph::new(search_display)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Nexus Catalog "),
        );
    f.render_widget(search_bar, chunks[0]);

    // Main content: mod list (left) + details (right)
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Mod list
    let results = &state.catalog_browse_results;
    if results.is_empty() {
        let empty = Paragraph::new("No mods found. Press 'p' to populate catalog.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(" Mods "));
        f.render_widget(empty, content_chunks[0]);
    } else {
        let items: Vec<ListItem> = results
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let style = if i == state.selected_catalog_index {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let author = m.author.as_deref().unwrap_or("Unknown");
                ListItem::new(format!(" {} by {}", m.name, author)).style(style)
            })
            .collect();

        let page_start = state.catalog_browse_offset + 1;
        let page_end = state.catalog_browse_offset + results.len() as i64;
        let title = format!(
            " Mods ({}-{} of {}) ",
            page_start, page_end, state.catalog_total_count
        );

        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(state.selected_catalog_index));
        f.render_stateful_widget(list, content_chunks[0], &mut list_state);
    }

    // Details panel
    if let Some(m) = results.get(state.selected_catalog_index) {
        let updated = m
            .updated_time
            .map(|t| {
                chrono::DateTime::from_timestamp(t, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| t.to_string())
            })
            .unwrap_or_else(|| "Unknown".to_string());

        let details = vec![
            Line::from(Span::styled(
                &m.name,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(format!("Mod ID:  {}", m.mod_id)),
            Line::from(format!(
                "Author:  {}",
                m.author.as_deref().unwrap_or("Unknown")
            )),
            Line::from(format!("Updated: {}", updated)),
            Line::from(""),
            Line::from(Span::styled(
                "Summary:",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(m.summary.as_deref().unwrap_or("No summary available")),
        ];

        let detail_widget = Paragraph::new(details)
            .block(Block::default().title(" Details ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(detail_widget, content_chunks[1]);
    } else {
        let empty_details = Paragraph::new("Select a mod to view details")
            .alignment(Alignment::Center)
            .block(Block::default().title(" Details ").borders(Borders::ALL));
        f.render_widget(empty_details, content_chunks[1]);
    }

    // Help bar
    let help_text =
        "j/k: Navigate | /: Search | n/p: Next/Prev Page | r: Reset catalog | Esc: Back | q: Quit";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help, chunks[2]);
}

fn render_status(f: &mut Frame, area: Rect, state: &AppState) {
    let game_domain = if state.catalog_game_domain.is_empty() {
        "None selected".to_string()
    } else {
        state.catalog_game_domain.clone()
    };

    let status_text = if let Some(sync_state) = &state.catalog_sync_state {
        vec![
            Line::from(vec![
                Span::styled("Game: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&game_domain),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    if sync_state.completed {
                        "✓ Completed"
                    } else {
                        "In Progress"
                    },
                    Style::default().fg(if sync_state.completed {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Current Page: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(sync_state.current_page.to_string()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Total Mods: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(sync_state.total_mods.to_string()),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Game: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&game_domain),
            ]),
            Line::from("No sync data available. Press 'p' to populate catalog."),
        ]
    };

    let status = Paragraph::new(status_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Catalog Status"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(status, area);
}

fn render_progress(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Progress bar
            Constraint::Min(3),    // Details
        ])
        .split(area);

    if let Some(progress) = &state.catalog_progress {
        // Progress bar based on offset vs total count
        let percent = if progress.total_count > 0 {
            let progress_pct =
                (progress.current_offset as f64 / progress.total_count as f64) * 100.0;
            progress_pct.min(100.0) as u16
        } else {
            0
        };

        let label = if progress.total_count > 0 {
            format!(
                "{}/{} mods ({}%)",
                progress.current_offset, progress.total_count, percent
            )
        } else {
            "Fetching...".to_string()
        };

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Population Progress"),
            )
            .gauge_style(Style::default().fg(Color::Cyan))
            .percent(percent)
            .label(label);
        f.render_widget(gauge, chunks[0]);

        // Details
        let details_text = vec![
            Line::from(vec![
                Span::styled(
                    "Pages Fetched: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(progress.pages_fetched.to_string()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Mods Inserted: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(progress.mods_inserted.to_string()),
            ]),
            Line::from(vec![
                Span::styled(
                    "Mods Updated: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(progress.mods_updated.to_string()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Populating catalog... Please wait.",
                Style::default().fg(Color::Yellow),
            )),
        ];

        let details = Paragraph::new(details_text)
            .block(Block::default().borders(Borders::ALL).title("Details"))
            .wrap(Wrap { trim: true });
        f.render_widget(details, chunks[1]);
    }
}

fn render_info(f: &mut Frame, area: Rect, _state: &AppState) {
    let info_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Nexus Mods Catalog",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("The catalog allows you to cache mod listings from Nexus Mods locally"),
        Line::from("for faster searches and mod matching during imports."),
        Line::from(""),
        Line::from(vec![
            Span::styled("p", Style::default().fg(Color::Yellow)),
            Span::raw(" - Populate catalog (fetch mod listings)"),
        ]),
        Line::from(vec![
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" - Reset and repopulate from beginning"),
        ]),
        Line::from(vec![
            Span::styled("s", Style::default().fg(Color::Yellow)),
            Span::raw(" - Refresh status"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Note: Population may take several minutes for large game catalogs.",
            Style::default().fg(Color::Gray),
        )),
    ];

    let info = Paragraph::new(info_text)
        .block(Block::default().borders(Borders::ALL).title("Information"))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);
    f.render_widget(info, area);
}

/// Handle input for the Nexus Catalog screen
pub async fn handle_input(app: &mut App, key: KeyCode) -> Result<()> {
    let state = app.state.read().await;

    // Don't allow actions while populating
    if state.catalog_populating {
        if key == KeyCode::Esc {
            drop(state);
            let mut state = app.state.write().await;
            state.go_back();
        }
        return Ok(());
    }

    let has_catalog = state
        .catalog_sync_state
        .as_ref()
        .map(|s| s.completed && s.total_mods > 0)
        .unwrap_or(false);
    let has_browse = has_catalog || !state.catalog_browse_results.is_empty();
    let result_count = state.catalog_browse_results.len();
    drop(state);

    if has_browse {
        // Browse mode keys
        match key {
            KeyCode::Down | KeyCode::Char('j') => {
                let mut state = app.state.write().await;
                if result_count > 0 && state.selected_catalog_index < result_count - 1 {
                    state.selected_catalog_index += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let mut state = app.state.write().await;
                if state.selected_catalog_index > 0 {
                    state.selected_catalog_index -= 1;
                }
            }
            KeyCode::Char('n') => {
                // Next page
                let mut state = app.state.write().await;
                let new_offset = state.catalog_browse_offset + 100;
                if new_offset < state.catalog_total_count {
                    let game_domain = state.catalog_game_domain.clone();
                    let search_query = state.catalog_search_query.clone();
                    state.catalog_browse_offset = new_offset;
                    state.selected_catalog_index = 0;
                    drop(state);
                    load_catalog_page(app, &game_domain, new_offset, &search_query).await?;
                }
            }
            KeyCode::Char('/') => {
                let mut state = app.state.write().await;
                state.input_mode = InputMode::CatalogSearch;
                state.input_buffer = state.catalog_search_query.clone();
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                populate_catalog(app, false).await?;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                populate_catalog(app, true).await?;
            }
            KeyCode::Esc => {
                let mut state = app.state.write().await;
                if !state.catalog_search_query.is_empty() {
                    // Clear search and reload default page
                    state.catalog_search_query.clear();
                    state.catalog_browse_offset = 0;
                    state.selected_catalog_index = 0;
                    let game_domain = state.catalog_game_domain.clone();
                    drop(state);
                    load_catalog_page(app, &game_domain, 0, "").await?;
                } else {
                    state.go_back();
                }
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                let mut state = app.state.write().await;
                state.should_quit = true;
            }
            _ => {}
        }
    } else {
        // Status view keys
        match key {
            KeyCode::Char('p') | KeyCode::Char('P') => {
                populate_catalog(app, false).await?;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                populate_catalog(app, true).await?;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                refresh_status(app).await?;
            }
            KeyCode::Esc => {
                let mut state = app.state.write().await;
                state.go_back();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                let mut state = app.state.write().await;
                state.should_quit = true;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Load a page of catalog mods (or search results)
pub async fn load_catalog_page(
    app: &mut App,
    game_domain: &str,
    offset: i64,
    search_query: &str,
) -> Result<()> {
    let results = if search_query.is_empty() {
        app.db.list_catalog_mods(game_domain, offset, 100)?
    } else {
        app.db.search_catalog(game_domain, search_query, 100)?
    };

    let total = if search_query.is_empty() {
        app.db.count_catalog_mods(game_domain)?
    } else {
        results.len() as i64
    };

    let mut state = app.state.write().await;
    state.catalog_browse_results = results;
    state.catalog_total_count = total;
    state.catalog_browse_offset = offset;
    Ok(())
}

async fn populate_catalog(app: &mut App, reset: bool) -> Result<()> {
    use crate::nexus::{CatalogPopulator, NexusRestClient, PopulateOptions};

    // Get game domain
    let game = match app.active_game().await {
        Some(g) => g,
        None => {
            let mut state = app.state.write().await;
            state.error_message = Some("No game selected".to_string());
            return Ok(());
        }
    };

    let game_domain = match game.id.as_str() {
        "skyrimse" | "skyrimvr" => "skyrimspecialedition",
        id => id,
    };

    // Get API key
    let api_key = match &app.config.read().await.nexus_api_key {
        Some(key) => key.clone(),
        None => {
            let mut state = app.state.write().await;
            state.error_message = Some("Nexus API key not configured".to_string());
            return Ok(());
        }
    };

    // Set populating state
    {
        let mut state = app.state.write().await;
        state.catalog_populating = true;
        state.catalog_game_domain = game_domain.to_string();
        state.catalog_progress = Some(CatalogProgress {
            pages_fetched: 0,
            mods_inserted: 0,
            mods_updated: 0,
            current_page: 0,
            total_count: 0,
            current_offset: 0,
        });
    }

    // Spawn population task
    let db = app.db.clone();
    let state_clone = app.state.clone();
    let game_domain = game_domain.to_string();

    tokio::spawn(async move {
        let result: Result<()> = async {
            let rest_client = NexusRestClient::new(&api_key)?;
            let populator = CatalogPopulator::new(db.clone(), rest_client, game_domain.clone())?;

            let options = PopulateOptions {
                reset,
                per_page: 100,
                max_pages: None,
                delay_between_pages_ms: 500,
            };

            // Create progress callback to update state
            let state_for_callback = state_clone.clone();
            let callback =
                move |pages: i32, inserted: i64, updated: i64, total: i64, offset: i32| {
                    if let Ok(mut state) = state_for_callback.try_write() {
                        state.catalog_progress = Some(CatalogProgress {
                            pages_fetched: pages,
                            mods_inserted: inserted,
                            mods_updated: updated,
                            current_page: pages + 1,
                            total_count: total,
                            current_offset: offset,
                        });
                    }
                };

            let stats = populator.populate(options, Some(callback)).await?;

            // Update final state
            let mut state = state_clone.write().await;
            state.catalog_populating = false;
            state.catalog_progress = None;
            state.set_status(format!(
                "Catalog populated: {} pages, {} total mods",
                stats.pages_fetched, stats.total_mods
            ));

            // Refresh status and load initial browse page
            if let Ok(sync_state) = db.get_sync_state(&game_domain) {
                let total_mods = db.count_catalog_mods(&game_domain).unwrap_or(0);
                state.catalog_sync_state = Some(CatalogSyncStatus {
                    current_page: sync_state.current_page,
                    completed: sync_state.completed,
                    last_sync: sync_state.last_sync,
                    last_error: sync_state.last_error,
                    total_mods,
                });
                state.catalog_total_count = total_mods;
            }

            // Load first page of browse results
            if let Ok(results) = db.list_catalog_mods(&game_domain, 0, 100) {
                state.catalog_browse_results = results;
                state.catalog_browse_offset = 0;
                state.selected_catalog_index = 0;
            }

            Ok(())
        }
        .await;

        if let Err(e) = result {
            let mut state = state_clone.write().await;
            state.catalog_populating = false;
            state.catalog_progress = None;
            state.error_message = Some(format!("Population failed: {}", e));
        }
    });

    Ok(())
}

async fn refresh_status(app: &mut App) -> Result<()> {
    let game = match app.active_game().await {
        Some(g) => g,
        None => return Ok(()),
    };

    let game_domain = match game.id.as_str() {
        "skyrimse" | "skyrimvr" => "skyrimspecialedition",
        id => id,
    };

    if let Ok(sync_state) = app.db.get_sync_state(game_domain) {
        let total_mods = app.db.count_catalog_mods(game_domain)?;

        let mut state = app.state.write().await;
        state.catalog_game_domain = game_domain.to_string();
        state.catalog_sync_state = Some(CatalogSyncStatus {
            current_page: sync_state.current_page,
            completed: sync_state.completed,
            last_sync: sync_state.last_sync,
            last_error: sync_state.last_error,
            total_mods,
        });
        state.set_status("Status refreshed");
    }

    Ok(())
}
