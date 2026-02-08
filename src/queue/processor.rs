//! Queue processor for downloading and installing mods

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::db::Database;
use crate::mods::{InstallResult, ModManager};
use crate::nexus::NexusClient;
use crate::queue::{QueueEntry, QueueStatus, QueueManager};

/// Queue processor handles downloading and installing queued mods
pub struct QueueProcessor {
    queue_manager: QueueManager,
    nexus_client: NexusClient,
    game_domain: String,
    game_id: String,
    download_dir: PathBuf,
    mods: Arc<ModManager>,
    max_concurrent: usize,
}

impl QueueProcessor {
    pub fn new(
        db: Arc<Database>,
        nexus_client: NexusClient,
        game_domain: String,
        game_id: String,
        download_dir: PathBuf,
        mods: Arc<ModManager>,
    ) -> Self {
        Self {
            queue_manager: QueueManager::new(db),
            nexus_client,
            game_domain,
            game_id,
            download_dir,
            mods,
            max_concurrent: 3, // Download 3 mods at once
        }
    }

    /// Process all entries in a batch
    pub async fn process_batch(&self, batch_id: &str, download_only: bool) -> Result<()> {
        let entries = self.queue_manager.get_batch(batch_id)?;

        tracing::info!("Processing batch {} with {} entries", batch_id, entries.len());

        // Filter entries that are ready to download.
        // NeedsReview entries are processable when the user decides to proceed.
        let downloadable: Vec<_> = entries
            .into_iter()
            .filter(|e| {
                e.status == QueueStatus::Matched
                    || e.status == QueueStatus::Pending
                    || e.status == QueueStatus::NeedsReview
            })
            .collect();

        if downloadable.is_empty() {
            tracing::info!("No entries ready to download in batch {}", batch_id);
            return Ok(());
        }

        // Create semaphore for concurrent downloads
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let mut handles = Vec::new();

        for entry in downloadable {
            let semaphore = Arc::clone(&semaphore);
            let processor = self.clone_for_task();
            let download_only = download_only;

            let handle = tokio::spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();
                processor.process_entry(entry, download_only).await
            });

