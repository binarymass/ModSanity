//! FOMOD condition evaluation engine
//!
//! Evaluates boolean expressions for plugin visibility and dependencies.
//! Supports AND/OR/NOT logic, flag dependencies, and file dependencies.

use super::{Dependencies, Plugin, Pattern};
use std::collections::HashMap;

/// File states for dependency checking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// File/plugin is active
    Active,
    /// File/plugin exists but is inactive
    Inactive,
    /// File/plugin is missing
    Missing,
}

impl FileState {
    /// Parse from FOMOD XML attribute
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => FileState::Active,
            "inactive" => FileState::Inactive,
            "missing" => FileState::Missing,
            _ => FileState::Missing, // Default to Missing for unknown states
        }
    }
}

/// Plugin type determined by conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Must be selected
    Required,
    /// Should be selected (default)
    Recommended,
    /// Can be selected
    Optional,
    /// Cannot be used (grayed out)
    NotUsable,
    /// Could be used if dependencies met
    CouldBeUsable,
}

impl PluginType {
    /// Parse from FOMOD XML type name
    pub fn from_str(s: &str) -> Self {
        match s {
            "Required" => PluginType::Required,
            "Recommended" => PluginType::Recommended,
            "Optional" => PluginType::Optional,
            "NotUsable" => PluginType::NotUsable,
            "CouldBeUsable" => PluginType::CouldBeUsable,
            _ => PluginType::Optional,
        }
    }
}

/// Condition types for evaluation
#[derive(Debug, Clone)]
pub enum Condition {
    /// All conditions must be true
    And(Vec<Condition>),
    /// At least one condition must be true
    Or(Vec<Condition>),
    /// Condition must be false
    Not(Box<Condition>),
    /// Flag must have specific value
    FlagDependency { flag: String, value: String },
    /// File must have specific state
    FileDependency { file: String, state: FileState },
}

impl Condition {
    /// Build condition tree from Dependencies
    pub fn from_dependencies(deps: &Dependencies) -> Self {
        let mut conditions = Vec::new();

        // Add flag dependencies
        for flag_dep in &deps.flag_dependencies {
            conditions.push(Condition::FlagDependency {
                flag: flag_dep.flag.clone(),
                value: flag_dep.value.clone(),
            });
        }

        // Add file dependencies
        for file_dep in &deps.file_dependencies {
            conditions.push(Condition::FileDependency {
                file: file_dep.file.clone(),
                state: FileState::from_str(&file_dep.state),
            });
        }

        // If multiple dependencies, they are implicitly AND-ed
        if conditions.is_empty() {
            // No dependencies - always true
            Condition::And(vec![])
        } else if conditions.len() == 1 {
            conditions.into_iter().next().unwrap()
        } else {
            Condition::And(conditions)
        }
    }
}

/// Condition evaluator with current state
pub struct ConditionEvaluator {
    /// Current flag values
    flags: HashMap<String, String>,
    /// File state checker function
    file_checker: Box<dyn Fn(&str) -> FileState + Send + Sync>,
}

impl std::fmt::Debug for ConditionEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConditionEvaluator")
            .field("flags", &self.flags)
            .field("file_checker", &"<function>")
            .finish()
    }
}

impl Clone for ConditionEvaluator {
    fn clone(&self) -> Self {
        // Clone flags but create new default file checker
        // (function pointers can't be cloned)
        Self {
            flags: self.flags.clone(),
            file_checker: Box::new(|_| FileState::Missing),
        }
    }
}

