//! ModSanity - A CLI/TUI mod manager for Bethesda games on Linux
//!
//! This crate provides a complete mod management solution with:
//! - NexusMods integration for downloading mods and collections
//! - Symlink-based deployment for clean mod management
//! - FOMOD installer support
//! - Plugin load order management
//! - Profile system for different mod configurations

pub const APP_VERSION: &str = "0.1.6.5";

pub mod app;
pub mod collections;
pub mod config;
pub mod db;
pub mod games;
pub mod import;
pub mod mods;
pub mod nexus;
pub mod plugins;
pub mod profiles;
pub mod queue;
pub mod tui;

pub use app::App;
pub use config::Config;
