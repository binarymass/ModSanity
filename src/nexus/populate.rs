//! Catalog population orchestrator

use super::rest::NexusRestClient;
use crate::db::{Database, NexusCatalogRecord};
use anyhow::{bail, Context, Result};
use chrono;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Options for catalog population
#[derive(Debug, Clone)]
pub struct PopulateOptions {
    pub reset: bool,
    pub per_page: i32,
    pub max_pages: Option<i32>,
    pub delay_between_pages_ms: u64,
}

impl Default for PopulateOptions {
    fn default() -> Self {
        Self {
            reset: false,
            per_page: 100,
            max_pages: None,
            delay_between_pages_ms: 500,
        }
    }
}

/// Statistics from catalog population
#[derive(Debug, Clone, Default)]
pub struct PopulateStats {
    pub pages_fetched: i32,
    pub mods_inserted: i64,
    pub mods_updated: i64,
    pub total_mods: i64,
}

/// Catalog populator
pub struct CatalogPopulator {
    db: Arc<Database>,
    rest_client: NexusRestClient,
    game_domain: String,
}

impl CatalogPopulator {
    /// Create a new catalog populator
    pub fn new(
        db: Arc<Database>,
        rest_client: NexusRestClient,
        game_domain: String,
    ) -> Result<Self> {
        // Validate game domain (security: prevent injection attacks)
        if !is_valid_game_domain(&game_domain) {
            bail!("Invalid game domain: must contain only lowercase letters, numbers, hyphens, and underscores");
        }

        if game_domain.len() > 50 {
            bail!("Invalid game domain: must be 50 characters or less");
        }

        Ok(Self {
            db,
            rest_client,
            game_domain,
        })
    }

    /// Populate the catalog with optional progress callback
    pub async fn populate<F>(
        &self,
        options: PopulateOptions,
        progress_callback: Option<F>,
    ) -> Result<PopulateStats>
    where
        F: Fn(i32, i64, i64, i64, i32) + Send + Sync,
    {
        let mut stats = PopulateStats::default();

        // Get or reset sync state
        let state = if options.reset {
            tracing::info!("Resetting sync state for {}", self.game_domain);
            self.db.reset_sync_state(&self.game_domain)?
        } else {
            self.db.get_sync_state(&self.game_domain)?
        };

        // Check if already completed
        if state.completed && !options.reset {
            tracing::info!("Sync already completed for {}", self.game_domain);
            stats.total_mods = self.db.count_catalog_mods(&self.game_domain)?;
            return Ok(stats);
        }

        // Start from checkpoint
        let mut current_offset = (state.current_page - 1) * options.per_page;

        tracing::info!(
            "Starting catalog population for {} from offset {}",
            self.game_domain,
            current_offset
        );

        loop {
            // Check max_pages limit
            if let Some(max) = options.max_pages {
                if stats.pages_fetched >= max {
                    tracing::info!("Reached max_pages limit: {}", max);
                    break;
                }
            }

            // Fetch page from GraphQL API
            tracing::info!(
                "Fetching page {} (offset={}, count={}) for {}",
                stats.pages_fetched + 1,
                current_offset,
                options.per_page,
                self.game_domain
            );

            let result = match self
                .rest_client
                .fetch_mods_page(&self.game_domain, current_offset, options.per_page)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    let error_msg =
                        format!("Failed to fetch page at offset {}: {}", current_offset, e);
                    tracing::error!("{}", error_msg);
                    self.db.update_sync_error(&self.game_domain, &error_msg)?;
                    return Err(e.context(error_msg));
                }
            };

            // Empty page means we're done
            if result.mods.is_empty() {
                tracing::info!("Reached end of catalog (empty page)");
                self.db.mark_sync_complete(&self.game_domain)?;
                break;
            }

            // Convert to catalog records
            let catalog_records: Vec<NexusCatalogRecord> = result
                .mods
                .iter()
                .map(|m| {
                    // Parse updated_at timestamp to Unix timestamp if available
                    let updated_time = m.updated_at.as_ref().and_then(|s| {
                        chrono::DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.timestamp())
                    });

                    NexusCatalogRecord {
                        game_domain: self.game_domain.clone(),
                        mod_id: m.mod_id,
                        name: m.name.clone(),
                        summary: m.summary.clone(),
                        description: m.description.clone(),
                        author: m.author.clone(),
                        updated_time,
                        synced_at: String::new(), // Will be set by DB
                    }
                })
                .collect();

            // Upsert to database in transaction
            let (inserted, updated) = self
                .db
                .upsert_catalog_page(&self.game_domain, &catalog_records)
                .context("Failed to upsert catalog page")?;

            stats.mods_inserted += inserted;
            stats.mods_updated += updated;
            stats.pages_fetched += 1;

            tracing::info!(
                "Page {} complete: {} inserted, {} updated",
                stats.pages_fetched,
                inserted,
                updated
            );

            // Call progress callback if provided
            if let Some(ref callback) = progress_callback {
                callback(
                    stats.pages_fetched,
                    stats.mods_inserted,
                    stats.mods_updated,
                    result.total_count,
                    current_offset,
                );
            }

            // Update checkpoint AFTER successful DB upsert
            let new_page = stats.pages_fetched + 1;
            self.db.update_sync_page(&self.game_domain, new_page)?;

            // Check if we've reached the end
            current_offset += options.per_page;
            if current_offset >= result.total_count as i32 {
                tracing::info!(
                    "Reached end of catalog (offset {} >= total {})",
                    current_offset,
                    result.total_count
                );
                self.db.mark_sync_complete(&self.game_domain)?;
                break;
            }

            // Rate limiting delay
            if options.delay_between_pages_ms > 0 {
                sleep(Duration::from_millis(options.delay_between_pages_ms)).await;
            }
        }

        // Get final count
        stats.total_mods = self.db.count_catalog_mods(&self.game_domain)?;

        tracing::info!(
            "Catalog population complete for {}: {} pages, {} total mods ({} inserted, {} updated)",
            self.game_domain,
            stats.pages_fetched,
            stats.total_mods,
            stats.mods_inserted,
            stats.mods_updated
        );

        Ok(stats)
    }
}

/// Validate game domain format (security check)
fn is_valid_game_domain(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }

    domain
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_game_domains() {
        assert!(is_valid_game_domain("skyrim"));
        assert!(is_valid_game_domain("skyrimspecialedition"));
        assert!(is_valid_game_domain("fallout4"));
        assert!(is_valid_game_domain("fallout-4"));
        assert!(is_valid_game_domain("test_game"));
        assert!(is_valid_game_domain("game123"));
    }

    #[test]
    fn test_invalid_game_domains() {
        assert!(!is_valid_game_domain(""));
        assert!(!is_valid_game_domain("Skyrim")); // uppercase
        assert!(!is_valid_game_domain("skyrim special edition")); // spaces
        assert!(!is_valid_game_domain("skyrim/game")); // slash
        assert!(!is_valid_game_domain("game;drop table")); // injection attempt
        assert!(!is_valid_game_domain("../../../etc")); // path traversal
    }
}
