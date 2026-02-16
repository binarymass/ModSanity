use anyhow::Result;
use clap::{Parser, Subcommand};
use modsanity::{App, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "modsanity")]
#[command(
    author,
    version = "0.1.7",
    about = "A CLI/TUI mod manager for Bethesda games on Linux"
)]
struct Cli {
    /// Run in non-interactive mode
    #[arg(short, long)]
    batch: bool,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Runtime staging/mods directory override for this invocation
    #[arg(long)]
    mods_dir: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the interactive TUI
    Tui,

    /// Manage games
    Game {
        #[command(subcommand)]
        action: GameCommands,
    },

    /// Manage mods
    Mod {
        #[command(subcommand)]
        action: ModCommands,
    },

    /// Manage profiles
    Profile {
        #[command(subcommand)]
        action: ProfileCommands,
    },

    /// Import and manage mod downloads
    Import {
        #[command(subcommand)]
        action: ImportCommands,
    },

    /// Manage download queue
    Queue {
        #[command(subcommand)]
        action: QueueCommands,
    },

    /// Save and load modlists
    Modlist {
        #[command(subcommand)]
        action: ModlistCommands,
    },

    /// Nexus Mods catalog operations
    Nexus {
        #[command(subcommand)]
        action: NexusCommands,
    },

    /// Manage deployment settings
    Deployment {
        #[command(subcommand)]
        action: DeploymentCommands,
    },

    /// Manage and launch external tools (Proton or native runtime)
    Tool {
        #[command(subcommand)]
        action: ToolCommands,
    },

    /// Deploy mods to game directory
    Deploy {
        /// Optional deployment method override: symlink, hardlink, copy
        #[arg(long)]
        method: Option<String>,
    },

    /// Show current status
    Status,

    /// Run system diagnostics (paths, tools, runtime checks)
    Doctor {
        /// Include detailed path and runtime checks
        #[arg(long)]
        verbose: bool,
    },

    /// Guided first-run initialization
    Init {
        /// Prompt for missing values interactively
        #[arg(long)]
        interactive: bool,
        /// Game ID (e.g., skyrimse)
        #[arg(long)]
        game_id: Option<String>,
        /// Platform source: steam, gog, manual
        #[arg(long, default_value = "steam")]
        platform: String,
        /// Optional explicit game install path
        #[arg(long)]
        game_path: Option<String>,
        /// Optional downloads directory override
        #[arg(long)]
        downloads_dir: Option<String>,
        /// Optional staging directory override
        #[arg(long)]
        staging_dir: Option<String>,
        /// Optional Proton prefix path
        #[arg(long)]
        proton_prefix: Option<String>,
    },

    /// Analyze current setup without writing files
    Audit {
        /// No-write analysis mode
        #[arg(long, default_value_t = true)]
        dry_run: bool,
    },

    /// Show a practical first-run command flow
    GettingStarted,
}

#[derive(Subcommand)]
enum GameCommands {
    /// List detected games
    List,
    /// Scan for games
    Scan,
    /// Select active game
    Select { name: String },
    /// Show game info
    Info,
    /// Add a custom game install path (GOG/manual/steam override)
    AddPath {
        /// Game ID (e.g., skyrimse, fallout4)
        game_id: String,
        /// Install directory containing executable + Data folder
        path: String,
        /// Platform source: steam, gog, manual
        #[arg(long, default_value = "manual")]
        platform: String,
        /// Optional Proton prefix path
        #[arg(long)]
        proton_prefix: Option<String>,
    },
    /// Remove a custom game install path
    RemovePath {
        /// Game ID
        game_id: String,
        /// Install directory that was previously added
        path: String,
    },
}

