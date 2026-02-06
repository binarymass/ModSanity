use anyhow::Result;
use clap::{Parser, Subcommand};
use modsanity::{App, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "modsanity")]
#[command(author, version = "0.1.6.5", about = "A CLI/TUI mod manager for Bethesda games on Linux")]
struct Cli {
    /// Run in non-interactive mode
    #[arg(short, long)]
    batch: bool,

    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

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

    /// Deploy mods to game directory
    Deploy {
        /// Optional deployment method override: symlink, hardlink, copy
        #[arg(long)]
        method: Option<String>,
    },

    /// Show current status
    Status,
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
    },
    /// Show import status for a batch
    Status {
        /// Batch ID (optional, shows latest if not specified)
        batch_id: Option<String>,
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
}

fn setup_logging(verbosity: u8) {
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

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| filter.into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(std::sync::Arc::new(file))
        )
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    setup_logging(cli.verbose);

    // Load configuration
    let config = Config::load().await?;

    // Initialize app
    let mut app = App::new(config).await?;

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
        },
        Some(Commands::Mod { action }) => match action {
            ModCommands::List => app.cmd_mod_list().await?,
            ModCommands::Install { path } => app.cmd_mod_install(&path).await?,
            ModCommands::Enable { name } => app.cmd_mod_enable(&name).await?,
            ModCommands::Disable { name } => app.cmd_mod_disable(&name).await?,
            ModCommands::Remove { name } => app.cmd_mod_remove(&name).await?,
            ModCommands::Info { name } => app.cmd_mod_info(&name).await?,
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
            ImportCommands::Modlist { path, auto_approve } => {
                app.cmd_import_modlist(&path, auto_approve).await?
            }
            ImportCommands::Status { batch_id } => {
                app.cmd_import_status(batch_id.as_deref()).await?
            }
        },
        Some(Commands::Queue { action }) => match action {
            QueueCommands::List => app.cmd_queue_list().await?,
            QueueCommands::Process { batch_id, download_only } => {
                app.cmd_queue_process(batch_id.as_deref(), download_only).await?
            }
            QueueCommands::Retry => app.cmd_queue_retry().await?,
            QueueCommands::Clear { batch_id } => {
                app.cmd_queue_clear(batch_id.as_deref()).await?
            }
        },
        Some(Commands::Modlist { action }) => match action {
            ModlistCommands::Save { path, format } => {
                app.cmd_modlist_save(&path, &format).await?
            }
            ModlistCommands::Load { path, auto_approve } => {
                app.cmd_modlist_load(&path, auto_approve).await?
            }
        },
        Some(Commands::Nexus { action }) => match action {
            NexusCommands::Populate { game, reset, per_page, max_pages } => {
                app.cmd_nexus_populate(&game, reset, per_page, max_pages).await?
            }
            NexusCommands::Status { game } => {
                app.cmd_nexus_status(&game).await?
            }
        },
        Some(Commands::Deployment { action }) => match action {
            DeploymentCommands::Show => app.cmd_deployment_show().await?,
            DeploymentCommands::SetMethod { method } => app.cmd_set_deployment_method(&method).await?,
        },
        Some(Commands::Deploy { method }) => {
            if let Some(method) = method {
                app.cmd_set_deployment_method(&method).await?;
            }
            app.cmd_deploy().await?
        }
        Some(Commands::Status) => app.cmd_status().await?,
    }

    Ok(())
}
