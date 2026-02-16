//! CLI command action handlers

use super::App;
use crate::config::{DeploymentMethod, ExternalTool, ToolRuntimeMode};
use crate::games::{GameDetector, GamePlatform};
use anyhow::{bail, Context, Result};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

struct CliStatusReporter {
    interactive: bool,
    last_line_len: usize,
    last_emit: Instant,
    min_emit_interval: Duration,
}

impl CliStatusReporter {
    fn new(min_emit_interval: Duration) -> Self {
        Self {
            interactive: io::stdout().is_terminal(),
            last_line_len: 0,
            last_emit: Instant::now() - min_emit_interval,
            min_emit_interval,
        }
    }

    fn emit_catalog_progress(
        &mut self,
        pages: i32,
        inserted: i64,
        updated: i64,
        total_count: i64,
    ) -> io::Result<()> {
        let now = Instant::now();
        if now.duration_since(self.last_emit) < self.min_emit_interval {
            return Ok(());
        }
        self.last_emit = now;

        let processed = inserted + updated;
        let percent = if total_count > 0 {
            ((processed as f64 / total_count as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        let line = if total_count > 0 {
            format!(
                "Progress: pages={} processed={}/{} ({:.1}%) inserted={} updated={}",
                pages, processed, total_count, percent, inserted, updated
            )
        } else {
            format!(
                "Progress: pages={} processed={} inserted={} updated={}",
                pages, processed, inserted, updated
            )
        };

        if self.interactive {
            print!(
                "\r{line:<width$}",
                width = self.last_line_len.max(line.len())
            );
            io::stdout().flush()?;
            self.last_line_len = self.last_line_len.max(line.len());
        } else {
            println!("{line}");
        }

        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if self.interactive {
            println!();
            io::stdout().flush()?;
        }
        Ok(())
    }
}

impl App {
    fn modlist_name_from_path(path: &str, fallback: &str) -> String {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn persist_modlist_to_db(
        &self,
        game_id: &str,
        name: &str,
        source_file: Option<&str>,
        entries: &[crate::db::ModlistEntryRecord],
    ) -> Result<i64> {
        self.db
            .upsert_modlist_with_entries(game_id, name, None, source_file, entries)
    }

    // ========== Game Commands ==========

    pub async fn cmd_game_list(&self) -> Result<()> {
        if self.games.is_empty() {
            println!("No games detected. Run 'modsanity game scan' to scan for games.");
            return Ok(());
        }

        let active = self.config.read().await.active_game.clone();
        let mut active_marked = false;

        println!("Detected Games:");
        println!("{:-<60}", "");
        for game in &self.games {
            let marker = if Some(&game.id) == active.as_ref() && !active_marked {
                active_marked = true;
                " [active]"
            } else {
                ""
            };
            println!(
                "  {} ({}, {}){}\n    Path: {}",
                game.name,
                game.id,
                game.platform.display_name(),
                marker,
                game.install_path.display()
            );
        }
        Ok(())
    }

    pub async fn cmd_game_scan(&mut self) -> Result<()> {
        println!("Scanning for games...");
        let custom = self.config.read().await.custom_games.clone();
        self.games = GameDetector::detect_all_with_custom(&custom).await;

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

    pub async fn cmd_game_add_path(
        &mut self,
        game_id: &str,
        path: &str,
        platform: &str,
        proton_prefix: Option<&str>,
    ) -> Result<()> {
        let platform = match platform.to_ascii_lowercase().as_str() {
            "steam" => GamePlatform::Steam,
            "gog" => GamePlatform::Gog,
            "manual" => GamePlatform::Manual,
            other => bail!("Unknown platform '{}'. Use: steam, gog, manual", other),
        };
        self.add_custom_game_path(game_id, path, platform, proton_prefix)
            .await?;
        println!(
            "Saved custom game path: {} ({}) -> {}",
            game_id,
            platform.display_name(),
            path
        );
        println!("Run 'modsanity game scan' or 'modsanity game list' to verify detection.");
        Ok(())
    }

    pub async fn cmd_game_remove_path(&mut self, game_id: &str, path: &str) -> Result<()> {
        self.remove_custom_game_path(game_id, path).await?;
        println!("Removed custom game path: {} -> {}", game_id, path);
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
            None => bail!(
                "Game '{}' not found. Run 'modsanity game list' to see available games.",
                name
            ),
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
        println!("Platform:     {}", game.platform.display_name());
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
        match self
            .mods
            .install_from_archive(&game.id, path, None, None, None, None)
            .await?
        {
            crate::mods::InstallResult::Completed(installed) => {
                println!("Installed: {} (v{})", installed.name, installed.version);
                println!("Run 'modsanity deploy' to apply changes.");
                Ok(())
            }
            crate::mods::InstallResult::RequiresWizard(context) => {
                println!(
                    "ERROR: {} requires FOMOD wizard interaction",
                    context.mod_name
                );
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

    pub async fn cmd_mod_rescan(&self) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Scanning staging directory for {}...", game.name);
        let stats = self.mods.rescan_mods(&game.id, None).await?;
        println!(
            "Rescan complete: {} added, {} updated, {} unchanged, {} failed",
            stats.added, stats.updated, stats.unchanged, stats.failed
        );
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

    pub async fn cmd_set_deployment_method(&self, method: &str) -> Result<()> {
        let parsed = DeploymentMethod::from_cli(method)?;
        self.set_deployment_method(parsed).await?;
        println!(
            "Deployment method set to: {} ({})",
            parsed.display_name(),
            parsed.as_str()
        );
        Ok(())
    }

    pub async fn cmd_deployment_show(&self) -> Result<()> {
        let config = self.config.read().await;
        println!("Deployment Settings");
        println!("{:-<40}", "");
        println!(
            "Method:           {}",
            config.deployment.method.display_name()
        );
        println!("Method (raw):     {}", config.deployment.method.as_str());
        println!(
            "Backup originals: {}",
            if config.deployment.backup_originals {
                "Yes"
            } else {
                "No"
            }
        );
        println!(
            "Purge on exit:    {}",
            if config.deployment.purge_on_exit {
                "Yes"
            } else {
                "No"
            }
        );
        println!("Downloads dir:    {}", config.downloads_dir().display());
        println!("Staging dir:      {}", config.staging_dir().display());
        Ok(())
    }

    pub async fn cmd_set_downloads_dir(&self, path: &str) -> Result<()> {
        Self::validate_directory_override(path)?;
        let override_path = if path.trim().is_empty() {
            None
        } else {
            Some(path)
        };
        self.set_downloads_dir_override(override_path).await?;
        let resolved = self.resolved_downloads_dir().await;
        if override_path.is_some() {
            println!(
                "Downloads directory override set to: {}",
                resolved.display()
            );
        } else {
            println!(
                "Downloads directory override cleared (using default): {}",
                resolved.display()
            );
        }
        Ok(())
    }

    pub async fn cmd_set_staging_dir(&self, path: &str) -> Result<()> {
        Self::validate_directory_override(path)?;
        let override_path = if path.trim().is_empty() {
            None
        } else {
            Some(path)
        };
        self.set_staging_dir_override(override_path).await?;
        let resolved = self.resolved_staging_dir().await;
        if override_path.is_some() {
            println!("Staging directory override set to: {}", resolved.display());
        } else {
            println!(
                "Staging directory override cleared (using default): {}",
                resolved.display()
            );
        }
        Ok(())
    }

    pub async fn cmd_migrate_staging(&self, from: &str, to: &str, dry_run: bool) -> Result<()> {
        use walkdir::WalkDir;

        let src = std::path::Path::new(from);
        let dst = std::path::Path::new(to);
        if !src.exists() || !src.is_dir() {
            bail!(
                "Source staging directory does not exist or is not a directory: {}",
                src.display()
            );
        }
        if src == dst {
            bail!("Source and destination staging directories are the same path");
        }

        let mut files_total = 0usize;
        let mut files_to_copy = 0usize;
        let mut files_skipped_existing = 0usize;
        let mut dirs_to_create = 0usize;

        for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p == src {
                continue;
            }
            let rel = p.strip_prefix(src).unwrap_or(p);
            let target = dst.join(rel);
            if entry.file_type().is_dir() {
                if !target.exists() {
                    dirs_to_create += 1;
                }
                continue;
            }
            if entry.file_type().is_file() {
                files_total += 1;
                if target.exists() {
                    files_skipped_existing += 1;
                } else {
                    files_to_copy += 1;
                }
            }
        }

        println!("Staging Migration Plan");
        println!("{:-<60}", "");
        println!("From: {}", src.display());
        println!("To:   {}", dst.display());
        println!("Directories to create: {}", dirs_to_create);
        println!("Files total scanned:   {}", files_total);
        println!("Files to copy:         {}", files_to_copy);
        println!("Files skipped(existing): {}", files_skipped_existing);
        println!(
            "Mode: {}",
            if dry_run {
                "dry-run (no writes)"
            } else {
                "apply"
            }
        );

        if dry_run {
            println!("Dry-run complete. Re-run without --dry-run to execute.");
            return Ok(());
        }

        std::fs::create_dir_all(dst)
            .with_context(|| format!("Failed to create destination root: {}", dst.display()))?;

        let mut copied = 0usize;
        for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if p == src {
                continue;
            }
            let rel = p.strip_prefix(src).unwrap_or(p);
            let target = dst.join(rel);
            if entry.file_type().is_dir() {
                if !target.exists() {
                    std::fs::create_dir_all(&target).with_context(|| {
                        format!("Failed creating directory: {}", target.display())
                    })?;
                }
                continue;
            }
            if entry.file_type().is_file() {
                if target.exists() {
                    continue;
                }
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("Failed creating parent: {}", parent.display()))?;
                }
                std::fs::copy(p, &target).with_context(|| {
                    format!(
                        "Failed copying file: {} -> {}",
                        p.display(),
                        target.display()
                    )
                })?;
                copied += 1;
            }
        }

        println!("Copied {} files.", copied);
        println!("Updating staging override to destination...");
        self.cmd_set_staging_dir(&dst.display().to_string()).await?;
        println!("Migration complete.");
        println!("Recommended: run 'modsanity mod rescan' to refresh DB indexes.");
        Ok(())
    }

    pub async fn cmd_tool_show(&self) -> Result<()> {
        let config = self.config.read().await;
        let detected_runtimes = self.detect_proton_runtimes();
        println!("External Tools");
        println!("{:-<60}", "");
        let runtime_display = config
            .external_tools
            .proton_runtime
            .clone()
            .unwrap_or_else(|| "custom-command".to_string());
        println!("Proton runtime selection: {}", runtime_display);
        println!(
            "Proton command fallback:  {}",
            config.external_tools.proton_command
        );
        match self.resolve_proton_launcher_from_config(&config) {
            Ok(resolved) => println!("Resolved Proton launcher: {}", resolved),
            Err(e) => println!("Resolved Proton launcher: <unresolved> ({})", e),
        }
        println!("Detected Proton runtimes: {}", detected_runtimes.len());
        for runtime in &detected_runtimes {
            println!(
                "  - {:<28} {:<24} {}",
                runtime.id,
                runtime.name,
                runtime.proton_path.display()
            );
        }
        for tool in ExternalTool::all() {
            let value = config.external_tool_path(*tool).unwrap_or("Not set");
            let mode = config.external_tool_runtime_mode(*tool).as_str();
            println!("{:>14}: {} (runtime: {})", tool.display_name(), value, mode);
        }
        Ok(())
    }

    pub async fn cmd_tool_set_proton(&self, path: &str) -> Result<()> {
        self.set_proton_command(path).await?;
        println!("Proton command set to: {}", path.trim());
        println!("Proton runtime selection cleared (using custom command/path mode).");
        Ok(())
    }

    pub async fn cmd_tool_list_proton(&self) -> Result<()> {
        let runtimes = self.detect_proton_runtimes();
        let config = self.config.read().await;
        let selected = config.external_tools.proton_runtime.as_deref();
        println!("Detected Proton Runtimes");
        println!("{:-<80}", "");
        if runtimes.is_empty() {
            println!("No Steam-managed Proton runtimes were detected.");
            println!("Install/select a Proton tool in Steam, then rerun this command.");
            return Ok(());
        }
        for runtime in runtimes {
            let marker = if selected == Some(runtime.id.as_str()) {
                " [selected]"
            } else {
                ""
            };
            println!(
                "{}{}",
                format!(
                    "{:<30} {:<22} {} ({})",
                    runtime.id,
                    runtime.name,
                    runtime.proton_path.display(),
                    runtime.source
                ),
                marker
            );
        }
        println!("\nUse one with: modsanity tool use-proton <runtime_id>");
        println!("Or auto-select: modsanity tool use-proton auto");
        Ok(())
    }

    pub async fn cmd_tool_use_proton(&self, runtime: &str) -> Result<()> {
        self.set_proton_runtime(Some(runtime)).await?;
        println!("Proton runtime selected: {}", runtime.trim());
        Ok(())
    }

    pub async fn cmd_tool_clear_proton_runtime(&self) -> Result<()> {
        self.set_proton_runtime(None).await?;
        println!("Proton runtime selection cleared (using custom command/path).");
        Ok(())
    }

    pub async fn cmd_tool_set_path(&self, tool: &str, path: &str) -> Result<()> {
        let parsed = ExternalTool::from_cli(tool)?;
        self.set_external_tool_path(parsed, Some(path)).await?;
        println!("{} path set to: {}", parsed.display_name(), path);
        Ok(())
    }

    pub async fn cmd_tool_clear_path(&self, tool: &str) -> Result<()> {
        let parsed = ExternalTool::from_cli(tool)?;
        self.set_external_tool_path(parsed, None).await?;
        println!("{} path cleared", parsed.display_name());
        Ok(())
    }

    pub async fn cmd_tool_set_runtime(&self, tool: &str, mode: &str) -> Result<()> {
        let parsed_tool = ExternalTool::from_cli(tool)?;
        let parsed_mode = ToolRuntimeMode::from_cli(mode)?;
        self.set_external_tool_runtime_mode(parsed_tool, Some(parsed_mode))
            .await?;
        println!(
            "{} runtime mode set to: {}",
            parsed_tool.display_name(),
            parsed_mode.as_str()
        );
        Ok(())
    }

    pub async fn cmd_tool_clear_runtime(&self, tool: &str) -> Result<()> {
        let parsed_tool = ExternalTool::from_cli(tool)?;
        self.set_external_tool_runtime_mode(parsed_tool, None)
            .await?;
        println!(
            "{} runtime mode reset to default: proton",
            parsed_tool.display_name()
        );
        Ok(())
    }

    pub async fn cmd_tool_run(&self, tool: &str, args: &[String]) -> Result<()> {
        let parsed = ExternalTool::from_cli(tool)?;
        println!("Launching {} via Proton...", parsed.display_name());
        let code = self.launch_external_tool(parsed, args).await?;
        println!("{} exited with code {}", parsed.display_name(), code);
        Ok(())
    }

    pub async fn cmd_status(&self) -> Result<()> {
        println!("ModSanity Status");
        println!("{:-<40}", "");

        let config = self.config.read().await;

        // Game status
        match self.active_game().await {
            Some(g) => println!("Active Game: {} ({})", g.name, g.id),
            None => println!("Active Game: None"),
        };

        // Profile status
        match &config.active_profile {
            Some(p) => println!("Profile:     {}", p),
            None => println!("Profile:     Default"),
        };
        println!("Deploy:      {}", config.deployment.method.display_name());

        // Mod counts
        if let Some(game) = self.active_game().await {
            let mods = self.mods.list_mods(&game.id).await?;
            let enabled = mods.iter().filter(|m| m.enabled).count();
            println!("Mods:        {} installed, {} enabled", mods.len(), enabled);
        }

        Ok(())
    }

    pub async fn cmd_doctor(&self, verbose: bool) -> Result<()> {
        fn dir_is_writable(path: &std::path::Path) -> bool {
            if !path.exists() || !path.is_dir() {
                return false;
            }
            let probe = path.join(format!(".modsanity_doctor_{}", std::process::id()));
            match std::fs::File::create(&probe) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe);
                    true
                }
                Err(_) => false,
            }
        }

        fn print_check(name: &str, passed: bool, detail: String, ok: &mut usize, fail: &mut usize) {
            if passed {
                *ok += 1;
                println!("[ OK ] {:<24} {}", name, detail);
            } else {
                *fail += 1;
                println!("[FAIL] {:<24} {}", name, detail);
            }
        }

        fn print_check_warn(
            name: &str,
            passed: bool,
            detail: String,
            ok: &mut usize,
            warn: &mut usize,
        ) {
            if passed {
                *ok += 1;
                println!("[ OK ] {:<24} {}", name, detail);
            } else {
                *warn += 1;
                println!("[WARN] {:<24} {}", name, detail);
            }
        }

        println!("ModSanity Doctor");
        println!("{:-<60}", "");

        let config = self.config.read().await;
        let mut ok = 0usize;
        let mut warn = 0usize;
        let mut fail = 0usize;
        let mut hints: Vec<String> = Vec::new();

        let config_path = config.paths.config_file();
        print_check(
            "Config file",
            config_path.exists(),
            config_path.display().to_string(),
            &mut ok,
            &mut fail,
        );
        print_check_warn(
            "Init marker",
            config.first_run_completed,
            config
                .first_run_completed_at
                .clone()
                .unwrap_or_else(|| "not completed".to_string()),
            &mut ok,
            &mut warn,
        );
        if !config.first_run_completed {
            hints.push("Run guided setup: modsanity init --interactive".to_string());
        }

        let db_path = config.paths.database_file();
        print_check(
            "Database path",
            db_path.parent().map(|p| p.exists()).unwrap_or(false),
            db_path.display().to_string(),
            &mut ok,
            &mut fail,
        );

        let downloads = config.downloads_dir();
        let staging = config.staging_dir();
        print_check(
            "Downloads dir",
            downloads.exists(),
            downloads.display().to_string(),
            &mut ok,
            &mut fail,
        );
        print_check(
            "Staging dir",
            staging.exists(),
            staging.display().to_string(),
            &mut ok,
            &mut fail,
        );
        let downloads_write = dir_is_writable(&downloads);
        print_check_warn(
            "Downloads writable",
            downloads_write,
            downloads.display().to_string(),
            &mut ok,
            &mut warn,
        );
        if !downloads_write {
            hints.push(format!(
                "Set a writable downloads path: modsanity deployment set-downloads-dir <path> (current: {})",
                downloads.display()
            ));
        }
        let staging_write = dir_is_writable(&staging);
        print_check_warn(
            "Staging writable",
            staging_write,
            staging.display().to_string(),
            &mut ok,
            &mut warn,
        );
        if !staging_write {
            hints.push(format!(
                "Set a writable staging path: modsanity deployment set-staging-dir <path> (current: {})",
                staging.display()
            ));
        }

        let steam_found = self
            .games
            .iter()
            .filter(|g| matches!(g.platform, crate::games::GamePlatform::Steam))
            .count();
        let gog_found = self
            .games
            .iter()
            .filter(|g| matches!(g.platform, crate::games::GamePlatform::Gog))
            .count();
        let manual_found = self
            .games
            .iter()
            .filter(|g| matches!(g.platform, crate::games::GamePlatform::Manual))
            .count();
        print_check_warn(
            "Game detection",
            !self.games.is_empty(),
            format!(
                "{} total (Steam {}, GOG {}, Manual {})",
                self.games.len(),
                steam_found,
                gog_found,
                manual_found
            ),
            &mut ok,
            &mut warn,
        );
        if self.games.is_empty() {
            hints.push(
                "No games detected. Use: modsanity init --game-id <game_id> --platform gog --game-path <path> (or steam/manual)"
                    .to_string(),
            );
        }

        if let Some(game) = self.active_game().await {
            let game_exe = game.install_path.join(&game.executable);
            print_check(
                "Active game path",
                game.install_path.exists(),
                game.install_path.display().to_string(),
                &mut ok,
                &mut fail,
            );
            print_check(
                "Game executable",
                game_exe.exists(),
                game_exe.display().to_string(),
                &mut ok,
                &mut fail,
            );
            if !game_exe.exists() {
                hints.push(format!(
                    "Executable missing at {}. Verify game path/platform mapping.",
                    game_exe.display()
                ));
            }
            if !dir_is_writable(&game.data_path) {
                print_check_warn(
                    "Data writable",
                    false,
                    game.data_path.display().to_string(),
                    &mut ok,
                    &mut warn,
                );
                hints.push(format!(
                    "Game Data is not writable. Fix permissions or deploy to a writable install path: {}",
                    game.data_path.display()
                ));
            } else {
                print_check_warn(
                    "Data writable",
                    true,
                    game.data_path.display().to_string(),
                    &mut ok,
                    &mut warn,
                );
            }
            print_check(
                "Active Data path",
                game.data_path.exists(),
                game.data_path.display().to_string(),
                &mut ok,
                &mut fail,
            );
            let has_prefix = game
                .proton_prefix
                .as_ref()
                .map(|p| p.exists())
                .unwrap_or(false);
            print_check_warn(
                "Proton prefix",
                has_prefix,
                game.proton_prefix
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not detected".to_string()),
                &mut ok,
                &mut warn,
            );
            if !has_prefix {
                hints.push(
                    "No Proton prefix detected. For custom installs, register with --proton-prefix or use native tools."
                        .to_string(),
                );
            } else if let Some(prefix) = &game.proton_prefix {
                let wineprefix = prefix.join("pfx");
                print_check_warn(
                    "WINEPREFIX exists",
                    wineprefix.exists(),
                    wineprefix.display().to_string(),
                    &mut ok,
                    &mut warn,
                );
            }

            let plugins_ready = game
                .plugins_txt_path
                .as_ref()
                .map(|p| p.parent().map(dir_is_writable).unwrap_or(false))
                .unwrap_or(false);
            print_check_warn(
                "plugins.txt target",
                plugins_ready,
                game.plugins_txt_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not configured".to_string()),
                &mut ok,
                &mut warn,
            );
            if !plugins_ready {
                hints.push("plugins/load order location is not writable or not configured; check Proton prefix/appdata path.".to_string());
            }
            let loadorder_ready = game
                .loadorder_txt_path
                .as_ref()
                .map(|p| p.parent().map(dir_is_writable).unwrap_or(false))
                .unwrap_or(false);
            print_check_warn(
                "loadorder.txt target",
                loadorder_ready,
                game.loadorder_txt_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "not configured".to_string()),
                &mut ok,
                &mut warn,
            );
            if !loadorder_ready {
                hints.push("loadorder.txt location is not writable or not configured; check Proton prefix/appdata path.".to_string());
            }
        } else {
            print_check_warn(
                "Active game",
                false,
                "none selected".to_string(),
                &mut ok,
                &mut warn,
            );
            hints.push(
                "Select a game first: modsanity game list && modsanity game select <id>"
                    .to_string(),
            );
        }

        let detected_runtimes = self.detect_proton_runtimes();
        let runtime_mode = config
            .external_tools
            .proton_runtime
            .clone()
            .unwrap_or_else(|| "custom-command".to_string());
        print_check_warn(
            "Proton runtime mode",
            true,
            runtime_mode.clone(),
            &mut ok,
            &mut warn,
        );
        if let Some(selected) = config.external_tools.proton_runtime.as_deref() {
            let runtime_ok = if selected.eq_ignore_ascii_case("auto") {
                !detected_runtimes.is_empty()
            } else {
                detected_runtimes.iter().any(|r| {
                    r.id.eq_ignore_ascii_case(selected) || r.name.eq_ignore_ascii_case(selected)
                })
            };
            let runtime_detail = if selected.eq_ignore_ascii_case("auto") {
                format!("auto ({} detected)", detected_runtimes.len())
            } else {
                selected.to_string()
            };
            print_check_warn(
                "Proton runtime",
                runtime_ok,
                runtime_detail,
                &mut ok,
                &mut warn,
            );
            if !runtime_ok {
                hints.push(
                    "Configured Proton runtime not found. Run: modsanity tool list-proton"
                        .to_string(),
                );
            }
        } else {
            let proton_cmd = &config.external_tools.proton_command;
            let proton_ok =
                std::path::Path::new(proton_cmd).exists() || which::which(proton_cmd).is_ok();
            print_check_warn(
                "Proton command",
                proton_ok,
                proton_cmd.clone(),
                &mut ok,
                &mut warn,
            );
            if !proton_ok {
                hints.push(
                    "Set Proton launcher path: modsanity tool set-proton /path/to/proton"
                        .to_string(),
                );
                hints.push(
                    "Or select a Steam-managed runtime: modsanity tool list-proton && modsanity tool use-proton <id>".to_string(),
                );
            }
        }

        for tool in ExternalTool::all() {
            let configured = config.external_tool_path(*tool).map(|s| s.to_string());
            let present = configured
                .as_deref()
                .map(|p| std::path::Path::new(p).exists())
                .unwrap_or(false);
            let looks_windows_exe = configured
                .as_deref()
                .map(|p| p.to_ascii_lowercase().ends_with(".exe"))
                .unwrap_or(false);
            print_check_warn(
                &format!("Tool {}", tool.as_id()),
                present,
                configured.unwrap_or_else(|| "not configured".to_string()),
                &mut ok,
                &mut warn,
            );
            if present && !looks_windows_exe {
                print_check_warn(
                    &format!("Tool {} extension", tool.as_id()),
                    false,
                    "path does not end with .exe".to_string(),
                    &mut ok,
                    &mut warn,
                );
            }
            if !present {
                hints.push(format!(
                    "Configure {}: modsanity tool set-path {} /path/to/tool.exe",
                    tool.display_name(),
                    tool.as_id()
                ));
            } else if self
                .active_game()
                .await
                .as_ref()
                .and_then(|g| g.proton_prefix.as_ref())
                .is_none()
            {
                hints.push(format!(
                    "{} is configured, but no Proton prefix is active. Use game add-path ... --proton-prefix ...",
                    tool.display_name()
                ));
            }
        }

        print_check_warn(
            "Dependency unrar",
            which::which("unrar").is_ok(),
            "needed for .rar extraction".to_string(),
            &mut ok,
            &mut warn,
        );
        print_check_warn(
            "Dependency LOOT",
            crate::plugins::loot::is_loot_available(),
            "optional plugin sorting integration".to_string(),
            &mut ok,
            &mut warn,
        );
        print_check_warn(
            "Dependency dotnet",
            which::which("dotnet").is_ok(),
            "optional for external patchers".to_string(),
            &mut ok,
            &mut warn,
        );
        print_check_warn(
            "Dependency protontricks",
            which::which("protontricks").is_ok(),
            "optional helper for Proton prefix runtime fixes".to_string(),
            &mut ok,
            &mut warn,
        );
        print_check_warn(
            "Dependency winetricks",
            which::which("winetricks").is_ok(),
            "optional helper for cert/runtime remediation".to_string(),
            &mut ok,
            &mut warn,
        );
        print_check_warn(
            "Dependency openssl",
            which::which("openssl").is_ok(),
            "optional for certificate diagnostics".to_string(),
            &mut ok,
            &mut warn,
        );
        if which::which("dotnet").is_err() {
            hints.push(
                "Install dotnet for native patcher workflows (Synthesis-class tools).".to_string(),
            );
        }
        if let Some(game) = self.active_game().await {
            if let Some(prefix) = &game.proton_prefix {
                let cert_store = prefix.join("pfx/drive_c/windows/system32/catroot2");
                let cert_store_ok = cert_store.exists();
                print_check_warn(
                    "Proton cert store",
                    cert_store_ok,
                    cert_store.display().to_string(),
                    &mut ok,
                    &mut warn,
                );
                if !cert_store_ok {
                    hints.push(format!(
                        "Proton cert store missing. Try: protontricks {} -q crypt32",
                        game.steam_app_id
                    ));
                }
                if which::which("protontricks").is_ok() {
                    hints.push(format!(
                        "For Synthesis/dotnet restore issues: protontricks {} -q dotnet48 corefonts",
                        game.steam_app_id
                    ));
                }
            }
        }
        if config.nexus_api_key.is_none() {
            print_check_warn(
                "Nexus API key",
                false,
                "not configured".to_string(),
                &mut ok,
                &mut warn,
            );
            hints.push("Set Nexus API key in Settings (F4) or config.toml to enable browse/import/download features.".to_string());
        } else {
            print_check_warn(
                "Nexus API key",
                true,
                "configured".to_string(),
                &mut ok,
                &mut warn,
            );
        }

        if verbose {
            println!("{:-<60}", "");
            println!("Custom game entries:");
            if config.custom_games.is_empty() {
                println!("  (none)");
            } else {
                for entry in &config.custom_games {
                    let install_exists = std::path::Path::new(&entry.install_path).exists();
                    let entry_ok = if install_exists { "ok" } else { "missing-path" };
                    println!(
                        "  - {} [{}] {}{} ({})",
                        entry.game_id,
                        entry.platform,
                        entry.install_path,
                        entry
                            .proton_prefix
                            .as_ref()
                            .map(|p| format!(" (prefix: {})", p))
                            .unwrap_or_default(),
                        entry_ok
                    );
                    if !install_exists {
                        hints.push(format!(
                            "Custom entry path missing: {} [{}]",
                            entry.install_path, entry.game_id
                        ));
                    }
                }
            }
        }

        println!("{:-<60}", "");
        println!("Doctor summary: {} OK, {} WARN, {} FAIL", ok, warn, fail);
        if fail > 0 {
            println!("Fix FAIL items first, then rerun: modsanity doctor --verbose");
        }
        if !hints.is_empty() {
            println!("{:-<60}", "");
            println!("Suggested next fixes:");
            for (i, hint) in hints.iter().enumerate().take(12) {
                println!("  {}. {}", i + 1, hint);
            }
            if hints.len() > 12 {
                println!("  ... and {} more", hints.len() - 12);
            }
        }
        Ok(())
    }

