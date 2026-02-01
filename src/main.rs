use anyhow::Result;
use clap::{Parser, Subcommand};
use modsanity::{App, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "modsanity")]
#[command(author, version, about = "A CLI/TUI mod manager for Bethesda games on Linux")]
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

    /// Deploy mods to game directory
    Deploy,

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
        Some(Commands::Deploy) => app.cmd_deploy().await?,
        Some(Commands::Status) => app.cmd_status().await?,
    }

    Ok(())
}
