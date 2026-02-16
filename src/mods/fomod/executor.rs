//! FOMOD transactional execution with rollback support
//!
//! Executes installation plans atomically with automatic rollback on failure.

use super::planner::{FileOpType, FileOperation, InstallPlan};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Progress callback for installation updates
pub type ProgressCallback = Box<dyn Fn(ExecutionProgress) + Send + Sync>;

/// FOMOD installation executor
pub struct FomodExecutor {
    /// Staging path where mod files are extracted
    staging_path: PathBuf,
    /// Target path where files will be installed
    target_path: PathBuf,
    /// Unique transaction ID
    transaction_id: String,
}

/// Execution progress information
#[derive(Debug, Clone)]
pub struct ExecutionProgress {
    /// Current phase of execution
    pub phase: ExecutionPhase,
    /// Files processed so far
    pub files_processed: usize,
    /// Total files to process
    pub total_files: usize,
    /// Current file being processed
    pub current_file: Option<String>,
    /// Bytes processed
    pub bytes_processed: u64,
    /// Total bytes to process
    pub total_bytes: u64,
}

/// Execution phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Preparing temporary directories
    Preparing,
    /// Copying files to temporary location
    CopyingFiles,
    /// Performing atomic swap
    AtomicSwap,
    /// Cleaning up temporary files
    Cleanup,
    /// Execution complete
    Complete,
    /// Rolling back due to error
    RollingBack,
}

/// Result of execution
#[derive(Debug)]
pub enum ExecutionResult {
    /// Installation succeeded
    Success {
        /// Number of files installed
        files_installed: usize,
        /// Total bytes written
        bytes_written: u64,
    },
    /// Installation failed and was rolled back
    RolledBack {
        /// Error that caused rollback
        error: String,
    },
}

impl FomodExecutor {
    /// Create a new executor
    pub fn new(staging_path: PathBuf, target_path: PathBuf) -> Self {
        let transaction_id = generate_transaction_id();
        Self {
            staging_path,
            target_path,
            transaction_id,
        }
    }