            handles.push(handle);
        }

        // Wait for all downloads to complete
        for handle in handles {
            if let Err(e) = handle.await? {
                tracing::error!("Failed to process entry: {}", e);
            }
        }

        tracing::info!("Batch {} processing complete", batch_id);
        Ok(())
    }

    /// Process a single queue entry
    async fn process_entry(&self, entry: QueueEntry, download_only: bool) -> Result<()> {
        tracing::info!("Processing entry: {} (mod_id: {})", entry.mod_name, entry.nexus_mod_id);

        let resolved_name = self.resolve_mod_name(&entry).await.unwrap_or_else(|| entry.mod_name.clone());
        if resolved_name != entry.mod_name {
            let _ = self.queue_manager.update_name(entry.id, &resolved_name);
        }

        if entry.nexus_mod_id <= 0 {
            let msg = "No Nexus mod ID (manual resolution required)".to_string();
            self.queue_manager
                .update_status(entry.id, QueueStatus::Failed, Some(msg.clone()))?;
            anyhow::bail!(msg);
        }

        // Skip entries that already exist in library by Nexus ID.
        let existing = self
            .queue_manager
            .db
            .find_mods_by_nexus_ids(&self.game_id, &[entry.nexus_mod_id])?;
        if existing.contains_key(&entry.nexus_mod_id) {
            self.queue_manager.update_status(
                entry.id,
                QueueStatus::Skipped,
                Some("Already installed".to_string()),
            )?;
            tracing::info!("Skipping {} (already installed)", entry.mod_name);
            return Ok(());
        }

        // Legacy fallback: unresolved installs may exist under a numeric name
        // without nexus_mod_id populated (e.g. "165498"). Treat these as installed
        // and upgrade metadata so they are not re-added as duplicates.
        let legacy_name = entry.nexus_mod_id.to_string();
        if let Some(mut legacy_mod) = self.queue_manager.db.get_mod(&self.game_id, &legacy_name)? {
            if legacy_mod.nexus_mod_id.is_none() || legacy_mod.nexus_mod_id == Some(entry.nexus_mod_id) {
                let target_name_conflict = !legacy_mod.name.eq_ignore_ascii_case(&resolved_name)
                    && self
                        .queue_manager
                        .db
                        .get_mod(&self.game_id, &resolved_name)?
                        .is_some();

                if !target_name_conflict && !resolved_name.trim().is_empty() {
                    legacy_mod.name = resolved_name.clone();
                }
                legacy_mod.nexus_mod_id = Some(entry.nexus_mod_id);
                if legacy_mod.nexus_file_id.is_none() {
                    legacy_mod.nexus_file_id = entry.selected_file_id;
                }
                legacy_mod.updated_at = chrono::Utc::now().to_rfc3339();
                if let Err(e) = self.queue_manager.db.update_mod(&legacy_mod) {
                    tracing::warn!("Failed to upgrade legacy mod record {}: {}", legacy_name, e);
                }

                self.queue_manager.update_status(
                    entry.id,
                    QueueStatus::Skipped,
                    Some("Already installed (legacy entry upgraded)".to_string()),
                )?;
                tracing::info!("Skipping {} (legacy install already present)", entry.mod_name);
                return Ok(());
            }
        }

        // Step 1: Get file ID if not already selected
        let file_id = if let Some(fid) = entry.selected_file_id {
            fid
        } else {
            // Get files and select main file
            match self.select_main_file(entry.nexus_mod_id).await {
                Ok(fid) => fid,
                Err(e) => {
                    tracing::error!("Failed to select file for {}: {}", entry.mod_name, e);
                    self.queue_manager.update_status(
                        entry.id,
                        QueueStatus::Failed,
                        Some(format!("No downloadable files found: {}", e)),
                    )?;
                    return Err(e);
                }
            }
        };

        // Step 2: Get download link
        self.queue_manager.update_status(entry.id, QueueStatus::Downloading, None)?;

        let download_links = match self.nexus_client
            .get_download_link(&self.game_domain, entry.nexus_mod_id, file_id)
            .await
        {
            Ok(links) => links,
            Err(e) => {
                tracing::error!("Failed to get download link for {}: {}", entry.mod_name, e);
                self.queue_manager.update_status(
                    entry.id,
                    QueueStatus::Failed,
                    Some(format!("Failed to get download link: {}", e)),
                )?;
                return Err(e);
            }
        };

        if download_links.is_empty() {
            let err = anyhow::anyhow!("No download links available");
            self.queue_manager.update_status(entry.id, QueueStatus::Failed, Some(err.to_string()))?;
            return Err(err);
        }

        // Step 3: Download file
        let download_url = &download_links[0].url;
        let filename = format!("{}-{}.zip", entry.nexus_mod_id, file_id);
        let dest_path = self.download_dir.join(&filename);

        tracing::info!("Downloading {} to {:?}", entry.mod_name, dest_path);

        let entry_id = entry.id;
        let queue_manager = self.queue_manager.clone();

        let result = NexusClient::download_file(
            download_url,
            &dest_path,
            move |downloaded, total| {
                let _ = queue_manager.update_progress(entry_id, downloaded as i64, Some(total as i64));
            },
        )
        .await;

        match result {
            Ok(_) => {
                tracing::info!("Downloaded {} successfully", entry.mod_name);
                self.queue_manager.update_status(entry.id, QueueStatus::Downloaded, None)?;
            }
            Err(e) => {
                tracing::error!("Failed to download {}: {}", entry.mod_name, e);
                self.queue_manager.update_status(
                    entry.id,
                    QueueStatus::Failed,
                    Some(format!("Download failed: {}", e)),
                )?;
                return Err(e);
            }
        }

        // Step 4: Install if requested
        if !download_only && entry.auto_install {
            self.queue_manager.update_status(entry.id, QueueStatus::Installing, None)?;

            let install_path = dest_path.to_string_lossy().to_string();
            match self
                .mods
                .install_from_archive(
                    &self.game_id,
                    &install_path,
                    None,
                    Some(entry.nexus_mod_id),
                    Some(file_id),
                    Some(&resolved_name),
                )
                .await
            {
                Ok(InstallResult::Completed(installed)) => {
                    self.queue_manager.update_status(entry.id, QueueStatus::Completed, None)?;
                    tracing::info!("Installed {} as {}", resolved_name, installed.name);
                }
                Ok(InstallResult::RequiresWizard(_)) => {
                    self.queue_manager.update_status(
                        entry.id,
                        QueueStatus::Failed,
                        Some("FOMOD wizard interaction required (use TUI install)".to_string()),
                    )?;
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("already installed") {
                        self.queue_manager.update_status(entry.id, QueueStatus::Skipped, Some(msg))?;
                    } else {
                        self.queue_manager.update_status(entry.id, QueueStatus::Failed, Some(msg.clone()))?;
                    }
                    return Err(e);
                }
            }
        } else {
            self.queue_manager.update_status(entry.id, QueueStatus::Completed, None)?;
            tracing::info!("Downloaded {} (install skipped)", entry.mod_name);
        }

        Ok(())
    }

    async fn resolve_mod_name(&self, entry: &QueueEntry) -> Option<String> {
        if entry.nexus_mod_id <= 0 {
            return Some(entry.mod_name.clone());
        }

        if let Ok(Some(catalog)) = self
            .queue_manager
            .db
            .get_catalog_mod_by_id(&self.game_domain, entry.nexus_mod_id)
        {
            let n = catalog.name.trim().to_string();
            if !n.is_empty() {
                return Some(n);
            }
        }

        match self
            .nexus_client
            .get_mod_name_by_id(&self.game_domain, entry.nexus_mod_id)
            .await
        {
            Ok(Some(name)) if !name.trim().is_empty() => Some(name.trim().to_string()),
            _ => {
                let fallback = entry.mod_name.trim();
                if fallback.is_empty() {
                    None
                } else {
                    Some(fallback.to_string())
                }
            }
        }
    }

    /// Select the main file for a mod
    async fn select_main_file(&self, mod_id: i64) -> Result<i64> {
        // Map game domain to game ID
        let game_id = match self.game_domain.as_str() {
            "skyrimspecialedition" => 1704,
            "skyrim" => 110,
            "fallout4" => 1151,
            "starfield" => 4187,
            other => anyhow::bail!("Unsupported game domain for file lookup: {}", other),
        };

        let files = self.nexus_client.get_mod_files(game_id, mod_id).await?;

        // Prefer "MAIN" category files
        let main_file = files
            .iter()
            .find(|f| f.category.to_uppercase() == "MAIN")
            .or_else(|| files.first())
            .context("No files available for mod")?;

        Ok(main_file.file_id)
    }

    /// Clone necessary fields for async task
    fn clone_for_task(&self) -> Self {
        Self {
            queue_manager: QueueManager::new(self.queue_manager.db.clone()),
            nexus_client: self.nexus_client.clone(),
            game_domain: self.game_domain.clone(),
            game_id: self.game_id.clone(),
            download_dir: self.download_dir.clone(),
            mods: Arc::clone(&self.mods),
            max_concurrent: self.max_concurrent,
        }
    }
}

impl QueueManager {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
        }
    }
}