impl ConditionEvaluator {
    /// Create new evaluator with default file checker
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            file_checker: Box::new(|_| FileState::Missing),
        }
    }

    /// Create evaluator with custom file checker
    pub fn with_file_checker<F>(file_checker: F) -> Self
    where
        F: Fn(&str) -> FileState + Send + Sync + 'static,
    {
        Self {
            flags: HashMap::new(),
            file_checker: Box::new(file_checker),
        }
    }

    /// Set a flag value
    pub fn set_flag(&mut self, name: String, value: String) {
        self.flags.insert(name, value);
    }

    /// Get a flag value
    pub fn get_flag(&self, name: &str) -> Option<&String> {
        self.flags.get(name)
    }

    /// Clear all flags
    pub fn clear_flags(&mut self) {
        self.flags.clear();
    }

    /// Evaluate a condition
    pub fn evaluate(&self, condition: &Condition) -> bool {
        match condition {
            Condition::And(conditions) => {
                // Empty AND is true (vacuous truth)
                if conditions.is_empty() {
                    return true;
                }
                conditions.iter().all(|c| self.evaluate(c))
            }
            Condition::Or(conditions) => {
                // Empty OR is false
                if conditions.is_empty() {
                    return false;
                }
                conditions.iter().any(|c| self.evaluate(c))
            }
            Condition::Not(condition) => !self.evaluate(condition),
            Condition::FlagDependency { flag, value } => {
                self.flags.get(flag).map(|v| v == value).unwrap_or(false)
            }
            Condition::FileDependency { file, state } => {
                let actual_state = (self.file_checker)(file);
                actual_state == *state
            }
        }
    }

    /// Evaluate optional Dependencies (None = always true)
    pub fn evaluate_dependencies(&self, deps: &Option<Dependencies>) -> bool {
        match deps {
            Some(deps) => {
                let condition = Condition::from_dependencies(deps);
                self.evaluate(&condition)
            }
            None => true, // No dependencies = always satisfied
        }
    }

    /// Check if a plugin is visible (dependencies met)
    pub fn is_plugin_visible(&self, plugin: &Plugin) -> bool {
        // Check if plugin has dependency-based type descriptor
        if let Some(td) = &plugin.type_descriptor {
            if let Some(dep_type) = &td.dependency_type {
                // Check patterns - plugin is visible if any pattern's dependencies are met
                if let Some(patterns) = &dep_type.patterns {
                    for pattern in &patterns.patterns {
                        if self.evaluate_dependencies(&pattern.dependencies) {
                            let pattern_type = PluginType::from_str(&pattern.pattern_type.name);
                            // Plugin is visible unless pattern makes it NotUsable
                            return pattern_type != PluginType::NotUsable;
                        }
                    }
                }
                // If no patterns match, use default type
                let default_type = PluginType::from_str(&dep_type.default_type.name);
                return default_type != PluginType::NotUsable;
            }
        }
        // No dependency type descriptor - always visible
        true
    }

    /// Get the plugin type based on current conditions
    pub fn get_plugin_type(&self, plugin: &Plugin) -> PluginType {
        // Check dependency-based type descriptor first
        if let Some(td) = &plugin.type_descriptor {
            if let Some(dep_type) = &td.dependency_type {
                // Check patterns in order - first match wins
                if let Some(patterns) = &dep_type.patterns {
                    for pattern in &patterns.patterns {
                        if self.evaluate_dependencies(&pattern.dependencies) {
                            return PluginType::from_str(&pattern.pattern_type.name);
                        }
                    }
                }
                // No pattern matched, use default type
                return PluginType::from_str(&dep_type.default_type.name);
            }

            // No dependency type, check simple default type
            if let Some(default_type) = &td.default_type {
                return PluginType::from_str(&default_type.name);
            }
        }

        // No type descriptor - default to Optional
        PluginType::Optional
    }

    /// Check if pattern dependencies are satisfied
    pub fn is_pattern_satisfied(&self, pattern: &Pattern) -> bool {
        self.evaluate_dependencies(&pattern.dependencies)
    }
}

impl Default for ConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_and_condition() {
        let evaluator = ConditionEvaluator::new();

        // Empty AND is true
        assert!(evaluator.evaluate(&Condition::And(vec![])));

        // Single condition
        let cond = Condition::And(vec![Condition::FlagDependency {
            flag: "test".to_string(),
            value: "true".to_string(),
        }]);
        assert!(!evaluator.evaluate(&cond));

