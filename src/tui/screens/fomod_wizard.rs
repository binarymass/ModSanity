//! FOMOD installation wizard UI
//!
//! Full-featured wizard for FOMOD installers with multi-step navigation,
//! live validation, and visual feedback.

use crate::app::state::{AppState, FomodWizardState, WizardPhase};
use crate::mods::fomod::{validation, ConflictSeverity, InstallPlan, InstallerValidator, PluginType};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Draw the FOMOD wizard screen
pub fn draw_fomod_wizard(f: &mut Frame, state: &AppState, area: Rect) {
    let wizard_state = match &state.fomod_wizard_state {
        Some(ws) => ws,
        None => {
            // No wizard state - show error
            let block = Block::default()
                .title("FOMOD Wizard")
                .borders(Borders::ALL);
            let text = Paragraph::new("No FOMOD wizard state available")
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(text, area);
            return;
        }
    };

    match wizard_state.phase {
        WizardPhase::Overview => draw_overview(f, wizard_state, area),
        WizardPhase::StepNavigation => draw_step_navigation(f, wizard_state, area),
        WizardPhase::Summary => draw_summary(f, wizard_state, area),
        WizardPhase::Confirm => draw_confirm(f, wizard_state, area),
    }
}

/// Draw the overview phase (mod info)
fn draw_overview(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    let block = Block::default()
        .title(format!(" FOMOD Installer: {} ", wizard_state.mod_name))
        .borders(Borders::ALL)
        .style(Style::default());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let config = &wizard_state.installer.config;

    let mut lines = vec![
        Line::from(Span::styled(
            config.module_name.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Module image (if available)
    if let Some(image) = &config.module_image {
        lines.push(Line::from(Span::styled(
            "Module Image:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", image.path),
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(""));
    }

    // Add step count
    let step_count = wizard_state.installer.steps().len();
    lines.push(Line::from(format!("Installation Steps: {}", step_count)));
    lines.push(Line::from(""));

    // Add step names
    if step_count > 0 {
        lines.push(Line::from(Span::styled(
            "Steps:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for (i, step) in wizard_state.installer.steps().iter().enumerate() {
            lines.push(Line::from(format!("  {}. {}", i + 1, step.name)));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Enter to continue, ? for help, Esc to cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let text = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    f.render_widget(text, inner);
}

/// Draw the step navigation phase (main wizard UI)
fn draw_step_navigation(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    // 3-column layout: Steps (20%) | Options (50%) | Details (30%)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(50),
            Constraint::Percentage(30),
        ])
        .split(area);

    draw_step_list(f, wizard_state, chunks[0]);
    draw_option_groups(f, wizard_state, chunks[1]);
    draw_option_details(f, wizard_state, chunks[2]);
}

/// Draw the step list (left panel)
fn draw_step_list(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    let steps: Vec<ListItem> = wizard_state
        .installer
        .steps()
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let style = if i == wizard_state.current_step {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if i == wizard_state.current_step {
                "▶ "
            } else if i < wizard_state.current_step {
                "✓ "
            } else {
                "  "
            };

            ListItem::new(format!("{}{}", prefix, step.name)).style(style)
        })
        .collect();

    let list = List::new(steps).block(
        Block::default()
            .title(" Installation Steps ")
            .borders(Borders::ALL),
    );

    f.render_widget(list, area);
}

/// Draw option groups (middle panel)
fn draw_option_groups(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    let step = match wizard_state.current_install_step() {
        Some(s) => s,
        None => {
            let text = Paragraph::new("No step selected")
                .block(Block::default().title(" Options ").borders(Borders::ALL));
            f.render_widget(text, area);
            return;
        }
    };

    // Split area for each group
    let group_count = step.groups.groups.len();
    if group_count == 0 {
        let text = Paragraph::new("No options in this step")
            .block(Block::default().title(" Options ").borders(Borders::ALL));
        f.render_widget(text, area);
        return;
    }

    // For now, show current group only (can enhance to show all groups later)
    let group = match step.groups.groups.get(wizard_state.current_group) {
        Some(g) => g,
        None => {
            let text = Paragraph::new("No group selected")
                .block(Block::default().title(" Options ").borders(Borders::ALL));
            f.render_widget(text, area);
            return;
        }
    };

    // Get current selections
    let selections = wizard_state
        .wizard
        .get_selections(wizard_state.current_step, wizard_state.current_group);

    // Build option list
    let items: Vec<ListItem> = group
        .plugins
        .plugins
        .iter()
        .enumerate()
        .map(|(i, plugin)| {
            let is_selected = selections.contains(&i);
            let is_current = i == wizard_state.selected_option;

            // Get plugin type from evaluator
            let plugin_type = wizard_state.wizard.evaluator.get_plugin_type(plugin);
            let is_visible = wizard_state.wizard.evaluator.is_plugin_visible(plugin);

            // Determine color based on type
            let color = if !is_visible {
                Color::DarkGray
            } else {
                match plugin_type {
                    PluginType::Required => Color::Red,
                    PluginType::Recommended => Color::Yellow,
                    PluginType::Optional => Color::White,
                    PluginType::NotUsable => Color::DarkGray,
                    PluginType::CouldBeUsable => Color::Gray,
                }
            };

            // Selection indicator
            let prefix = match &group.group_type[..] {
                "SelectExactlyOne" | "SelectAtMostOne" => {
                    // Radio button
                    if is_selected {
                        "◉ "
                    } else {
                        "○ "
                    }
                }
                _ => {
                    // Checkbox
                    if is_selected {
                        "[✓] "
                    } else {
                        "[ ] "
                    }
                }
            };

            let mut style = Style::default().fg(color);
            if is_current {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if is_selected {
                style = style.add_modifier(Modifier::BOLD);
            }

            ListItem::new(format!("{}{}", prefix, plugin.name)).style(style)
        })
        .collect();

    // Validation indicator
    let group_validation =
        validation::validate_group(&group, &selections, wizard_state.current_step, wizard_state.current_group);
    let validation_indicator = if group_validation.is_ok() {
        "✓"
    } else {
        "✗"
    };

    let title = format!(
        " {} {} ({}) {} ",
        validation_indicator, group.name, group.group_type, validation_indicator
    );

    let list = List::new(items).block(Block::default().title(title).borders(Borders::ALL));

    f.render_widget(list, area);
}

/// Draw option details (right panel)
fn draw_option_details(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    let plugin = wizard_state
        .current_option_group()
        .and_then(|group| group.plugins.plugins.get(wizard_state.selected_option));

    let block = Block::default()
        .title(" Option Details ")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let plugin = match plugin {
        Some(p) => p,
        None => {
            let text = Paragraph::new("No option selected");
            f.render_widget(text, inner);
            return;
        }
    };

    let mut lines = vec![
        Line::from(Span::styled(
            plugin.name.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Plugin type
    let plugin_type = wizard_state.wizard.evaluator.get_plugin_type(plugin);
    let type_str = match plugin_type {
        PluginType::Required => "Required",
        PluginType::Recommended => "Recommended",
        PluginType::Optional => "Optional",
        PluginType::NotUsable => "Not Usable",
        PluginType::CouldBeUsable => "Could Be Usable",
    };
    let type_color = match plugin_type {
        PluginType::Required => Color::Red,
        PluginType::Recommended => Color::Yellow,
        _ => Color::White,
    };
    lines.push(Line::from(Span::styled(
        format!("Type: {}", type_str),
        Style::default().fg(type_color),
    )));
    lines.push(Line::from(""));

    // Description
    if !plugin.description.is_empty() {
        lines.push(Line::from(Span::styled(
            "Description:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(plugin.description.as_str()));
        lines.push(Line::from(""));
    }

    // Image path (if available)
    if let Some(image) = &plugin.image {
        lines.push(Line::from(Span::styled(
            "Image:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", image.path),
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(Span::styled(
            "  (Press 'i' to open in external viewer)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    // File count
    if let Some(files) = &plugin.files {
        let file_count = files.files.len() + files.folders.len();
        lines.push(Line::from(format!("Files: {}", file_count)));
    }

    // Keyboard shortcuts
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "━━━ Keyboard Shortcuts ━━━",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "j/k or ↓/↑   Navigate options",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "Space        Toggle selection",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "Tab          Next group",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "Enter        Next step/confirm",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "b            Back to previous step",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "p            Preview install plan",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "Esc          Cancel installation",
        Style::default().fg(Color::DarkGray),
    )));

    let text = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(text, inner);
}

/// Draw the summary phase
fn draw_summary(f: &mut Frame, wizard_state: &FomodWizardState, area: Rect) {
    let block = Block::default()
        .title(" Installation Summary ")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into two columns: selections (60%) and file preview (40%)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(inner);

    // Left panel: Selected options
    let mut selection_lines = vec![
        Line::from(Span::styled(
            "Selected Options:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Show all selections
    for (step_idx, step) in wizard_state.installer.steps().iter().enumerate() {
        selection_lines.push(Line::from(Span::styled(
            step.name.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for (group_idx, group) in step.groups.groups.iter().enumerate() {
            let selections = wizard_state.wizard.get_selections(step_idx, group_idx);
            if selections.is_empty() {
                continue;
            }

            selection_lines.push(Line::from(format!("  {}:", group.name)));

            for plugin_idx in selections {
                if let Some(plugin) = group.plugins.plugins.get(plugin_idx) {
                    selection_lines.push(Line::from(format!("    • {}", plugin.name)));
                }
            }
        }
        selection_lines.push(Line::from(""));
    }

    selection_lines.push(Line::from(""));
    selection_lines.push(Line::from(Span::styled(
        "Press Enter to install, b to go back, Esc to cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let selections_text = Paragraph::new(selection_lines).wrap(Wrap { trim: true });
    f.render_widget(selections_text, chunks[0]);

    // Right panel: File preview and stats
    let file_instructions = wizard_state
        .wizard
        .get_files_to_install(&wizard_state.installer.config);

    let mut file_lines = vec![
        Line::from(Span::styled(
            "Installation Details:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Files to Install: {}", file_instructions.len()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Show sample file paths (first 10)
    if !file_instructions.is_empty() {
        file_lines.push(Line::from(Span::styled(
            "Sample Files:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for (i, instruction) in file_instructions.iter().take(10).enumerate() {
            let (source, _dest) = match instruction {
                crate::mods::fomod::FileInstruction::File {
                    source,
                    destination,
                    ..
                } => (source, destination),
                crate::mods::fomod::FileInstruction::Folder {
                    source,
                    destination,
                    ..
                } => (source, destination),
            };

            file_lines.push(Line::from(format!("  {}", source)));

            if i == 9 && file_instructions.len() > 10 {
                file_lines.push(Line::from(format!(
                    "  ... and {} more",
                    file_instructions.len() - 10
                )));
            }
        }
    }

    let file_text = Paragraph::new(file_lines).wrap(Wrap { trim: true });
    f.render_widget(file_text, chunks[1]);
}

/// Draw the confirm phase
fn draw_confirm(f: &mut Frame, _wizard_state: &FomodWizardState, area: Rect) {
    let block = Block::default()
        .title(" Confirm Installation ")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let text = Paragraph::new("Confirm installation...")
        .alignment(Alignment::Center);

    f.render_widget(text, inner);
}