    pub async fn cmd_getting_started(&self) -> Result<()> {
        let game_hint = self
            .active_game()
            .await
            .map(|g| g.id)
            .or_else(|| self.games.first().map(|g| g.id.clone()))
            .unwrap_or_else(|| "<game_id>".to_string());

        println!("ModSanity Getting Started");
        println!("{:-<60}", "");
        println!("Step 1: scan and choose game");
        println!("  modsanity game scan");
        println!("  modsanity game list");
        println!("  modsanity game select {}", game_hint);
        println!();
        println!("Step 2: run diagnostics");
        println!("  modsanity doctor --verbose");
        println!();
        println!("Step 3: set paths if needed");
        println!("  modsanity deployment set-downloads-dir <path>");
        println!("  modsanity deployment set-staging-dir <path>");
        println!();
        println!("Step 4: install and deploy");
        println!("  modsanity mod install /path/to/mod.zip");
        println!("  modsanity deploy");
        println!();
        println!("Optional (GOG/manual path registration):");
        println!(
            "  modsanity game add-path {} <install_path> --platform gog",
            game_hint
        );
        println!("  modsanity game add-path {} <install_path> --platform manual --proton-prefix <prefix>", game_hint);
        println!();
        println!("Interactive mode:");
        println!("  modsanity");
        println!();
        println!("You can also run: modsanity help getting-started");
        Ok(())
    }