        // Multiple conditions
        let cond = Condition::And(vec![
            Condition::FlagDependency {
                flag: "a".to_string(),
                value: "1".to_string(),
            },
            Condition::FlagDependency {
                flag: "b".to_string(),
                value: "2".to_string(),
            },
        ]);
        assert!(!evaluator.evaluate(&cond));
    }

    #[test]
    fn test_or_condition() {
        let evaluator = ConditionEvaluator::new();

        // Empty OR is false
        assert!(!evaluator.evaluate(&Condition::Or(vec![])));

        // Single false condition
        let cond = Condition::Or(vec![Condition::FlagDependency {
            flag: "test".to_string(),
            value: "true".to_string(),
        }]);
        assert!(!evaluator.evaluate(&cond));
    }

    #[test]
    fn test_not_condition() {
        let evaluator = ConditionEvaluator::new();

        let cond = Condition::Not(Box::new(Condition::FlagDependency {
            flag: "test".to_string(),
            value: "true".to_string(),
        }));
        assert!(evaluator.evaluate(&cond)); // Flag not set, so NOT true
    }

    #[test]
    fn test_flag_dependency() {
        let mut evaluator = ConditionEvaluator::new();

        let cond = Condition::FlagDependency {
            flag: "test".to_string(),
            value: "true".to_string(),
        };

        // Flag not set
        assert!(!evaluator.evaluate(&cond));

        // Flag set to wrong value
        evaluator.set_flag("test".to_string(), "false".to_string());
        assert!(!evaluator.evaluate(&cond));

        // Flag set to correct value
        evaluator.set_flag("test".to_string(), "true".to_string());
        assert!(evaluator.evaluate(&cond));
    }

    #[test]
    fn test_file_dependency() {
        let evaluator = ConditionEvaluator::with_file_checker(|file| {
            if file == "active.esp" {
                FileState::Active
            } else if file == "inactive.esp" {
                FileState::Inactive
            } else {
                FileState::Missing
            }
        });

        // Check active file
        let cond = Condition::FileDependency {
            file: "active.esp".to_string(),
            state: FileState::Active,
        };
        assert!(evaluator.evaluate(&cond));

        // Check inactive file
        let cond = Condition::FileDependency {
            file: "inactive.esp".to_string(),
            state: FileState::Inactive,
        };
        assert!(evaluator.evaluate(&cond));

        // Check missing file
        let cond = Condition::FileDependency {
            file: "missing.esp".to_string(),
            state: FileState::Missing,
        };
        assert!(evaluator.evaluate(&cond));

        // Wrong state check
        let cond = Condition::FileDependency {
            file: "active.esp".to_string(),
            state: FileState::Missing,
        };
        assert!(!evaluator.evaluate(&cond));
    }

    #[test]
    fn test_complex_condition() {
        let mut evaluator = ConditionEvaluator::new();
        evaluator.set_flag("a".to_string(), "1".to_string());
        evaluator.set_flag("b".to_string(), "2".to_string());

        // (a=1 AND b=2) OR c=3
        let cond = Condition::Or(vec![
            Condition::And(vec![
                Condition::FlagDependency {
                    flag: "a".to_string(),
                    value: "1".to_string(),
                },
                Condition::FlagDependency {
                    flag: "b".to_string(),
                    value: "2".to_string(),
                },
            ]),
            Condition::FlagDependency {
                flag: "c".to_string(),
                value: "3".to_string(),
            },
        ]);

        // Should be true because first branch is true
        assert!(evaluator.evaluate(&cond));

        // NOT (a=1 AND b=2)
        let cond = Condition::Not(Box::new(Condition::And(vec![
            Condition::FlagDependency {
                flag: "a".to_string(),
                value: "1".to_string(),
            },
            Condition::FlagDependency {
                flag: "b".to_string(),
                value: "2".to_string(),
            },
        ])));

        // Should be false because (a=1 AND b=2) is true
        assert!(!evaluator.evaluate(&cond));
    }

    #[test]
    fn test_plugin_type() {
        let evaluator = ConditionEvaluator::new();

        // Plugin with no type descriptor
        let plugin = Plugin {
            name: "Test".to_string(),
            description: String::new(),
            image: None,
            files: None,
            condition_flags: None,
            type_descriptor: None,
        };

        assert_eq!(evaluator.get_plugin_type(&plugin), PluginType::Optional);
    }

    #[test]
    fn test_evaluate_dependencies_none() {
        let evaluator = ConditionEvaluator::new();
        // None should always evaluate to true
        assert!(evaluator.evaluate_dependencies(&None));
    }

    #[test]
    fn test_file_state_from_str() {
        assert_eq!(FileState::from_str("Active"), FileState::Active);
        assert_eq!(FileState::from_str("active"), FileState::Active);
        assert_eq!(FileState::from_str("Inactive"), FileState::Inactive);
        assert_eq!(FileState::from_str("Missing"), FileState::Missing);
        assert_eq!(FileState::from_str("unknown"), FileState::Missing);
    }

    #[test]
    fn test_plugin_type_from_str() {
        assert_eq!(PluginType::from_str("Required"), PluginType::Required);
        assert_eq!(PluginType::from_str("Recommended"), PluginType::Recommended);
        assert_eq!(PluginType::from_str("Optional"), PluginType::Optional);
        assert_eq!(PluginType::from_str("NotUsable"), PluginType::NotUsable);
        assert_eq!(
            PluginType::from_str("CouldBeUsable"),
            PluginType::CouldBeUsable
        );
        assert_eq!(PluginType::from_str("unknown"), PluginType::Optional);
    }
}
