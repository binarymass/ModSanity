//! FOMOD installation wizard logic

use super::{ConditionEvaluator, ModuleConfig, Plugin};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Wizard state
#[derive(Debug, Clone)]
pub struct WizardState {
    /// Current step index
    pub current_step: usize,

    /// Selections for each step/group
    /// Key: (step_index, group_index), Value: set of selected plugin indices
    pub selections: HashMap<(usize, usize), HashSet<usize>>,

    /// Condition evaluator for dependency checking
    pub evaluator: ConditionEvaluator,
}

impl WizardState {
    pub fn new() -> Self {
        Self {
            current_step: 0,
            selections: HashMap::new(),
            evaluator: ConditionEvaluator::new(),
        }
    }

    /// Create wizard state with custom file checker
    pub fn with_file_checker<F>(file_checker: F) -> Self
    where
        F: Fn(&str) -> super::FileState + Send + Sync + 'static,
    {
        Self {
            current_step: 0,
            selections: HashMap::new(),
            evaluator: ConditionEvaluator::with_file_checker(file_checker),
        }
    }

    /// Get selections for a group
    pub fn get_selections(&self, step: usize, group: usize) -> HashSet<usize> {
        self.selections
            .get(&(step, group))
            .cloned()
            .unwrap_or_default()
    }

    /// Set selection for a group
    pub fn set_selection(&mut self, step: usize, group: usize, selections: HashSet<usize>) {
        self.selections.insert((step, group), selections);
    }

    /// Toggle a selection and update condition flags
    pub fn toggle_selection(
        &mut self,
        step: usize,
        group: usize,
        plugin_idx: usize,
        group_type: &str,
        plugin: &Plugin,
    ) {
        let key = (step, group);
        let selections = self.selections.entry(key).or_default();

        let was_selected = selections.contains(&plugin_idx);

        match group_type {
            "SelectExactlyOne" | "SelectAtMostOne" => {
                // Radio button behavior
                if was_selected && group_type == "SelectAtMostOne" {
                    selections.remove(&plugin_idx);
                } else {
                    selections.clear();
                    selections.insert(plugin_idx);
                }
            }
            "SelectAny" => {
                // Checkbox behavior
                if was_selected {
                    selections.remove(&plugin_idx);
                } else {
                    selections.insert(plugin_idx);
                }
            }
            "SelectAll" => {
                // All must be selected (informational only)
                selections.insert(plugin_idx);
            }
            _ => {
                // Default to checkbox
                if was_selected {
                    selections.remove(&plugin_idx);
                } else {
                    selections.insert(plugin_idx);
                }
            }
        }

        // Update condition flags if plugin is now selected
        let is_now_selected = selections.contains(&plugin_idx);
        if is_now_selected && !was_selected {
            // Set flags for newly selected plugin
            if let Some(cflags) = &plugin.condition_flags {
                for flag in &cflags.flags {
                    self.evaluator
                        .set_flag(flag.name.clone(), flag.value.clone());
                }
            }
        }
    }

    /// Check if a selection is valid for the group type
    pub fn is_valid_for_group(&self, step: usize, group: usize, group_type: &str) -> bool {
        let count = self.get_selections(step, group).len();

        match group_type {
            "SelectExactlyOne" => count == 1,
            "SelectAtMostOne" => count <= 1,
            "SelectAny" => true,
            "SelectAll" => true, // Will be validated differently
            _ => true,
        }
    }