    pub async fn cmd_init(
        &mut self,
        interactive: bool,
        game_id: Option<&str>,
        platform: &str,
        game_path: Option<&str>,
        downloads_dir: Option<&str>,
        staging_dir: Option<&str>,
        proton_prefix: Option<&str>,
    ) -> Result<()> {
        fn ask(prompt: &str, default: Option<&str>) -> Result<String> {
            use std::io::{self, Write};
            if let Some(d) = default {
                print!("{} [{}]: ", prompt, d);
            } else {
                print!("{}: ", prompt);
            }
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            let v = buf.trim();
            if v.is_empty() {
                Ok(default.unwrap_or("").to_string())
            } else {
                Ok(v.to_string())
            }
        }

        println!("ModSanity Init");
        println!("{:-<60}", "");

        let mut chosen_game_id = game_id.map(str::to_string);
        let mut chosen_platform = platform.to_string();
        let mut chosen_game_path = game_path.map(str::to_string);
        let mut chosen_downloads = downloads_dir.map(str::to_string);
        let mut chosen_staging = staging_dir.map(str::to_string);
        let mut chosen_prefix = proton_prefix.map(str::to_string);

        if interactive {
            if chosen_platform.trim().is_empty() {
                chosen_platform = "steam".to_string();
            }
            let p = ask("Platform (steam/gog/manual)", Some(&chosen_platform))?;
            chosen_platform = if p.trim().is_empty() {
                "steam".to_string()
            } else {
                p
            };

            if chosen_game_id.is_none() {
                let default_game_id = self.games.first().map(|g| g.id.as_str());
                let gid = ask(
                    "Game ID (supported: skyrimse, skyrimvr, fallout4, fallout4vr, starfield)",
                    default_game_id,
                )?;
                if !gid.trim().is_empty() {
                    chosen_game_id = Some(gid);
                }
            }
            if chosen_game_path.is_none() && !matches!(chosen_platform.as_str(), "steam") {
                let gp = ask("Game install path", None)?;
                if !gp.trim().is_empty() {
                    chosen_game_path = Some(gp);
                }
            }
            if chosen_downloads.is_none() {
                let dd = ask("Downloads dir override (optional)", Some(""))?;
                if !dd.trim().is_empty() {
                    chosen_downloads = Some(dd);
                }
            }
            if chosen_staging.is_none() {
                let sd = ask("Staging dir override (optional)", Some(""))?;
                if !sd.trim().is_empty() {
                    chosen_staging = Some(sd);
                }
            }
            if chosen_prefix.is_none() && !matches!(chosen_platform.as_str(), "steam") {
                let pp = ask("Proton prefix (optional)", Some(""))?;
                if !pp.trim().is_empty() {
                    chosen_prefix = Some(pp);
                }
            }
        }

        self.cmd_game_scan().await?;

        if let Some(path) = chosen_downloads.as_deref() {
            self.cmd_set_downloads_dir(path).await?;
        }
        if let Some(path) = chosen_staging.as_deref() {
            self.cmd_set_staging_dir(path).await?;
        }

        if let (Some(game_id), Some(game_path)) =
            (chosen_game_id.as_deref(), chosen_game_path.as_deref())
        {
            self.cmd_game_add_path(
                game_id,
                game_path,
                &chosen_platform,
                chosen_prefix.as_deref(),
            )
            .await?;
            self.cmd_game_scan().await?;
            self.cmd_game_select(game_id).await?;
        } else if let Some(game_id) = chosen_game_id.as_deref() {
            self.cmd_game_select(game_id).await?;
        } else if self.active_game().await.is_none() {
            if let Some(first) = self.games.first().cloned() {
                self.set_active_game(Some(first.clone())).await?;
                println!("Selected detected game: {} ({})", first.name, first.id);
            }
        }

        println!("{:-<60}", "");
        println!("Init complete. Next commands:");
        println!("  1. modsanity doctor --verbose");
        println!("  2. modsanity mod list");
        println!("  3. modsanity deploy");
        println!("  4. modsanity  (launch TUI)");
        self.mark_init_completed().await?;
        Ok(())
    }