#[derive(Subcommand)]
enum ModCommands {
    /// List installed mods
    List,
    /// Install a mod from archive
    Install { path: String },
    /// Enable a mod
    Enable { name: String },
    /// Disable a mod
    Disable { name: String },
    /// Remove a mod
    Remove { name: String },
    /// Show mod info
    Info { name: String },
    /// Scan staging folder and sync mods into the database
    Rescan,
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// List profiles
    List,
    /// Create a new profile
    Create { name: String },
    /// Switch to a profile
    Switch { name: String },
    /// Delete a profile
    Delete { name: String },
    /// Export a profile
    Export { name: String, path: String },
    /// Import a profile
    Import { path: String },
}

#[derive(Subcommand)]
enum ImportCommands {
    /// Import a MO2 modlist.txt file
    Modlist {
        /// Path to modlist.txt
        path: String,
        /// Auto-approve all matches without review
        #[arg(long)]
        auto_approve: bool,
        /// Preview matching results only (no queue/db writes)
        #[arg(long)]
        preview: bool,
    },
    /// Show import status for a batch
    Status {
        /// Batch ID (optional, shows latest if not specified)
        batch_id: Option<String>,
    },
    /// Apply MO2 plugin enabled/disabled state to currently installed mods (migration bridge)
    ApplyEnabled {
        /// Path to MO2 modlist.txt
        path: String,
        /// Preview changes only (no writes)
        #[arg(long)]
        preview: bool,
    },
}