    /// Get all files to install based on current selections
    pub fn get_files_to_install(&self, config: &ModuleConfig) -> Vec<FileInstruction> {
        let mut instructions = Vec::new();

        // Required files (always installed)
        if let Some(required) = &config.required_files {
            for file in &required.files {
                instructions.push(FileInstruction::File {
                    source: file.source.clone(),
                    destination: file.destination.clone(),
                    priority: file.priority,
                });
            }
            for folder in &required.folders {
                instructions.push(FileInstruction::Folder {
                    source: folder.source.clone(),
                    destination: folder.destination.clone(),
                    priority: folder.priority,
                });
            }
        }

        // Selected optional files
        for (step_idx, step) in config.install_steps.steps.iter().enumerate() {
            for (group_idx, group) in step.groups.groups.iter().enumerate() {
                let selections = self.get_selections(step_idx, group_idx);

                for plugin_idx in selections {
                    if let Some(plugin) = group.plugins.plugins.get(plugin_idx) {
                        // Add files from this plugin
                        if let Some(files) = &plugin.files {
                            for file in &files.files {
                                instructions.push(FileInstruction::File {
                                    source: file.source.clone(),
                                    destination: file.destination.clone(),
                                    priority: file.priority,
                                });
                            }
                            for folder in &files.folders {
                                instructions.push(FileInstruction::Folder {
                                    source: folder.source.clone(),
                                    destination: folder.destination.clone(),
                                    priority: folder.priority,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Conditional file installs
        if let Some(conditional) = &config.conditional_installs {
            if let Some(patterns) = &conditional.patterns {
                for pattern in &patterns.patterns {
                    // Check if pattern conditions are satisfied
                    if self.evaluator.evaluate_dependencies(&pattern.dependencies) {
                        // Add files from this pattern
                        if let Some(files) = &pattern.files {
                            for file in &files.files {
                                instructions.push(FileInstruction::File {
                                    source: file.source.clone(),
                                    destination: file.destination.clone(),
                                    priority: file.priority,
                                });
                            }
                            for folder in &files.folders {
                                instructions.push(FileInstruction::Folder {
                                    source: folder.source.clone(),
                                    destination: folder.destination.clone(),
                                    priority: folder.priority,
                                });
                            }
                        }
                    }
                }
            }
        }

        instructions
    }
}

/// File installation instruction
#[derive(Debug, Clone)]
pub enum FileInstruction {
    File {
        source: String,
        destination: String,
        priority: i32,
    },
    Folder {
        source: String,
        destination: String,
        priority: i32,
    },
}

impl FileInstruction {
    /// Execute this instruction
    pub fn execute(&self, mod_path: &Path, dest_path: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut installed = Vec::new();

        match self {
            FileInstruction::File {
                source,
                destination,
                ..
            } => {
                let src = mod_path.join(source);
                let dst = if destination.is_empty() {
                    dest_path.join(Path::new(source).file_name().unwrap_or_default())
                } else {
                    dest_path.join(destination)
                };

                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(&src, &dst)?;
                    installed.push(dst);
                }
            }
            FileInstruction::Folder {
                source,
                destination,
                ..
            } => {
                let src = mod_path.join(source);
                let dst = if destination.is_empty() {
                    dest_path.to_path_buf()
                } else {
                    dest_path.join(destination)
                };

                if src.exists() && src.is_dir() {
                    installed.extend(copy_dir_recursive(&src, &dst)?);
                }
            }
        }

        Ok(installed)
    }
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut installed = Vec::new();
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            installed.extend(copy_dir_recursive(&src_path, &dst_path)?);
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            installed.push(dst_path);
        }
    }

    Ok(installed)
}

/// Initialize wizard with default selections
pub fn init_wizard_state(config: &ModuleConfig) -> WizardState {
    let mut state = WizardState::new();

    for (step_idx, step) in config.install_steps.steps.iter().enumerate() {
        for (group_idx, group) in step.groups.groups.iter().enumerate() {
            let mut selections = HashSet::new();

            for (plugin_idx, plugin) in group.plugins.plugins.iter().enumerate() {
                // Check if plugin is visible based on conditions
                if !state.evaluator.is_plugin_visible(plugin) {
                    continue;
                }

                // Get plugin type to determine if it should be selected by default
                let plugin_type = state.evaluator.get_plugin_type(plugin);
                let should_select = matches!(
                    plugin_type,
                    super::PluginType::Required | super::PluginType::Recommended
                );

                if should_select {
                    selections.insert(plugin_idx);
                    // Set flags for selected plugin
                    if let Some(cflags) = &plugin.condition_flags {
                        for flag in &cflags.flags {
                            state
                                .evaluator
                                .set_flag(flag.name.clone(), flag.value.clone());
                        }
                    }
                }
            }

            // For SelectExactlyOne, ensure at least first visible option is selected if none recommended
            if group.group_type == "SelectExactlyOne" && selections.is_empty() {
                for (plugin_idx, plugin) in group.plugins.plugins.iter().enumerate() {
                    if state.evaluator.is_plugin_visible(plugin) {
                        selections.insert(plugin_idx);
                        // Set flags for selected plugin
                        if let Some(cflags) = &plugin.condition_flags {
                            for flag in &cflags.flags {
                                state
                                    .evaluator
                                    .set_flag(flag.name.clone(), flag.value.clone());
                            }
                        }
                        break;
                    }
                }
            }

            // For SelectAll, select all visible options
            if group.group_type == "SelectAll" {
                for (plugin_idx, plugin) in group.plugins.plugins.iter().enumerate() {
                    if state.evaluator.is_plugin_visible(plugin) {
                        selections.insert(plugin_idx);
                        // Set flags for selected plugin
                        if let Some(cflags) = &plugin.condition_flags {
                            for flag in &cflags.flags {
                                state
                                    .evaluator
                                    .set_flag(flag.name.clone(), flag.value.clone());
                            }
                        }
                    }
                }
            }

            if !selections.is_empty() {
                state.set_selection(step_idx, group_idx, selections);
            }
        }
    }

    state
}