    pub async fn cmd_audit(&self, dry_run: bool) -> Result<()> {
        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("ModSanity Audit");
        println!("{:-<60}", "");
        println!(
            "Mode: {}",
            if dry_run {
                "dry-run (no writes)"
            } else {
                "live"
            }
        );
        println!("Game: {} ({})", game.name, game.id);

        let mods = self.mods.list_mods(&game.id).await?;
        let enabled_mods = mods.iter().filter(|m| m.enabled).count();
        println!("Mods: {} total, {} enabled", mods.len(), enabled_mods);

        let plugins = crate::plugins::get_plugins(&game).unwrap_or_default();
        let enabled_plugins = plugins.iter().filter(|p| p.enabled).count();
        println!(
            "Plugins: {} total, {} enabled",
            plugins.len(),
            enabled_plugins
        );

        let show_full_trace = dry_run || self.cli_verbosity >= 2;
        let missing_limit = if show_full_trace { usize::MAX } else { 10 };
        let order_limit = if show_full_trace { usize::MAX } else { 10 };
        let conflict_limit = if show_full_trace { usize::MAX } else { 5 };

        let missing_masters = crate::plugins::check_missing_masters(&plugins);
        println!("Missing masters: {}", missing_masters.len());
        for (plugin, missing) in missing_masters.iter().take(missing_limit) {
            println!("  - {} -> {}", plugin, missing.join(", "));
        }
        if missing_masters.len() > missing_limit {
            println!("  ... and {} more", missing_masters.len() - missing_limit);
        }

        let order_issues = crate::plugins::validate_load_order(&plugins);
        println!("Load-order issues: {}", order_issues.len());
        for issue in order_issues.iter().take(order_limit) {
            println!("  - {}", issue);
        }
        if order_issues.len() > order_limit {
            println!("  ... and {} more", order_issues.len() - order_limit);
        }

        let conflicts = crate::mods::get_conflicts_grouped(&self.db, &game.id)?;
        let conflict_files: usize = conflicts.iter().map(|c| c.files.len()).sum();
        println!(
            "Conflicts: {} mod-pair conflicts, {} files affected",
            conflicts.len(),
            conflict_files
        );
        for conflict in conflicts.iter().take(conflict_limit) {
            println!(
                "  - {} vs {} ({} files, winner: {})",
                conflict.mod1,
                conflict.mod2,
                conflict.files.len(),
                conflict.winner
            );
        }
        if conflicts.len() > conflict_limit {
            println!("  ... and {} more", conflicts.len() - conflict_limit);
        }

        if dry_run {
            println!("Audit complete (no changes were made).");
        } else {
            println!("Audit complete.");
        }
        Ok(())
    }