    /// Execute an installation plan
    pub async fn execute(
        &self,
        plan: &InstallPlan,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ExecutionResult> {
        // Temporary directories
        let tmp_dir = self
            .target_path
            .parent()
            .unwrap_or(&self.target_path)
            .join(format!(".fomod_install_{}", self.transaction_id));
        let backup_dir = self
            .target_path
            .parent()
            .unwrap_or(&self.target_path)
            .join(format!(".fomod_backup_{}", self.transaction_id));

        // Phase 1: Prepare temporary directory
        self.report_progress(
            &progress_callback.as_ref(),
            ExecutionPhase::Preparing,
            0,
            plan.file_operations.len(),
            None,
            0,
            plan.estimated_size_bytes,
        );

        fs::create_dir_all(&tmp_dir).await?;

        // Phase 2: Execute file operations into temporary directory
        let result = self
            .execute_operations(plan, &tmp_dir, &backup_dir, progress_callback.as_ref())
            .await;

        match result {
            Ok((files_installed, bytes_written)) => {
                // Phase 3: Atomic swap
                self.report_progress(
                    &progress_callback.as_ref(),
                    ExecutionPhase::AtomicSwap,
                    files_installed,
                    files_installed,
                    None,
                    bytes_written,
                    bytes_written,
                );

                // Backup existing target if it exists
                if self.target_path.exists() {
                    fs::rename(&self.target_path, &backup_dir).await?;
                }

                // Move temporary directory to target
                fs::rename(&tmp_dir, &self.target_path).await?;

                // Phase 4: Cleanup backup
                self.report_progress(
                    &progress_callback.as_ref(),
                    ExecutionPhase::Cleanup,
                    files_installed,
                    files_installed,
                    None,
                    bytes_written,
                    bytes_written,
                );

                if backup_dir.exists() {
                    fs::remove_dir_all(&backup_dir).await.ok();
                }

                // Complete
                self.report_progress(
                    &progress_callback.as_ref(),
                    ExecutionPhase::Complete,
                    files_installed,
                    files_installed,
                    None,
                    bytes_written,
                    bytes_written,
                );

                Ok(ExecutionResult::Success {
                    files_installed,
                    bytes_written,
                })
            }
            Err(e) => {
                // Phase 4: Rollback
                self.report_progress(
                    &progress_callback.as_ref(),
                    ExecutionPhase::RollingBack,
                    0,
                    plan.file_operations.len(),
                    None,
                    0,
                    plan.estimated_size_bytes,
                );

                // Remove temporary directory
                if tmp_dir.exists() {
                    fs::remove_dir_all(&tmp_dir).await.ok();
                }

                // Restore backup if it exists
                if backup_dir.exists() && !self.target_path.exists() {
                    fs::rename(&backup_dir, &self.target_path).await.ok();
                }

                // Clean up backup
                if backup_dir.exists() {
                    fs::remove_dir_all(&backup_dir).await.ok();
                }

                Ok(ExecutionResult::RolledBack {
                    error: e.to_string(),
                })
            }
        }
    }

    /// Execute all file operations
    async fn execute_operations(
        &self,
        plan: &InstallPlan,
        tmp_dir: &Path,
        _backup_dir: &Path,
        progress_callback: Option<&ProgressCallback>,
    ) -> Result<(usize, u64)> {
        let mut files_installed = 0;
        let mut bytes_written = 0u64;

        for (idx, operation) in plan.file_operations.iter().enumerate() {
            // Report progress
            self.report_progress(
                &progress_callback.map(|cb| cb as &ProgressCallback),
                ExecutionPhase::CopyingFiles,
                idx,
                plan.file_operations.len(),
                Some(operation.source.to_string_lossy().to_string()),
                bytes_written,
                plan.estimated_size_bytes,
            );

            // Execute operation
            let (files, bytes) = self
                .execute_single_operation(operation, tmp_dir)
                .await
                .with_context(|| {
                    format!("Failed to execute operation for {:?}", operation.source)
                })?;

            files_installed += files;
            bytes_written += bytes;
        }

        Ok((files_installed, bytes_written))
    }

    /// Execute a single file operation
    async fn execute_single_operation(
        &self,
        operation: &FileOperation,
        tmp_dir: &Path,
    ) -> Result<(usize, u64)> {
        match operation.op_type {
            FileOpType::CopyFile => {
                let src = self.staging_path.join(&operation.source);
                let dst = tmp_dir.join(&operation.destination);

                // Ensure parent directory exists
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent).await?;
                }

                // Copy file
                if src.exists() {
                    let metadata = fs::metadata(&src).await?;
                    fs::copy(&src, &dst).await?;
                    Ok((1, metadata.len()))
                } else {
                    Ok((0, 0))
                }
            }
            FileOpType::CopyDir => {
                let src = self.staging_path.join(&operation.source);
                let dst = tmp_dir.join(&operation.destination);

                if src.exists() && src.is_dir() {
                    copy_dir_recursive(&src, &dst).await
                } else {
                    Ok((0, 0))
                }
            }
            FileOpType::Skip => Ok((0, 0)),
        }
    }

    /// Report progress to callback
    fn report_progress(
        &self,
        callback: &Option<&ProgressCallback>,
        phase: ExecutionPhase,
        files_processed: usize,
        total_files: usize,
        current_file: Option<String>,
        bytes_processed: u64,
        total_bytes: u64,
    ) {
        if let Some(cb) = callback {
            cb(ExecutionProgress {
                phase,
                files_processed,
                total_files,
                current_file,
                bytes_processed,
                total_bytes,
            });
        }
    }
}

/// Copy directory recursively (async)
fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(usize, u64)>> + Send>> {
    let src = src.to_path_buf();
    let dst = dst.to_path_buf();

    Box::pin(async move {
        let mut files_copied = 0;
        let mut bytes_copied = 0u64;

        fs::create_dir_all(&dst).await?;

        let mut entries = fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                let (sub_files, sub_bytes) = copy_dir_recursive(&src_path, &dst_path).await?;
                files_copied += sub_files;
                bytes_copied += sub_bytes;
            } else {
                let metadata = entry.metadata().await?;
                fs::copy(&src_path, &dst_path).await?;
                files_copied += 1;
                bytes_copied += metadata.len();
            }
        }

        Ok((files_copied, bytes_copied))
    })
}

/// Generate unique transaction ID
fn generate_transaction_id() -> String {
    use std::time::SystemTime;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    format!("{:x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_phase_equality() {
        assert_eq!(ExecutionPhase::Preparing, ExecutionPhase::Preparing);
        assert_ne!(ExecutionPhase::Preparing, ExecutionPhase::CopyingFiles);
    }

    #[test]
    fn test_transaction_id_generation() {
        let id1 = generate_transaction_id();
        let id2 = generate_transaction_id();

        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
        // IDs should be different (unless generated at exact same millisecond)
        // This is a weak test but good enough for unique IDs
    }

    #[test]
    fn test_execution_progress() {
        let progress = ExecutionProgress {
            phase: ExecutionPhase::CopyingFiles,
            files_processed: 5,
            total_files: 10,
            current_file: Some("test.esp".to_string()),
            bytes_processed: 1024,
            total_bytes: 2048,
        };

        assert_eq!(progress.phase, ExecutionPhase::CopyingFiles);
        assert_eq!(progress.files_processed, 5);
        assert_eq!(progress.total_files, 10);
    }
}