#[derive(Subcommand)]
enum QueueCommands {
    /// List all queued downloads
    List,
    /// Process the download queue
    Process {
        /// Batch ID to process (optional, processes all if not specified)
        #[arg(long)]
        batch_id: Option<String>,
        /// Only download, don't install
        #[arg(long)]
        download_only: bool,
    },
    /// Retry failed downloads
    Retry,
    /// Clear the download queue
    Clear {
        /// Batch ID to clear (optional, clears all if not specified)
        batch_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum ModlistCommands {
    /// Save modlist to a file
    Save {
        /// Path to output file
        path: String,
        /// Format: native (JSON) or mo2 (text)
        #[arg(long, default_value = "native")]
        format: String,
    },
    /// Load modlist from a file
    Load {
        /// Path to modlist file
        path: String,
        /// Auto-approve all downloads without review
        #[arg(long)]
        auto_approve: bool,
        /// Preview import/load effects only (no queue/db writes)
        #[arg(long)]
        preview: bool,
    },
}

#[derive(Subcommand)]
enum NexusCommands {
    /// Populate local catalog with Nexus mods
    Populate {
        /// Game domain (e.g., skyrimspecialedition, fallout4)
        #[arg(short, long)]
        game: String,
        /// Reset and start from beginning
        #[arg(long)]
        reset: bool,
        /// Mods per page
        #[arg(long, default_value_t = 100)]
        per_page: i32,
        /// Maximum pages to fetch (optional, fetches all if not specified)
        #[arg(long)]
        max_pages: Option<i32>,
    },
    /// Show catalog sync status
    Status {
        /// Game domain (e.g., skyrimspecialedition, fallout4)
        #[arg(short, long)]
        game: String,
    },
}

#[derive(Subcommand)]
enum DeploymentCommands {
    /// Show current deployment settings
    Show,
    /// Set deployment method: symlink, hardlink, copy
    SetMethod { method: String },
    /// Set downloads directory override
    SetDownloadsDir { path: String },
    /// Clear downloads directory override (use default)
    ClearDownloadsDir,
    /// Set staging/installed mods directory override
    SetStagingDir { path: String },
    /// Clear staging/installed mods directory override (use default)
    ClearStagingDir,
    /// Safely migrate staging directory contents to a new path
    MigrateStaging {
        /// Source staging directory path
        from: String,
        /// Destination staging directory path
        to: String,
        /// Preview changes without copying
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum ToolCommands {
    /// Show configured external tool paths and Proton command
    Show,
    /// List detected Steam-managed Proton runtimes
    ListProton,
    /// Select a detected Steam-managed Proton runtime (or `auto`)
    UseProton { runtime: String },
    /// Clear selected Proton runtime and use custom command/path mode
    ClearProtonRuntime,
    /// Set Proton command/path
    SetProton { path: String },
    /// Set tool executable path
    SetPath { tool: String, path: String },
    /// Set per-tool runtime mode
    SetRuntime { tool: String, mode: String },
    /// Clear per-tool runtime override (defaults to proton)
    ClearRuntime { tool: String },
    /// Clear tool executable path
    ClearPath { tool: String },
    /// Launch a configured tool using its selected runtime mode
    Run {
        tool: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn setup_logging(verbosity: u8, also_stderr: bool) {
    let filter = match verbosity {
        0 => "modsanity=info",
        1 => "modsanity=debug",
        2 => "modsanity=trace",
        _ => "trace",
    };

    // Write logs to a file to avoid corrupting TUI
    let log_dir = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".modsanity");

    std::fs::create_dir_all(&log_dir).ok();
    let log_file = log_dir.join("modsanity.log");

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .expect("Failed to open log file");

    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into());
    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_writer(std::sync::Arc::new(file));

    if also_stderr {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_writer(std::io::stderr),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let is_tui = matches!(cli.command, Some(Commands::Tui) | None);
    setup_logging(cli.verbose, !is_tui);

    // Load configuration
    let mut config = Config::load().await?;
    if let Some(mods_dir) = cli.mods_dir.as_deref() {
        let trimmed = mods_dir.trim();
        if trimmed.is_empty() {
            anyhow::bail!("--mods-dir cannot be empty");
        }
        config.staging_dir_override = Some(trimmed.to_string());
    }

    // Initialize app
    let mut app = App::new(config).await?;
    app.set_cli_verbosity(cli.verbose);

    match cli.command {
        Some(Commands::Tui) | None => {
            // Launch TUI (default behavior)
            app.run_tui().await?;
        }
        Some(Commands::Game { action }) => match action {
            GameCommands::List => app.cmd_game_list().await?,
            GameCommands::Scan => app.cmd_game_scan().await?,
            GameCommands::Select { name } => app.cmd_game_select(&name).await?,
            GameCommands::Info => app.cmd_game_info().await?,
            GameCommands::AddPath {
                game_id,
                path,
                platform,
                proton_prefix,
            } => {
                app.cmd_game_add_path(&game_id, &path, &platform, proton_prefix.as_deref())
                    .await?
            }
            GameCommands::RemovePath { game_id, path } => {
                app.cmd_game_remove_path(&game_id, &path).await?
            }
        },
        Some(Commands::Mod { action }) => match action {
            ModCommands::List => app.cmd_mod_list().await?,
            ModCommands::Install { path } => app.cmd_mod_install(&path).await?,
            ModCommands::Enable { name } => app.cmd_mod_enable(&name).await?,
            ModCommands::Disable { name } => app.cmd_mod_disable(&name).await?,
            ModCommands::Remove { name } => app.cmd_mod_remove(&name).await?,
            ModCommands::Info { name } => app.cmd_mod_info(&name).await?,
            ModCommands::Rescan => app.cmd_mod_rescan().await?,
        },
        Some(Commands::Profile { action }) => match action {
            ProfileCommands::List => app.cmd_profile_list().await?,
            ProfileCommands::Create { name } => app.cmd_profile_create(&name).await?,
            ProfileCommands::Switch { name } => app.cmd_profile_switch(&name).await?,
            ProfileCommands::Delete { name } => app.cmd_profile_delete(&name).await?,
            ProfileCommands::Export { name, path } => app.cmd_profile_export(&name, &path).await?,
            ProfileCommands::Import { path } => app.cmd_profile_import(&path).await?,
        },
        Some(Commands::Import { action }) => match action {
            ImportCommands::Modlist {
                path,
                auto_approve,
                preview,
            } => app.cmd_import_modlist(&path, auto_approve, preview).await?,
            ImportCommands::Status { batch_id } => {
                app.cmd_import_status(batch_id.as_deref()).await?
            }
            ImportCommands::ApplyEnabled { path, preview } => {
                app.cmd_import_apply_enabled(&path, preview).await?
            }
        },
        Some(Commands::Queue { action }) => match action {
            QueueCommands::List => app.cmd_queue_list().await?,
            QueueCommands::Process {
                batch_id,
                download_only,
            } => {
                app.cmd_queue_process(batch_id.as_deref(), download_only)
                    .await?
            }
            QueueCommands::Retry => app.cmd_queue_retry().await?,
            QueueCommands::Clear { batch_id } => app.cmd_queue_clear(batch_id.as_deref()).await?,
        },
        Some(Commands::Modlist { action }) => match action {
            ModlistCommands::Save { path, format } => app.cmd_modlist_save(&path, &format).await?,
            ModlistCommands::Load {
                path,
                auto_approve,
                preview,
            } => app.cmd_modlist_load(&path, auto_approve, preview).await?,
        },
        Some(Commands::Nexus { action }) => match action {
            NexusCommands::Populate {
                game,
                reset,
                per_page,
                max_pages,
            } => {
                app.cmd_nexus_populate(&game, reset, per_page, max_pages)
                    .await?
            }
            NexusCommands::Status { game } => app.cmd_nexus_status(&game).await?,
        },
        Some(Commands::Deployment { action }) => match action {
            DeploymentCommands::Show => app.cmd_deployment_show().await?,
            DeploymentCommands::SetMethod { method } => {
                app.cmd_set_deployment_method(&method).await?
            }
            DeploymentCommands::SetDownloadsDir { path } => {
                app.cmd_set_downloads_dir(&path).await?
            }
            DeploymentCommands::ClearDownloadsDir => app.cmd_set_downloads_dir("").await?,
            DeploymentCommands::SetStagingDir { path } => app.cmd_set_staging_dir(&path).await?,
            DeploymentCommands::ClearStagingDir => app.cmd_set_staging_dir("").await?,
            DeploymentCommands::MigrateStaging { from, to, dry_run } => {
                app.cmd_migrate_staging(&from, &to, dry_run).await?
            }
        },
        Some(Commands::Tool { action }) => match action {
            ToolCommands::Show => app.cmd_tool_show().await?,
            ToolCommands::ListProton => app.cmd_tool_list_proton().await?,
            ToolCommands::UseProton { runtime } => app.cmd_tool_use_proton(&runtime).await?,
            ToolCommands::ClearProtonRuntime => app.cmd_tool_clear_proton_runtime().await?,
            ToolCommands::SetProton { path } => app.cmd_tool_set_proton(&path).await?,
            ToolCommands::SetPath { tool, path } => app.cmd_tool_set_path(&tool, &path).await?,
            ToolCommands::SetRuntime { tool, mode } => {
                app.cmd_tool_set_runtime(&tool, &mode).await?
            }
            ToolCommands::ClearRuntime { tool } => app.cmd_tool_clear_runtime(&tool).await?,
            ToolCommands::ClearPath { tool } => app.cmd_tool_clear_path(&tool).await?,
            ToolCommands::Run { tool, args } => app.cmd_tool_run(&tool, &args).await?,
        },
        Some(Commands::Deploy { method }) => {
            if let Some(method) = method {
                app.cmd_set_deployment_method(&method).await?;
            }
            app.cmd_deploy().await?
        }
        Some(Commands::Status) => app.cmd_status().await?,
        Some(Commands::Doctor { verbose }) => app.cmd_doctor(verbose).await?,
        Some(Commands::Init {
            interactive,
            game_id,
            platform,
            game_path,
            downloads_dir,
            staging_dir,
            proton_prefix,
        }) => {
            app.cmd_init(
                interactive,
                game_id.as_deref(),
                &platform,
                game_path.as_deref(),
                downloads_dir.as_deref(),
                staging_dir.as_deref(),
                proton_prefix.as_deref(),
            )
            .await?
        }
        Some(Commands::Audit { dry_run }) => app.cmd_audit(dry_run).await?,
        Some(Commands::GettingStarted) => app.cmd_getting_started().await?,
    }

    Ok(())
}
