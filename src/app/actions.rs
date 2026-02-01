//! CLI command action handlers

use super::App;
use crate::games::GameDetector;
use anyhow::{bail, Result};

impl App {
    // ========== Game Commands ==========

    pub async fn cmd_game_list(&self) -> Result<()> {
        if self.games.is_empty() {
            println!("No games detected. Run 'modsanity game scan' to scan for games.");
            return Ok(());
        }

        let active = self.config.read().await.active_game.clone();

        println!("Detected Games:");
        println!("{:-<60}", "");
        for game in &self.games {
            let marker = if Some(&game.id) == active.as_ref() {
                " [active]"
            } else {
                ""
            };
            println!(
                "  {} ({}){}\n    Path: {}",
                game.name,
                game.id,
                marker,
                game.install_path.display()
            );
        }
        Ok(())
    }

    pub async fn cmd_game_scan(&mut self) -> Result<()> {
        println!("Scanning for games...");
        self.games = GameDetector::detect_all().await;

        if self.games.is_empty() {
            println!("No games found.");
        } else {
            println!("Found {} game(s):", self.games.len());
            for game in &self.games {
                println!("  - {} at {}", game.name, game.install_path.display());
            }
        }
        Ok(())
    }

    pub async fn cmd_game_select(&mut self, name: &str) -> Result<()> {
        let game = self
            .games
            .iter()
            .find(|g| g.id == name || g.name.to_lowercase().contains(&name.to_lowercase()))
            .cloned();

        match game {
            Some(g) => {
                println!("Selected: {} ({})", g.name, g.id);
                self.set_active_game(Some(g)).await?;
            }
            None => bail!("Game '{}' not found. Run 'modsanity game list' to see available games.", name),
        }
        Ok(())
    }

    pub async fn cmd_game_info(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Game Information");
        println!("{:-<40}", "");
        println!("Name:         {}", game.name);
        println!("ID:           {}", game.id);
        println!("Install Path: {}", game.install_path.display());
        println!("Data Path:    {}", game.data_path.display());
        if let Some(prefix) = &game.proton_prefix {
            println!("Proton Prefix: {}", prefix.display());
        }
        if let Some(appdata) = &game.appdata_path {
            println!("AppData:      {}", appdata.display());
        }
        Ok(())
    }

    // ========== Mod Commands ==========

    pub async fn cmd_mod_list(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let mods = self.mods.list_mods(&game.id).await?;

        if mods.is_empty() {
            println!("No mods installed for {}.", game.name);
            return Ok(());
        }

        println!("Installed Mods for {}:", game.name);
        println!("{:-<60}", "");
        for (i, m) in mods.iter().enumerate() {
            let status = if m.enabled { "[✓]" } else { "[ ]" };
            println!("{:>3}. {} {} (v{})", i + 1, status, m.name, m.version);
        }
        Ok(())
    }

    pub async fn cmd_mod_install(&self, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Installing mod from: {}", path);
        match self.mods.install_from_archive(&game.id, path, None, None, None).await? {
            crate::mods::InstallResult::Completed(installed) => {
                println!("Installed: {} (v{})", installed.name, installed.version);
                println!("Run 'modsanity deploy' to apply changes.");
                Ok(())
            }
            crate::mods::InstallResult::RequiresWizard(context) => {
                println!("ERROR: {} requires FOMOD wizard interaction", context.mod_name);
                println!("FOMOD wizards are only supported in TUI mode (run without arguments)");
                bail!("Interactive wizard required")
            }
        }
    }

    pub async fn cmd_mod_enable(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.enable_mod(&game.id, name).await?;
        println!("Enabled: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_mod_disable(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.disable_mod(&game.id, name).await?;
        println!("Disabled: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_mod_remove(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.mods.remove_mod(&game.id, name).await?;
        println!("Removed: {}", name);
        Ok(())
    }

    pub async fn cmd_mod_info(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let m = self.mods.get_mod(&game.id, name).await?;

        println!("Mod Information");
        println!("{:-<40}", "");
        println!("Name:     {}", m.name);
        println!("Version:  {}", m.version);
        println!("Enabled:  {}", if m.enabled { "Yes" } else { "No" });
        println!("Priority: {}", m.priority);
        if let Some(author) = &m.author {
            println!("Author:   {}", author);
        }
        if let Some(nexus_id) = m.nexus_mod_id {
            println!("Nexus ID: {}", nexus_id);
        }
        println!("Files:    {}", m.file_count);
        Ok(())
    }

    // ========== Profile Commands ==========

    pub async fn cmd_profile_list(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let profiles = self.profiles.list_profiles(&game.id).await?;
        let active = self.config.read().await.active_profile.clone();

        if profiles.is_empty() {
            println!("No profiles for {}.", game.name);
            return Ok(());
        }

        println!("Profiles for {}:", game.name);
        println!("{:-<40}", "");
        for p in profiles {
            let marker = if Some(&p.name) == active.as_ref() {
                " [active]"
            } else {
                ""
            };
            println!("  {}{}", p.name, marker);
        }
        Ok(())
    }

    pub async fn cmd_profile_create(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.create_profile(&game.id, name).await?;
        println!("Created profile: {}", name);
        Ok(())
    }

    pub async fn cmd_profile_switch(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.switch_profile(&game.id, name).await?;
        println!("Switched to profile: {}", name);
        println!("Run 'modsanity deploy' to apply changes.");
        Ok(())
    }

    pub async fn cmd_profile_delete(&self, name: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.delete_profile(&game.id, name).await?;
        println!("Deleted profile: {}", name);
        Ok(())
    }

    pub async fn cmd_profile_export(&self, name: &str, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        self.profiles.export_profile(&game.id, name, path).await?;
        println!("Exported '{}' to: {}", name, path);
        Ok(())
    }

    pub async fn cmd_profile_import(&self, path: &str) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        let profile = self.profiles.import_profile(&game.id, path).await?;
        println!("Imported profile: {}", profile.name);
        Ok(())
    }

    // ========== Other Commands ==========

    pub async fn cmd_deploy(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected."),
        };

        println!("Deploying mods to {}...", game.name);
        let stats = self.mods.deploy(&game).await?;
        println!(
            "Deployed {} files from {} mods.",
            stats.files_deployed, stats.mods_deployed
        );
        Ok(())
    }

    pub async fn cmd_status(&self) -> Result<()> {
        println!("ModSanity Status");
        println!("{:-<40}", "");

        // Game status
        match self.active_game().await {
            Some(g) => println!("Active Game: {} ({})", g.name, g.id),
            None => println!("Active Game: None"),
        };

        // Profile status
        match &self.config.read().await.active_profile {
            Some(p) => println!("Profile:     {}", p),
            None => println!("Profile:     Default"),
        };

        // Mod counts
        if let Some(game) = self.active_game().await {
            let mods = self.mods.list_mods(&game.id).await?;
            let enabled = mods.iter().filter(|m| m.enabled).count();
            println!("Mods:        {} installed, {} enabled", mods.len(), enabled);
        }

        Ok(())
    }
}
