//! FOMOD choice persistence and re-run support
//!
//! Stores FOMOD installation choices for re-running with the same selections.

use super::planner::InstallPlan;
use crate::db::Database;
use anyhow::{Context, Result};

/// Manager for FOMOD choice persistence
pub struct FomodChoiceManager<'a> {
    db: &'a Database,
}

impl<'a> FomodChoiceManager<'a> {
    /// Create a new choice manager
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Save a FOMOD choice to the database
    pub fn save_choice(
        &self,
        mod_id: i64,
        profile_id: Option<i64>,
        plan: &InstallPlan,
    ) -> Result<()> {
        let plan_json = serde_json::to_string(plan).context("Failed to serialize plan")?;

        self.db
            .save_fomod_choice(mod_id, profile_id, &plan.config_hash, &plan_json)?;

        Ok(())
    }

    /// Load a previously saved FOMOD choice
    pub fn load_choice(&self, mod_id: i64, profile_id: Option<i64>) -> Result<Option<InstallPlan>> {
        let choice = self.db.get_fomod_choice(mod_id, profile_id)?;

        match choice {
            Some((config_hash, plan_json)) => {
                let plan: InstallPlan = serde_json::from_str(&plan_json)
                    .context("Failed to deserialize install plan")?;

                // Verify the plan has the same hash
                if plan.config_hash == config_hash {
                    Ok(Some(plan))
                } else {
                    // Hash mismatch - plan is stale
                    tracing::warn!(
                        "Config hash mismatch for mod_id={}: expected {}, got {}",
                        mod_id,
                        config_hash,
                        plan.config_hash
                    );
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Check if a saved choice is still valid
    pub fn is_choice_valid(&self, plan: &InstallPlan, current_hash: &str) -> bool {
        Self::is_choice_valid_static(plan, current_hash)
    }

    /// Check if a saved choice is still valid (static version)
    pub fn is_choice_valid_static(plan: &InstallPlan, current_hash: &str) -> bool {
        plan.config_hash == current_hash
    }

    /// Delete a saved choice
    pub fn delete_choice(&self, mod_id: i64, profile_id: Option<i64>) -> Result<()> {
        self.db.delete_fomod_choice(mod_id, profile_id)?;
        Ok(())
    }

    /// Get all saved choices for a profile
    pub fn get_profile_choices(&self, profile_id: i64) -> Result<Vec<(i64, InstallPlan)>> {
        let choices = self.db.get_profile_fomod_choices(profile_id)?;
        let mut result = Vec::new();

        for (mod_id, _config_hash, plan_json) in choices {
            if let Ok(plan) = serde_json::from_str(&plan_json) {
                result.push((mod_id, plan));
            }
        }

        Ok(result)
    }

    /// Get all saved choices for a mod across all profiles
    pub fn get_mod_choices(&self, mod_id: i64) -> Result<Vec<(Option<i64>, InstallPlan)>> {
        let choices = self.db.get_mod_fomod_choices(mod_id)?;
        let mut result = Vec::new();

        for (profile_id, _config_hash, plan_json) in choices {
            if let Ok(plan) = serde_json::from_str(&plan_json) {
                result.push((profile_id, plan));
            }
        }

        Ok(result)
    }
}

/// Compute hash of ModuleConfig for invalidation detection
pub fn hash_module_config(config: &super::ModuleConfig) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    // Hash module name
    config.module_name.hash(&mut hasher);

    // Hash number of steps
    config.install_steps.steps.len().hash(&mut hasher);

    // Hash each step's structure
    for step in &config.install_steps.steps {
        step.name.hash(&mut hasher);
        step.groups.groups.len().hash(&mut hasher);

        for group in &step.groups.groups {
            group.name.hash(&mut hasher);
            group.group_type.hash(&mut hasher);
            group.plugins.plugins.len().hash(&mut hasher);

            for plugin in &group.plugins.plugins {
                plugin.name.hash(&mut hasher);
            }
        }
    }

    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_hash_stability() {
        // Create a simple config
        use crate::mods::fomod::{
            InstallStep, InstallSteps, ModuleConfig, OptionGroup, OptionGroups, Plugin, PluginList,
        };

        let config = ModuleConfig {
            module_name: "Test Mod".to_string(),
            module_image: None,
            dependencies: None,
            required_files: None,
            install_steps: InstallSteps {
                order: String::new(),
                steps: vec![InstallStep {
                    name: "Step 1".to_string(),
                    groups: OptionGroups {
                        order: String::new(),
                        groups: vec![OptionGroup {
                            name: "Group 1".to_string(),
                            group_type: "SelectExactlyOne".to_string(),
                            plugins: PluginList {
                                order: String::new(),
                                plugins: vec![
                                    Plugin {
                                        name: "Option A".to_string(),
                                        description: String::new(),
                                        image: None,
                                        files: None,
                                        condition_flags: None,
                                        type_descriptor: None,
                                    },
                                    Plugin {
                                        name: "Option B".to_string(),
                                        description: String::new(),
                                        image: None,
                                        files: None,
                                        condition_flags: None,
                                        type_descriptor: None,
                                    },
                                ],
                            },
                        }],
                    },
                }],
            },
            conditional_installs: None,
        };

        // Hash should be consistent
        let hash1 = hash_module_config(&config);
        let hash2 = hash_module_config(&config);
        assert_eq!(hash1, hash2);

        // Different config should have different hash
        let mut config2 = config.clone();
        config2.module_name = "Different Mod".to_string();
        let hash3 = hash_module_config(&config2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_choice_validation() {
        let plan = InstallPlan {
            mod_name: "Test".to_string(),
            profile_id: None,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            config_hash: "abc123".to_string(),
            selected_options: vec![],
            flags_set: HashMap::new(),
            file_operations: vec![],
            estimated_file_count: 0,
            estimated_size_bytes: 0,
            conflicts: vec![],
        };

        // Test the static method directly without needing a database instance
        assert!(FomodChoiceManager::is_choice_valid_static(&plan, "abc123"));
        assert!(!FomodChoiceManager::is_choice_valid_static(&plan, "def456"));
    }
}
