//! MO2 Modlist Import System
//!
//! This module handles importing Mod Organizer 2 modlist.txt files,
//! extracting mod names from plugin names, and matching them with
//! NexusMods entries.

pub mod filters;
pub mod library_check;
pub mod matcher;
pub mod modlist_format;
pub mod modlist_parser;

pub use filters::PluginFilter;
pub use library_check::{check_library, LibraryCheckResult};
pub use matcher::{MatchConfidence, MatchResult, ModMatcher};
pub use modlist_format::{
    detect_format, ModSanityModlist, ModlistEntry, ModlistFormat, ModlistMeta, PluginOrderEntry,
};
pub use modlist_parser::{ModlistParser, PluginEntry};

use crate::db::Database;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Main orchestrator for MO2 modlist import
pub struct ModlistImporter {
    parser: ModlistParser,
    filter: PluginFilter,
    matcher: ModMatcher,
}

impl ModlistImporter {
    /// Create a new importer for the given game
    pub fn new(game_id: &str, nexus_client: crate::nexus::NexusClient) -> Self {
        Self::with_catalog(game_id, nexus_client, None)
    }

    /// Create a new importer with optional local catalog search
    pub fn with_catalog(
        game_id: &str,
        nexus_client: crate::nexus::NexusClient,
        db: Option<Arc<Database>>,
    ) -> Self {
        Self {
            parser: ModlistParser::new(),
            filter: PluginFilter::for_game(game_id),
            matcher: ModMatcher::with_catalog(game_id.to_string(), nexus_client, db),
        }
    }

    /// Import a modlist.txt file
    pub async fn import_modlist(&self, path: &Path) -> Result<ImportResult> {
        self.import_modlist_with_progress(path, None::<fn(usize, usize, &str)>)
            .await
    }

    /// Import a modlist.txt file with progress callback
    pub async fn import_modlist_with_progress<F>(
        &self,
        path: &Path,
        mut progress_callback: Option<F>,
    ) -> Result<ImportResult>
    where
        F: FnMut(usize, usize, &str),
    {
        // Parse modlist.txt
        if let Some(ref mut cb) = progress_callback {
            cb(0, 0, "Parsing modlist file...");
        }

        let plugins = self.parser.parse_file(path)?;

        // Filter out base game/DLC/CC plugins
        let filtered: Vec<_> = plugins
            .into_iter()
            .filter(|p| !self.filter.should_skip(&p.plugin_name))
            .collect();

        tracing::info!(
            "Parsed {} plugins, {} after filtering",
            filtered.len() + self.filter.skipped_count(),
            filtered.len()
        );

        let total_plugins = filtered.len();

        // Match each plugin to a NexusMods mod
        let mut matches = Vec::new();
        for (index, plugin) in filtered.into_iter().enumerate() {
            // Update progress
            if let Some(ref mut cb) = progress_callback {
                cb(index + 1, total_plugins, &plugin.plugin_name);
            }

            match self.matcher.match_plugin(&plugin).await {
                Ok(result) => matches.push(result),
                Err(e) => {
                    tracing::warn!("Failed to match plugin {}: {}", plugin.plugin_name, e);
                    matches.push(MatchResult::no_match(plugin));
                }
            }
        }

        Ok(ImportResult {
            total_plugins: matches.len(),
            matches,
        })
    }
}

/// Result of importing a modlist
#[derive(Debug)]
pub struct ImportResult {
    pub total_plugins: usize,
    pub matches: Vec<MatchResult>,
}

impl ImportResult {
    /// Get plugins that matched automatically
    pub fn auto_matched(&self) -> impl Iterator<Item = &MatchResult> {
        self.matches.iter().filter(|m| m.confidence.is_high())
    }

    /// Get plugins that need user review
    pub fn needs_review(&self) -> impl Iterator<Item = &MatchResult> {
        self.matches.iter().filter(|m| m.confidence.needs_review())
    }

    /// Get plugins with no matches
    pub fn no_matches(&self) -> impl Iterator<Item = &MatchResult> {
        self.matches.iter().filter(|m| m.confidence.is_none())
    }
}
