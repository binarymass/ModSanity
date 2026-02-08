//! Download Queue Management System
//!
//! Manages the download queue for mods, including state tracking,
//! persistence, and processing.

pub mod processor;
pub mod state;

pub use processor::QueueProcessor;
pub use state::{QueueState, QueueStatus};

use anyhow::Result;
use std::sync::Arc;
use crate::db::{Database, DownloadQueueEntry, MatchAlternativeRecord, QueueBatchSummary};
use uuid::Uuid;

/// Queue manager for CRUD operations on download queue
pub struct QueueManager {
    db: Arc<Database>,
}

impl QueueManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Create a new import batch
    pub fn create_batch(&self) -> String {
        Uuid::new_v4().to_string()
    }

    /// Add entry to queue
    pub fn add_entry(&self, entry: QueueEntry) -> Result<i64> {
        let db_entry = DownloadQueueEntry {
            id: None,
            game_id: entry.game_id.clone(),
            nexus_mod_id: entry.nexus_mod_id,
            nexus_file_id: entry.selected_file_id,
            name: entry.mod_name.clone(),
            filename: None,
            status: entry.status.to_string(),
            queue_position: Some(entry.queue_position),
            plugin_name: Some(entry.plugin_name.clone()),
            match_confidence: entry.match_confidence,
            import_batch_id: Some(entry.batch_id.clone()),
            selected_file_id: entry.selected_file_id,
            auto_install: entry.auto_install,
            downloaded: 0,
            size: None,
            error: None,
            started_at: None,
            completed_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let id = self.db.insert_download_queue_entry(&db_entry)?;

        // Insert alternatives if any
        if !entry.alternatives.is_empty() {
            let alt_records: Vec<_> = entry.alternatives.iter().map(|alt| {
                MatchAlternativeRecord {
                    id: None,
                    download_id: id,
                    nexus_mod_id: alt.mod_id,
                    mod_name: alt.name.clone(),
                    match_score: alt.score,
                    summary: Some(alt.summary.clone()),
                    downloads_count: Some(alt.downloads),
                    thumbnail_url: alt.thumbnail_url.clone(),
                }
            }).collect();

            self.db.insert_match_alternatives(id, &alt_records)?;
        }

        Ok(id)
    }

    /// Get all entries for a batch
    pub fn get_batch(&self, batch_id: &str) -> Result<Vec<QueueEntry>> {
        let db_entries = self.db.get_queue_entries(batch_id)?;

        let mut entries = Vec::new();
        for db_entry in db_entries {
            let alternatives_records = self.db.get_match_alternatives(db_entry.id.unwrap())?;
            let alternatives = alternatives_records.into_iter().map(|alt| {
                QueueAlternative {
                    mod_id: alt.nexus_mod_id,
                    name: alt.mod_name,
                    summary: alt.summary.unwrap_or_default(),
                    downloads: alt.downloads_count.unwrap_or(0),
                    score: alt.match_score,
                    thumbnail_url: alt.thumbnail_url,
                }
            }).collect();

            entries.push(QueueEntry {
                id: db_entry.id.unwrap(),
                batch_id: db_entry.import_batch_id.unwrap_or_default(),
                game_id: db_entry.game_id,
                queue_position: db_entry.queue_position.unwrap_or(0),
                plugin_name: db_entry.plugin_name.unwrap_or_default(),
                mod_name: db_entry.name,
                nexus_mod_id: db_entry.nexus_mod_id,
                selected_file_id: db_entry.selected_file_id,
                auto_install: db_entry.auto_install,
                match_confidence: db_entry.match_confidence,
                alternatives,
                status: QueueStatus::from_str(&db_entry.status),
                progress: if let Some(size) = db_entry.size {
                    (db_entry.downloaded as f32 / size as f32).min(1.0)
                } else {
                    0.0
                },
                error: db_entry.error,
            });
        }

        Ok(entries)
    }

    /// Update entry status
    pub fn update_status(&self, entry_id: i64, status: QueueStatus, error: Option<String>) -> Result<()> {
        self.db.update_download_status(entry_id, &status.to_string(), error.as_deref())
    }

    /// Update download progress
    pub fn update_progress(&self, entry_id: i64, downloaded: i64, total: Option<i64>) -> Result<()> {
        self.db.update_download_progress(entry_id, downloaded, total)
    }

    /// Update queue entry display name.
    pub fn update_name(&self, entry_id: i64, name: &str) -> Result<()> {
        self.db.update_download_name(entry_id, name)
    }

    /// Delete an entry
    pub fn delete_entry(&self, entry_id: i64) -> Result<()> {
        self.db.delete_download(entry_id)
    }

    /// Clear entire batch
    pub fn clear_batch(&self, batch_id: &str) -> Result<()> {
        self.db.clear_batch(batch_id)
    }

    /// List queue batches with summary counts
    pub fn list_batches(&self, game_id: Option<&str>) -> Result<Vec<QueueBatchSummary>> {
        self.db.list_queue_batches(game_id)
    }

    /// Get batches that have failed entries
    pub fn failed_batches(&self, game_id: Option<&str>) -> Result<Vec<String>> {
        self.db.get_failed_batches(game_id)
    }

    /// Reset failed entries in a batch to pending
    pub fn retry_failed_in_batch(&self, batch_id: &str) -> Result<usize> {
        self.db.retry_failed_in_batch(batch_id)
    }

    /// Resolve an entry by assigning a Nexus target and status.
    pub fn resolve_entry(
        &self,
        entry_id: i64,
        nexus_mod_id: i64,
        mod_name: &str,
        status: QueueStatus,
    ) -> Result<()> {
        self.db
            .resolve_queue_entry(entry_id, nexus_mod_id, mod_name, &status.to_string())
    }
}

/// A queue entry
#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub id: i64,
    pub batch_id: String,
    pub game_id: String,
    pub queue_position: i32,
    pub plugin_name: String,
    pub mod_name: String,
    pub nexus_mod_id: i64,
    pub selected_file_id: Option<i64>,
    pub auto_install: bool,
    pub match_confidence: Option<f32>,
    pub alternatives: Vec<QueueAlternative>,
    pub status: QueueStatus,
    pub progress: f32,
    pub error: Option<String>,
}

/// Alternative match for a queue entry
#[derive(Debug, Clone)]
pub struct QueueAlternative {
    pub mod_id: i64,
    pub name: String,
    pub summary: String,
    pub downloads: i64,
    pub score: f32,
    pub thumbnail_url: Option<String>,
}
