//! Profile management for mod configurations

mod manager;

pub use manager::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A profile stores a specific mod configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name
    pub name: String,

    /// Profile description
    pub description: Option<String>,

    /// Game ID this profile is for
    pub game_id: String,

    /// Enabled mods and their priorities
    pub mods: HashMap<String, ModState>,

    /// Plugin load order
    pub load_order: Vec<String>,

    /// Enabled plugins
    pub enabled_plugins: Vec<String>,

    /// Creation timestamp
    pub created_at: String,

    /// Last modified timestamp
    pub updated_at: String,
}

/// Mod state within a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModState {
    /// Is the mod enabled
    pub enabled: bool,

    /// Priority (load order)
    pub priority: i32,
}

impl Profile {
    /// Create a new profile
    pub fn new(name: impl Into<String>, game_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: name.into(),
            description: None,
            game_id: game_id.into(),
            mods: HashMap::new(),
            load_order: Vec::new(),
            enabled_plugins: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Add a mod to the profile
    pub fn add_mod(&mut self, mod_name: impl Into<String>, enabled: bool, priority: i32) {
        self.mods.insert(
            mod_name.into(),
            ModState { enabled, priority },
        );
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Remove a mod from the profile
    pub fn remove_mod(&mut self, mod_name: &str) {
        self.mods.remove(mod_name);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Set load order
    pub fn set_load_order(&mut self, order: Vec<String>) {
        self.load_order = order;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Set enabled plugins
    pub fn set_enabled_plugins(&mut self, plugins: Vec<String>) {
        self.enabled_plugins = plugins;
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}