    // ========== Modlist Commands ==========

    pub async fn cmd_modlist_save(&self, path: &str, format: &str) -> Result<()> {
        use crate::import::modlist_format::{
            ModSanityModlist, ModlistEntry, ModlistMeta, PluginOrderEntry,
        };
        use crate::plugins;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let out_path = std::path::Path::new(path);

        match format {
            "native" | "json" => {
                let mods: Vec<_> = self
                    .mods
                    .list_mods(&game.id)
                    .await?
                    .into_iter()
                    .filter(|m| m.enabled)
                    .collect();

                // Get category names for installed mods
                let categories = self.db.get_all_categories()?;
                let cat_map: std::collections::HashMap<i64, String> = categories
                    .into_iter()
                    .filter_map(|c| c.id.map(|id| (id, c.name)))
                    .collect();

                let mod_entries: Vec<ModlistEntry> = mods
                    .iter()
                    .map(|m| ModlistEntry {
                        name: m.name.clone(),
                        version: m.version.clone(),
                        nexus_mod_id: m.nexus_mod_id,
                        nexus_file_id: m.nexus_file_id,
                        author: m.author.clone(),
                        priority: m.priority,
                        enabled: m.enabled,
                        category: m.category_id.and_then(|id| cat_map.get(&id).cloned()),
                    })
                    .collect();

                // Get plugins from game data directory
                let plugin_entries: Vec<PluginOrderEntry> = match plugins::get_plugins(&game) {
                    Ok(plist) => plist
                        .iter()
                        .map(|p| PluginOrderEntry {
                            filename: p.filename.clone(),
                            load_order: p.load_order as i32,
                            enabled: p.enabled,
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                };

                let profile_name = self.config.read().await.active_profile.clone();

                let modlist = ModSanityModlist {
                    meta: ModlistMeta {
                        format_version: 1,
                        modsanity_version: crate::APP_VERSION.to_string(),
                        game_id: game.id.clone(),
                        game_domain: game.nexus_game_domain(),
                        exported_at: chrono::Utc::now().to_rfc3339(),
                        profile_name,
                    },
                    mods: mod_entries,
                    plugins: plugin_entries,
                };

                crate::import::modlist_format::save_native(out_path, &modlist)?;
                let db_entries: Vec<crate::db::ModlistEntryRecord> = modlist
                    .mods
                    .iter()
                    .enumerate()
                    .map(|(i, m)| crate::db::ModlistEntryRecord {
                        id: None,
                        modlist_id: 0,
                        name: m.name.clone(),
                        nexus_mod_id: m.nexus_mod_id,
                        plugin_name: None,
                        match_confidence: None,
                        position: i as i32,
                        enabled: m.enabled,
                        author: m.author.clone(),
                        version: Some(m.version.clone()),
                    })
                    .collect();
                let modlist_name = Self::modlist_name_from_path(path, "Saved Modlist");
                self.persist_modlist_to_db(&game.id, &modlist_name, None, &db_entries)?;
                println!("Saved native modlist to: {}", path);
                println!(
                    "  {} mods, {} plugins",
                    modlist.mods.len(),
                    modlist.plugins.len()
                );
                println!("Stored in database as: {}", modlist_name);
            }
            "mo2" => {
                // Write MO2 modlist.txt format (plugin list)
                let plugin_list = match plugins::get_plugins(&game) {
                    Ok(plist) => plist,
                    Err(e) => bail!("Failed to read plugins: {}", e),
                };

                let mut lines = Vec::new();
                for plugin in &plugin_list {
                    let prefix = if plugin.enabled { "*" } else { "" };
                    lines.push(format!("{}{}", prefix, plugin.filename));
                }

                std::fs::write(out_path, lines.join("\n"))
                    .context("Failed to write MO2 modlist file")?;

                let mods: Vec<_> = self
                    .mods
                    .list_mods(&game.id)
                    .await?
                    .into_iter()
                    .filter(|m| m.enabled)
                    .collect();
                let db_entries: Vec<crate::db::ModlistEntryRecord> = mods
                    .iter()
                    .enumerate()
                    .map(|(i, m)| crate::db::ModlistEntryRecord {
                        id: None,
                        modlist_id: 0,
                        name: m.name.clone(),
                        nexus_mod_id: m.nexus_mod_id,
                        plugin_name: None,
                        match_confidence: None,
                        position: i as i32,
                        enabled: m.enabled,
                        author: m.author.clone(),
                        version: Some(m.version.clone()),
                    })
                    .collect();
                let modlist_name = Self::modlist_name_from_path(path, "Saved Modlist");
                self.persist_modlist_to_db(&game.id, &modlist_name, None, &db_entries)?;

                println!("Saved MO2 modlist to: {}", path);
                println!("  {} plugins", plugin_list.len());
                println!("Stored in database as: {}", modlist_name);
            }
            _ => bail!("Unknown format '{}'. Use 'native' or 'mo2'.", format),
        }

        Ok(())
    }

    pub async fn cmd_modlist_load(
        &self,
        path: &str,
        auto_approve: bool,
        preview: bool,
    ) -> Result<()> {
        use crate::import::{detect_format, ModlistFormat};

        let file_path = std::path::Path::new(path);
        let format = detect_format(file_path)?;

        match format {
            ModlistFormat::Native => {
                self.cmd_modlist_load_native(path, auto_approve, preview)
                    .await
            }
            ModlistFormat::Mo2 => {
                println!("Detected MO2 format, delegating to import command...");
                self.cmd_import_modlist(path, auto_approve, preview).await
            }
        }
    }

    async fn cmd_modlist_load_native(
        &self,
        path: &str,
        auto_approve: bool,
        preview: bool,
    ) -> Result<()> {
        use crate::import::library_check;
        use crate::import::modlist_format;
        use crate::queue::QueueManager;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let modlist = modlist_format::load_native(std::path::Path::new(path))?;

        // Validate game matches
        if modlist.meta.game_id != game.id {
            bail!(
                "Modlist is for game '{}' but active game is '{}'. Select the correct game first.",
                modlist.meta.game_id,
                game.id
            );
        }

        println!("Loading native modlist from: {}", path);
        println!("Game: {} ({})", game.name, game.id);
        println!(
            "Modlist version: {}, exported: {}",
            modlist.meta.modsanity_version, modlist.meta.exported_at
        );
        println!(
            "  {} mods, {} plugins",
            modlist.mods.len(),
            modlist.plugins.len()
        );

        if preview {
            let check_result = library_check::check_library(&self.db, &game.id, modlist.mods)?;
            println!("\nPreview mode: no database or queue writes");
            println!("Library Check:");
            println!(
                "  Already installed: {}",
                check_result.already_installed.len()
            );
            println!("  Needs download:    {}", check_result.needs_download.len());
            println!(
                "  Queueable (has Nexus ID): {}",
                check_result
                    .needs_download
                    .iter()
                    .filter(|e| e.nexus_mod_id.unwrap_or_default() > 0)
                    .count()
            );
            return Ok(());
        }

        let modlist_name = Self::modlist_name_from_path(path, "Imported Native Modlist");
        let db_entries: Vec<crate::db::ModlistEntryRecord> = modlist
            .mods
            .iter()
            .enumerate()
            .map(|(i, m)| crate::db::ModlistEntryRecord {
                id: None,
                modlist_id: 0,
                name: m.name.clone(),
                nexus_mod_id: m.nexus_mod_id,
                plugin_name: None,
                match_confidence: None,
                position: i as i32,
                enabled: m.enabled,
                author: m.author.clone(),
                version: Some(m.version.clone()),
            })
            .collect();
        self.persist_modlist_to_db(&game.id, &modlist_name, Some(path), &db_entries)?;
        println!("Stored in database as: {}", modlist_name);

        // Library check: skip already-installed mods
        let check_result = library_check::check_library(&self.db, &game.id, modlist.mods)?;

        println!("\nLibrary Check:");
        println!(
            "  Already installed: {}",
            check_result.already_installed.len()
        );
        println!("  Needs download:    {}", check_result.needs_download.len());

        if check_result.needs_download.is_empty() {
            println!("\nAll mods are already installed!");
            return Ok(());
        }

        // Queue missing mods for download (nexus IDs already resolved, no matching needed)
        let queue_manager = QueueManager::new(self.db.clone());
        let batch_id = queue_manager.create_batch();

        println!("\nCreating download queue (batch: {})...", batch_id);

        let mut queue_position = 0;
        let mut skipped_no_id = 0;

        for entry in &check_result.needs_download {
            let nexus_mod_id = match entry.nexus_mod_id {
                Some(id) if id > 0 => id,
                _ => {
                    skipped_no_id += 1;
                    continue;
                }
            };

            let queue_entry = crate::queue::QueueEntry {
                id: 0,
                batch_id: batch_id.clone(),
                game_id: game.id.clone(),
                queue_position,
                plugin_name: entry.name.clone(),
                mod_name: entry.name.clone(),
                nexus_mod_id,
                selected_file_id: entry.nexus_file_id,
                auto_install: true,
                match_confidence: Some(1.0),
                alternatives: Vec::new(),
                status: crate::queue::QueueStatus::Matched,
                progress: 0.0,
                error: None,
            };

            queue_manager.add_entry(queue_entry)?;
            queue_position += 1;
        }

        println!("Added {} entries to download queue", queue_position);
        if skipped_no_id > 0 {
            println!("Skipped {} entries without Nexus mod IDs", skipped_no_id);
        }

        if auto_approve {
            println!("\nAuto-approve enabled. Processing queue...");
            self.cmd_queue_process(Some(&batch_id), false).await?;
        } else {
            println!(
                "\nQueue created. Use 'modsanity queue process --batch-id {}' to start downloads",
                batch_id
            );
        }

        Ok(())
    }

    // ========== Import Commands ==========

    pub async fn cmd_import_modlist(
        &self,
        path: &str,
        auto_approve: bool,
        preview: bool,
    ) -> Result<()> {
        use crate::import::ModlistImporter;
        use crate::queue::QueueManager;
        use std::path::Path;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let nexus = match &self.nexus {
            Some(client) => client.clone(),
            None => bail!("NexusMods API key not configured. Set NEXUS_API_KEY environment variable or add to config."),
        };

        println!("Importing modlist from: {}", path);
        println!("Game: {} ({})", game.name, game.id);

        let importer =
            ModlistImporter::with_catalog(&game.id, (*nexus).clone(), Some(self.db.clone()));
        let started = std::time::Instant::now();
        let result = importer
            .import_modlist_with_progress(
                Path::new(path),
                Some(|current: usize, total: usize, plugin: &str| {
                    if current == 1 || current % 25 == 0 || current == total {
                        println!("Matching {:>4}/{}: {}", current, total.max(current), plugin);
                    }
                }),
            )
            .await?;
        println!(
            "Matching completed in {:.1}s",
            started.elapsed().as_secs_f32()
        );

        println!("\nImport Results:");
        println!("{:-<60}", "");
        println!("Total plugins: {}", result.total_plugins);
        println!("Auto-matched:  {}", result.auto_matched().count());
        println!("Needs review:  {}", result.needs_review().count());
        println!("No matches:    {}", result.no_matches().count());

        if preview {
            println!("\nPreview mode: no database or queue writes");
            let queueable = result
                .matches
                .iter()
                .filter(|m| {
                    m.best_match
                        .as_ref()
                        .map(|bm| bm.mod_id > 0)
                        .unwrap_or(false)
                })
                .count();
            println!("Queueable matches (with Nexus ID): {}", queueable);
            return Ok(());
        }

        let modlist_name = Self::modlist_name_from_path(path, "Imported Modlist");
        let db_entries: Vec<crate::db::ModlistEntryRecord> = result
            .matches
            .iter()
            .enumerate()
            .map(|(i, m)| crate::db::ModlistEntryRecord {
                id: None,
                modlist_id: 0,
                name: m.mod_name.clone(),
                nexus_mod_id: m.best_match.as_ref().map(|bm| bm.mod_id),
                plugin_name: Some(m.plugin.plugin_name.clone()),
                match_confidence: Some(m.confidence.score()),
                position: i as i32,
                enabled: true,
                author: m.best_match.as_ref().map(|bm| bm.author.clone()),
                version: m.best_match.as_ref().map(|bm| bm.version.clone()),
            })
            .collect();
        self.persist_modlist_to_db(&game.id, &modlist_name, Some(path), &db_entries)?;
        println!("Stored in database as: {}", modlist_name);

        // Library check: skip mods that are already installed
        let matched_nexus_ids: Vec<i64> = result
            .matches
            .iter()
            .filter_map(|m| m.best_match.as_ref().map(|bm| bm.mod_id))
            .filter(|id| *id > 0)
            .collect();

        let installed_mods = self
            .db
            .find_mods_by_nexus_ids(&game.id, &matched_nexus_ids)?;
        let installed_count = installed_mods.len();

        if installed_count > 0 {
            println!("Already installed: {} (will be skipped)", installed_count);
        }

        // Create download queue batch
        let queue_manager = QueueManager::new(self.db.clone());
        let batch_id = queue_manager.create_batch();

        println!("\nCreating download queue (batch: {})...", batch_id);

        let mut queue_position = 0;
        let mut skipped = 0;
        for match_result in &result.matches {
            // Skip already-installed mods
            if let Some(best_match) = &match_result.best_match {
                if installed_mods.contains_key(&best_match.mod_id) {
                    skipped += 1;
                    continue;
                }
            }

            let alternatives = match_result
                .alternatives
                .iter()
                .map(|alt| crate::queue::QueueAlternative {
                    mod_id: alt.mod_id,
                    name: alt.name.clone(),
                    summary: alt.summary.clone(),
                    downloads: alt.downloads,
                    score: alt.score,
                    thumbnail_url: None,
                })
                .collect();

            let (mod_name, nexus_mod_id, status) =
                if let Some(best_match) = &match_result.best_match {
                    (
                        best_match.name.clone(),
                        best_match.mod_id,
                        if match_result.confidence.is_high() {
                            crate::queue::QueueStatus::Matched
                        } else if match_result.confidence.needs_review() {
                            crate::queue::QueueStatus::NeedsReview
                        } else {
                            crate::queue::QueueStatus::NeedsManual
                        },
                    )
                } else {
                    (
                        match_result.mod_name.clone(),
                        0,
                        crate::queue::QueueStatus::NeedsManual,
                    )
                };

            let entry = crate::queue::QueueEntry {
                id: 0,
                batch_id: batch_id.clone(),
                game_id: game.id.clone(),
                queue_position,
                plugin_name: match_result.plugin.plugin_name.clone(),
                mod_name,
                nexus_mod_id,
                selected_file_id: None,
                auto_install: true,
                match_confidence: Some(match_result.confidence.score()),
                alternatives,
                status,
                progress: 0.0,
                error: None,
            };

            queue_manager.add_entry(entry)?;
            queue_position += 1;
        }

        println!("Added {} entries to download queue", queue_position);
        if skipped > 0 {
            println!("Skipped {} already-installed mods", skipped);
        }

        if auto_approve {
            println!("\nAuto-approve enabled. Processing queue...");
            self.cmd_queue_process(Some(&batch_id), false).await?;
        } else {
            println!(
                "\nQueue created. Use 'modsanity queue process --batch-id {}' to start downloads",
                batch_id
            );
            println!(
                "Or use 'modsanity import status {}' to review matches",
                batch_id
            );
        }

        Ok(())
    }

    pub async fn cmd_import_status(&self, batch_id: Option<&str>) -> Result<()> {
        use crate::queue::QueueManager;

        let queue_manager = QueueManager::new(self.db.clone());
        let batch = match batch_id {
            Some(id) => id.to_string(),
            None => {
                let active_game = self.active_game().await;
                let game_filter = active_game.as_ref().map(|g| g.id.as_str());
                let batches = queue_manager.list_batches(game_filter)?;
                if let Some(latest) = batches.first() {
                    println!(
                        "No batch ID provided. Showing latest batch: {}",
                        latest.batch_id
                    );
                    latest.batch_id.clone()
                } else {
                    println!("No import batches found.");
                    return Ok(());
                }
            }
        };

        let entries = queue_manager.get_batch(&batch)?;

        if entries.is_empty() {
            println!("No entries found for batch: {}", batch);
            return Ok(());
        }

        println!("Import Batch: {}", batch);
        println!("{:-<80}", "");

        for entry in entries {
            let status_icon = match entry.status {
                crate::queue::QueueStatus::Completed => "✓",
                crate::queue::QueueStatus::Failed => "✗",
                crate::queue::QueueStatus::NeedsReview => "⚠",
                crate::queue::QueueStatus::NeedsManual => "!",
                crate::queue::QueueStatus::Downloading => "↓",
                _ => "○",
            };

            println!(
                "{} {} -> {}",
                status_icon, entry.plugin_name, entry.mod_name
            );

            if entry.match_confidence.is_some() {
                println!(
                    "   Confidence: {:.1}%",
                    entry.match_confidence.unwrap() * 100.0
                );
            }

            if !entry.alternatives.is_empty() {
                println!("   {} alternative(s) available", entry.alternatives.len());
            }

            if let Some(err) = &entry.error {
                println!("   Error: {}", err);
            }
        }

        Ok(())
    }

    pub async fn cmd_import_apply_enabled(&self, path: &str, preview: bool) -> Result<()> {
        use crate::import::ModlistParser;
        use std::collections::{HashMap, HashSet};
        use std::path::Path;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        println!("Applying MO2 enabled-state bridge from: {}", path);
        println!("Game: {} ({})", game.name, game.id);

        let parser = ModlistParser::new();
        let entries = parser.parse_file(Path::new(path))?;
        if entries.is_empty() {
            println!("No plugin entries found in file.");
            return Ok(());
        }

        let mut desired_by_mod: HashMap<String, bool> = HashMap::new();
        let mut unresolved_plugins = 0usize;
        let mut ambiguous_plugins = 0usize;
        let mut seen_mods = HashSet::new();

        for entry in &entries {
            let hits = self
                .db
                .find_mods_by_plugin_filename(&game.id, &entry.plugin_name)?;
            if hits.is_empty() {
                unresolved_plugins += 1;
                continue;
            }
            if hits.len() > 1 {
                ambiguous_plugins += 1;
            }
            // Use top-ranked hit from DB query ordering.
            let mod_name = hits[0].mod_name.clone();
            seen_mods.insert(mod_name.clone());
            desired_by_mod.entry(mod_name).or_insert(entry.enabled);
        }

        let installed = self.mods.list_mods(&game.id).await?;
        let mut to_enable = Vec::new();
        let mut to_disable = Vec::new();
        for m in &installed {
            if let Some(desired_enabled) = desired_by_mod.get(&m.name) {
                if *desired_enabled && !m.enabled {
                    to_enable.push(m.name.clone());
                } else if !*desired_enabled && m.enabled {
                    to_disable.push(m.name.clone());
                }
            }
        }

        println!("Bridge summary:");
        println!("  Parsed plugins:           {}", entries.len());
        println!("  Resolved installed mods:  {}", seen_mods.len());
        println!("  Unresolved plugins:       {}", unresolved_plugins);
        println!("  Ambiguous plugin matches: {}", ambiguous_plugins);
        println!("  Mods to enable:           {}", to_enable.len());
        println!("  Mods to disable:          {}", to_disable.len());
        if preview {
            println!("Preview mode: no mod state changes were applied.");
            return Ok(());
        }

        for name in &to_enable {
            self.mods.enable_mod(&game.id, name).await?;
        }
        for name in &to_disable {
            self.mods.disable_mod(&game.id, name).await?;
        }

        println!(
            "Applied bridge changes: {} enabled, {} disabled.",
            to_enable.len(),
            to_disable.len()
        );
        println!("Run 'modsanity deploy' to apply changes to game files.");
        Ok(())
    }

    // ========== Queue Commands ==========

    pub async fn cmd_queue_list(&self) -> Result<()> {
        use crate::queue::QueueManager;

        let queue_manager = QueueManager::new(self.db.clone());
        let active_game = self.active_game().await;
        let game_id = active_game.as_ref().map(|g| g.id.as_str());
        let batches = queue_manager.list_batches(game_id)?;

        if batches.is_empty() {
            if let Some(game) = active_game {
                println!("No queue batches found for {}.", game.name);
            } else {
                println!("No queue batches found.");
            }
            return Ok(());
        }

        println!("Queue Batches:");
        println!("{:-<100}", "");
        for batch in batches {
            println!(
                "Batch: {}\n  Game: {}\n  Total: {} | Pending: {} | Matched: {} | Review: {} | Manual: {}\n  Active: {} downloading, {} installing | Done: {} completed, {} failed\n  Created: {}",
                batch.batch_id,
                batch.game_id,
                batch.total,
                batch.pending,
                batch.matched,
                batch.needs_review,
                batch.needs_manual,
                batch.downloading,
                batch.installing,
                batch.completed,
                batch.failed,
                batch.created_at,
            );
            println!("{:-<100}", "");
        }

        Ok(())
    }

    pub async fn cmd_queue_process(
        &self,
        batch_id: Option<&str>,
        download_only: bool,
    ) -> Result<()> {
        use crate::queue::{QueueManager, QueueProcessor};

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let nexus = match &self.nexus {
            Some(client) => client.clone(),
            None => bail!("NexusMods API key not configured."),
        };

        let config = self.config.read().await;
        let download_dir = config.downloads_dir();

        let game_domain = game.nexus_game_domain();
        let processor = QueueProcessor::new(
            self.db.clone(),
            (*nexus).clone(),
            game_domain,
            game.id.clone(),
            download_dir,
            self.mods.clone(),
        );

        let batches: Vec<String> = match batch_id {
            Some(id) => vec![id.to_string()],
            None => {
                let queue_manager = QueueManager::new(self.db.clone());
                let summaries = queue_manager.list_batches(Some(&game.id))?;
                summaries.into_iter().map(|s| s.batch_id).collect()
            }
        };

        if batches.is_empty() {
            println!("No queue batches found for {}.", game.name);
            return Ok(());
        }

        if download_only {
            println!("Download-only mode enabled");
        }

        for batch in &batches {
            println!("Processing batch: {}", batch);
            processor.process_batch(batch, download_only).await?;
        }

        println!("Processed {} batch(es).", batches.len());
        Ok(())
    }

    pub async fn cmd_queue_retry(&self) -> Result<()> {
        use crate::queue::QueueManager;

        let game = match self.active_game().await {
            Some(g) => g,
            None => bail!("No game selected. Use 'modsanity game select <name>' first."),
        };

        let queue_manager = QueueManager::new(self.db.clone());
        let failed_batches = queue_manager.failed_batches(Some(&game.id))?;

        if failed_batches.is_empty() {
            println!("No failed downloads found for {}.", game.name);
            return Ok(());
        }

        let mut total_retried = 0usize;
        for batch_id in failed_batches {
            let retried = queue_manager.retry_failed_in_batch(&batch_id)?;
            if retried == 0 {
                continue;
            }

            total_retried += retried;
            println!("Retrying {} failed entries in batch {}", retried, batch_id);
            self.cmd_queue_process(Some(&batch_id), false).await?;
        }

        if total_retried == 0 {
            println!("No failed entries were eligible for retry.");
        } else {
            println!("Retried {} failed download(s).", total_retried);
        }

        Ok(())
    }

    pub async fn cmd_queue_clear(&self, batch_id: Option<&str>) -> Result<()> {
        use crate::queue::QueueManager;

        let queue_manager = QueueManager::new(self.db.clone());

        if let Some(batch) = batch_id {
            println!("Clearing batch: {}", batch);
            queue_manager.clear_batch(batch)?;
            println!("Batch cleared");
        } else {
            let batches = queue_manager.list_batches(None)?;
            if batches.is_empty() {
                println!("No queue batches found.");
                return Ok(());
            }

            let mut cleared = 0usize;
            for batch in &batches {
                queue_manager.clear_batch(&batch.batch_id)?;
                cleared += 1;
            }
            println!("Cleared {} batch(es).", cleared);
        }

        Ok(())
    }

    // ========== Nexus Catalog Commands ==========

    pub async fn cmd_nexus_populate(
        &self,
        game_domain: &str,
        reset: bool,
        per_page: i32,
        max_pages: Option<i32>,
    ) -> Result<()> {
        use crate::nexus::{CatalogPopulator, NexusRestClient, PopulateOptions};

        // Get API key
        let api_key = match &self.config.read().await.nexus_api_key {
            Some(key) => key.clone(),
            None => bail!("NexusMods API key not configured. Set NEXUS_API_KEY environment variable or add to config."),
        };

        // Create REST client
        let rest_client =
            NexusRestClient::new(&api_key).context("Failed to create REST API client")?;

        // Create populator
        let populator =
            CatalogPopulator::new(self.db.clone(), rest_client, game_domain.to_string())?;

        // Set up options
        let options = PopulateOptions {
            reset,
            per_page,
            max_pages,
            delay_between_pages_ms: 500,
        };

        println!("Nexus Mods Catalog Population");
        println!("{:-<60}", "");
        println!("Game domain:  {}", game_domain);
        println!("Mods per page: {}", per_page);
        if let Some(max) = max_pages {
            println!("Max pages:    {}", max);
        } else {
            println!("Max pages:    unlimited");
        }
        if reset {
            println!("Mode:         RESET (starting from beginning)");
        } else {
            println!("Mode:         RESUME (continuing from checkpoint)");
        }
        println!("{:-<60}", "");
        println!();

        // Run population with terminal status feedback.
        let reporter = std::sync::Mutex::new(CliStatusReporter::new(Duration::from_millis(300)));
        let progress_callback =
            |pages: i32, inserted: i64, updated: i64, total: i64, _offset: i32| {
                if let Ok(mut guard) = reporter.lock() {
                    let _ = guard.emit_catalog_progress(pages, inserted, updated, total);
                }
            };

        let stats = populator.populate(options, Some(progress_callback)).await?;
        if let Ok(mut guard) = reporter.lock() {
            let _ = guard.finish();
        }

        // Display results
        println!();
        println!("Population Complete!");
        println!("{:-<60}", "");
        println!("Pages fetched:   {}", stats.pages_fetched);
        println!("Mods inserted:   {}", stats.mods_inserted);
        println!("Mods updated:    {}", stats.mods_updated);
        println!("Total mods:      {}", stats.total_mods);
        println!("{:-<60}", "");

        Ok(())
    }

    pub async fn cmd_nexus_status(&self, game_domain: &str) -> Result<()> {
        // Validate game domain
        if !game_domain
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            bail!("Invalid game domain: must contain only lowercase letters, numbers, hyphens, and underscores");
        }

        println!("Nexus Catalog Status");
        println!("{:-<60}", "");
        println!("Game domain: {}", game_domain);
        println!();

        // Get sync state
        let state = self.db.get_sync_state(game_domain)?;

        if state.completed {
            println!("Status:      ✓ Completed");
        } else {
            println!("Status:      In progress / Incomplete");
        }

        println!("Current page: {}", state.current_page);

        if let Some(last_sync) = &state.last_sync {
            println!("Last sync:    {}", last_sync);
        } else {
            println!("Last sync:    Never");
        }

        if let Some(error) = &state.last_error {
            println!("Last error:   {}", error);
        }

        // Get mod count
        let count = self.db.count_catalog_mods(game_domain)?;
        println!();
        println!("Total mods:   {}", count);
        println!("{:-<60}", "");

        if !state.completed {
            println!();
            println!("To resume: modsanity nexus populate --game {}", game_domain);
            println!(
                "To restart: modsanity nexus populate --game {} --reset",
                game_domain
            );
        }

        Ok(())
    }
}
