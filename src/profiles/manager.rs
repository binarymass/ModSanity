//! Profile manager

use super::Profile;
use crate::config::Config;
use crate::db::{Database, ProfileRecord};
use crate::games::GameDetector;
use crate::plugins;
use anyhow::{bail, Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Profile manager handles profile CRUD operations
pub struct ProfileManager {
    config: Arc<RwLock<Config>>,
    db: Arc<Database>,
}

impl ProfileManager {
    /// Create a new ProfileManager
    pub fn new(config: Arc<RwLock<Config>>, db: Arc<Database>) -> Self {
        Self { config, db }
    }

    /// List all profiles for a game
    pub async fn list_profiles(&self, game_id: &str) -> Result<Vec<Profile>> {
        let records = self.db.get_profiles_for_game(game_id)?;

        let mut profiles = Vec::new();
        for record in records {
            // Load full profile from file
            let profile_path = self
                .config
                .read()
                .await
                .paths
                .game_profiles_dir(game_id)
                .join(format!("{}.json", record.name));

            if profile_path.exists() {
                let content = tokio::fs::read_to_string(&profile_path).await?;
                let profile: Profile = serde_json::from_str(&content)?;
                profiles.push(profile);
            } else {
                // Create minimal profile from DB record
                profiles.push(Profile {
                    name: record.name,
                    description: record.description,
                    game_id: record.game_id,
                    mods: Default::default(),
                    load_order: Vec::new(),
                    enabled_plugins: Vec::new(),
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                });
            }
        }

        Ok(profiles)
    }

    /// Create a new profile
    pub async fn create_profile(&self, game_id: &str, name: &str) -> Result<Profile> {
        // Check if exists
        let existing = self.db.get_profiles_for_game(game_id)?;
        if existing.iter().any(|p| p.name == name) {
            bail!("Profile '{}' already exists", name);
        }

        // Create profile
        let profile = Profile::new(name, game_id);

        // Save to database
        let now = chrono::Utc::now().to_rfc3339();
        let record = ProfileRecord {
            id: None,
            game_id: game_id.to_string(),
            name: name.to_string(),
            description: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.db.insert_profile(&record)?;

        // Save to file
        self.save_profile(&profile).await?;

        Ok(profile)
    }

    /// Delete a profile
    pub async fn delete_profile(&self, game_id: &str, name: &str) -> Result<()> {
        let records = self.db.get_profiles_for_game(game_id)?;
        let record = records
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

        // Delete from database
        self.db.delete_profile(record.id.unwrap())?;

        // Delete file
        let profile_path = self
            .config
            .read()
            .await
            .paths
            .game_profiles_dir(game_id)
            .join(format!("{}.json", name));

        if profile_path.exists() {
            tokio::fs::remove_file(profile_path).await?;
        }

        Ok(())
    }

    /// Switch to a profile
    pub async fn switch_profile(&self, game_id: &str, name: &str) -> Result<()> {
        // Load the profile
        let profiles = self.list_profiles(game_id).await?;
        let profile = profiles
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

        // Apply profile settings to mods
        let all_mods = self.db.get_mods_for_game(game_id)?;

        for mod_record in all_mods {
            let mod_id = mod_record.id.unwrap();

            if let Some(mod_state) = profile.mods.get(&mod_record.name) {
                // Mod is in profile - apply its settings
                if mod_record.enabled != mod_state.enabled {
                    self.db.set_mod_enabled(mod_id, mod_state.enabled)?;
                }
                if mod_record.priority != mod_state.priority {
                    self.db.set_mod_priority(mod_id, mod_state.priority)?;
                }
            } else {
                // Mod not in profile - disable it
                if mod_record.enabled {
                    self.db.set_mod_enabled(mod_id, false)?;
                }
            }
        }

        // Apply plugin state/load order files if we can resolve the game installation.
        if !profile.enabled_plugins.is_empty() || !profile.load_order.is_empty() {
            let detected = GameDetector::detect_all().await;
            if let Some(game) = detected.into_iter().find(|g| g.id == game_id) {
                if !profile.enabled_plugins.is_empty() {
                    plugins::write_plugins_txt(&game, &profile.enabled_plugins)
                        .context("Failed to write plugins.txt for profile switch")?;
                }

                if !profile.load_order.is_empty() {
                    plugins::write_loadorder_txt(&game, &profile.load_order)
                        .context("Failed to write loadorder.txt for profile switch")?;
                }
            } else {
                tracing::warn!(
                    "Profile '{}' has plugin state, but game '{}' is not currently detected; skipping plugins/loadorder write",
                    name,
                    game_id
                );
            }
        }

        // Update config
        let mut config = self.config.write().await;
        config.active_profile = Some(name.to_string());
        config.save().await?;

        Ok(())
    }

    /// Export a profile to a file
    pub async fn export_profile(&self, game_id: &str, name: &str, path: &str) -> Result<()> {
        let profiles = self.list_profiles(game_id).await?;
        let profile = profiles
            .iter()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

        let content = serde_json::to_string_pretty(profile)?;
        tokio::fs::write(path, content).await?;

        Ok(())
    }

    /// Import a profile from a file
    pub async fn import_profile(&self, game_id: &str, path: &str) -> Result<Profile> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read profile file")?;

        let mut profile: Profile =
            serde_json::from_str(&content).context("Failed to parse profile")?;

        // Update game ID to match target
        profile.game_id = game_id.to_string();

        // Check for name conflict
        let existing = self.db.get_profiles_for_game(game_id)?;
        if existing.iter().any(|p| p.name == profile.name) {
            // Append suffix
            let mut i = 1;
            let base_name = profile.name.clone();
            while existing.iter().any(|p| p.name == profile.name) {
                profile.name = format!("{} ({})", base_name, i);
                i += 1;
            }
        }

        // Save to database
        let now = chrono::Utc::now().to_rfc3339();
        let record = ProfileRecord {
            id: None,
            game_id: game_id.to_string(),
            name: profile.name.clone(),
            description: profile.description.clone(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.db.insert_profile(&record)?;

        // Save to file
        self.save_profile(&profile).await?;

        Ok(profile)
    }

    /// Save a profile to disk
    async fn save_profile(&self, profile: &Profile) -> Result<()> {
        let profiles_dir = self
            .config
            .read()
            .await
            .paths
            .game_profiles_dir(&profile.game_id);

        tokio::fs::create_dir_all(&profiles_dir).await?;

        let profile_path = profiles_dir.join(format!("{}.json", profile.name));
        let content = serde_json::to_string_pretty(profile)?;
        tokio::fs::write(profile_path, content).await?;

        Ok(())
    }

    /// Capture current mod state into a profile
    pub async fn capture_current_state(
        &self,
        game_id: &str,
        profile_name: &str,
        mods: &[crate::mods::InstalledMod],
    ) -> Result<Profile> {
        let mut profile = Profile::new(profile_name, game_id);

        for m in mods {
            profile.add_mod(&m.name, m.enabled, m.priority);
        }

        self.save_profile(&profile).await?;

        Ok(profile)
    }
}
