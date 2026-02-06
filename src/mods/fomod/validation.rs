//! FOMOD group validation logic

use super::OptionGroup;
use std::collections::HashSet;

/// Validate a group's selections against its constraints
pub fn validate_group(
    group: &OptionGroup,
    selections: &HashSet<usize>,
    _step_idx: usize,
    _group_idx: usize,
) -> Result<(), String> {
    let count = selections.len();
    let group_type = &group.group_type;

    match group_type.as_str() {
        "SelectExactlyOne" => {
            if count != 1 {
                return Err(format!(
                    "Group '{}' requires exactly 1 selection (currently: {})",
                    group.name, count
                ));
            }
        }
        "SelectAtMostOne" => {
            if count > 1 {
                return Err(format!(
                    "Group '{}' allows at most 1 selection (currently: {})",
                    group.name, count
                ));
            }
        }
        "SelectAtLeastOne" => {
            if count < 1 {
                return Err(format!(
                    "Group '{}' requires at least 1 selection (currently: {})",
                    group.name, count
                ));
            }
        }
        "SelectAll" => {
            let total_plugins = group.plugins.plugins.len();
            if count != total_plugins {
                return Err(format!(
                    "Group '{}' requires all options to be selected ({}/{})",
                    group.name, count, total_plugins
                ));
            }
        }
        "SelectAny" => {
            // No constraints for SelectAny
        }
        _ => {
            // Unknown group type - treat as SelectAny (no constraints)
        }
    }

    Ok(())
}

/// Check if the wizard can proceed to the next step
pub fn can_proceed_to_next_step(wizard_state: &FomodWizardState) -> bool {
    wizard_state.validation_errors.is_empty()
}

/// Validate all groups in the current step
pub fn validate_current_step(wizard_state: &FomodWizardState) -> Vec<String> {
    let mut errors = Vec::new();

    if let Some(step) = wizard_state.current_install_step() {
        for (group_idx, group) in step.groups.groups.iter().enumerate() {
            let selections = wizard_state
                .wizard
                .get_selections(wizard_state.current_step, group_idx);

            if let Err(e) = validate_group(group, &selections, wizard_state.current_step, group_idx)
            {
                errors.push(e);
            }
        }
    }

    errors
}

// Import FomodWizardState for validation
use crate::app::state::FomodWizardState;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::fomod::{OptionGroup, PluginList};

    fn make_test_group(name: &str, group_type: &str, plugin_count: usize) -> OptionGroup {
        let mut plugins = Vec::new();
        for i in 0..plugin_count {
            plugins.push(crate::mods::fomod::Plugin {
                name: format!("Plugin {}", i),
                description: String::new(),
                image: None,
                files: None,
                condition_flags: None,
                type_descriptor: None,
            });
        }

        OptionGroup {
            name: name.to_string(),
            group_type: group_type.to_string(),
            plugins: PluginList {
                order: String::new(),
                plugins,
            },
        }
    }

    #[test]
    fn test_select_exactly_one() {
        let group = make_test_group("Test", "SelectExactlyOne", 3);

        // No selection - error
        let selections = HashSet::new();
        assert!(validate_group(&group, &selections, 0, 0).is_err());

        // One selection - OK
        let mut selections = HashSet::new();
        selections.insert(0);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // Two selections - error
        selections.insert(1);
        assert!(validate_group(&group, &selections, 0, 0).is_err());
    }

    #[test]
    fn test_select_at_most_one() {
        let group = make_test_group("Test", "SelectAtMostOne", 3);

        // No selection - OK
        let selections = HashSet::new();
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // One selection - OK
        let mut selections = HashSet::new();
        selections.insert(0);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // Two selections - error
        selections.insert(1);
        assert!(validate_group(&group, &selections, 0, 0).is_err());
    }

    #[test]
    fn test_select_at_least_one() {
        let group = make_test_group("Test", "SelectAtLeastOne", 3);

        // No selection - error
        let selections = HashSet::new();
        assert!(validate_group(&group, &selections, 0, 0).is_err());

        // One selection - OK
        let mut selections = HashSet::new();
        selections.insert(0);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // Two selections - OK
        selections.insert(1);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());
    }

    #[test]
    fn test_select_all() {
        let group = make_test_group("Test", "SelectAll", 3);

        // No selection - error
        let selections = HashSet::new();
        assert!(validate_group(&group, &selections, 0, 0).is_err());

        // Partial selection - error
        let mut selections = HashSet::new();
        selections.insert(0);
        assert!(validate_group(&group, &selections, 0, 0).is_err());

        // All selected - OK
        selections.insert(1);
        selections.insert(2);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());
    }

    #[test]
    fn test_select_any() {
        let group = make_test_group("Test", "SelectAny", 3);

        // No selection - OK
        let selections = HashSet::new();
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // One selection - OK
        let mut selections = HashSet::new();
        selections.insert(0);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());

        // All selected - OK
        selections.insert(1);
        selections.insert(2);
        assert!(validate_group(&group, &selections, 0, 0).is_ok());
    }
}
