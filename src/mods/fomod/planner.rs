//! FOMOD installation planning and conflict detection
//!
//! Compiles wizard selections into explicit install plans with conflict detection.

use super::{FileInstruction, FomodInstaller, ModuleConfig, WizardState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Complete installation plan for a FOMOD installer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Name of the mod being installed
    pub mod_name: String,
    /// Profile ID (None = global)
    pub profile_id: Option<i64>,
    /// Timestamp of plan creation
    pub timestamp: String,
    /// Hash of the ModuleConfig.xml for invalidation
    pub config_hash: String,

    /// Selected options from wizard
    pub selected_options: Vec<OptionSelection>,
    /// Flags that were set during selection
    pub flags_set: HashMap<String, String>,

    /// File operations to perform
    pub file_operations: Vec<FileOperation>,
    /// Estimated number of files to install
    pub estimated_file_count: usize,
    /// Estimated total size in bytes
    pub estimated_size_bytes: u64,

    /// Detected conflicts with existing mods
    pub conflicts: Vec<ConflictItem>,
}

/// A selected option from the wizard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionSelection {
    /// Step index
    pub step_idx: usize,
    /// Step name
    pub step_name: String,
    /// Group index within step
    pub group_idx: usize,
    /// Group name
    pub group_name: String,
    /// Selected plugin indices
    pub plugin_indices: Vec<usize>,
    /// Selected plugin names
    pub plugin_names: Vec<String>,
}

/// A file operation to perform during installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    /// Type of operation
    pub op_type: FileOpType,
    /// Source path (relative to mod staging directory)
    pub source: PathBuf,
    /// Destination path (relative to game data directory)
    pub destination: PathBuf,
    /// Priority for conflict resolution
    pub priority: i32,
}

/// Type of file operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOpType {
    /// Copy a single file
    CopyFile,
    /// Copy entire directory recursively
    CopyDir,
    /// Skip this file (overridden by higher priority)
    Skip,
}

/// A conflict with an existing file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictItem {
    /// Path of the conflicting file
    pub path: PathBuf,
    /// Mod that currently owns this file
    pub existing_mod: Option<String>,
    /// Severity of conflict
    pub severity: ConflictSeverity,
    /// Description of the conflict
    pub description: String,
}

/// Severity of a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictSeverity {
    /// Low priority - informational only
    Low,
    /// Medium priority - should review
    Medium,
    /// High priority - requires attention
    High,
}

impl InstallPlan {
    /// Create an install plan from wizard state
    pub fn from_wizard_state(
        wizard: &WizardState,
        installer: &FomodInstaller,
        mod_name: String,
        staging_path: &Path,
        target_path: &Path,
    ) -> anyhow::Result<Self> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let config_hash = compute_config_hash(&installer.config);

        // Collect selected options
        let selected_options = collect_selections(wizard, installer);

        // Get flags from evaluator
        let flags_set = collect_flags(wizard);

        // Compile file operations
        let file_operations =
            compile_file_operations(wizard, installer, staging_path, target_path)?;

        // Estimate file count and size
        let (estimated_file_count, estimated_size_bytes) =
            estimate_install_size(&file_operations, staging_path)?;

        // Detect conflicts (placeholder for now, will be implemented with full conflict system)
        let conflicts = Vec::new();

        Ok(Self {
            mod_name,
            profile_id: None,
            timestamp,
            config_hash,
            selected_options,
            flags_set,
            file_operations,
            estimated_file_count,
            estimated_size_bytes,
            conflicts,
        })
    }

    /// Generate a preview text of the installation plan
    pub fn preview_text(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "=== Installation Plan for {} ===\n\n",
            self.mod_name
        ));

        // Selected options
        output.push_str("Selected Options:\n");
        for opt in &self.selected_options {
            output.push_str(&format!("  {}: {}\n", opt.step_name, opt.group_name));
            for name in &opt.plugin_names {
                output.push_str(&format!("    • {}\n", name));
            }
        }
        output.push('\n');

        // File stats
        output.push_str(&format!(
            "Files to Install: {} ({} MB)\n\n",
            self.estimated_file_count,
            self.estimated_size_bytes / 1_048_576
        ));

        // Conflicts
        if !self.conflicts.is_empty() {
            output.push_str("Conflicts:\n");
            for conflict in &self.conflicts {
                output.push_str(&format!(
                    "  {:?}: {} - {}\n",
                    conflict.severity,
                    conflict.path.display(),
                    conflict.description
                ));
            }
            output.push('\n');
        }

        output
    }

    /// Check if plan is valid (no high-severity conflicts)
    pub fn is_valid(&self) -> bool {
        !self
            .conflicts
            .iter()
            .any(|c| c.severity == ConflictSeverity::High)
    }
}

/// Collect all selections from wizard state
fn collect_selections(wizard: &WizardState, installer: &FomodInstaller) -> Vec<OptionSelection> {
    let mut selections = Vec::new();

    for (step_idx, step) in installer.steps().iter().enumerate() {
        for (group_idx, group) in step.groups.groups.iter().enumerate() {
            let selected = wizard.get_selections(step_idx, group_idx);
            if selected.is_empty() {
                continue;
            }

            let plugin_names: Vec<String> = selected
                .iter()
                .filter_map(|&idx| group.plugins.plugins.get(idx))
                .map(|p| p.name.clone())
                .collect();

            selections.push(OptionSelection {
                step_idx,
                step_name: step.name.clone(),
                group_idx,
                group_name: group.name.clone(),
                plugin_indices: selected.into_iter().collect(),
                plugin_names,
            });
        }
    }

    selections
}

/// Collect all flags from wizard evaluator
fn collect_flags(_wizard: &WizardState) -> HashMap<String, String> {
    // For now, we'll need to track flags as they're set
    // The evaluator doesn't expose its internal flags map
    // This is a limitation we'll need to address
    HashMap::new()
}

/// Compile file operations from wizard selections
fn compile_file_operations(
    wizard: &WizardState,
    installer: &FomodInstaller,
    staging_path: &Path,
    target_path: &Path,
) -> anyhow::Result<Vec<FileOperation>> {
    let mut operations = Vec::new();

    // Get file instructions from wizard
    let instructions = wizard.get_files_to_install(&installer.config);

    for instruction in instructions {
        match instruction {
            FileInstruction::File {
                source,
                destination,
                priority,
            } => {
                let _src = staging_path.join(&source);
                let dst = if destination.is_empty() {
                    target_path.join(Path::new(&source).file_name().unwrap_or_default())
                } else {
                    target_path.join(&destination)
                };

                operations.push(FileOperation {
                    op_type: FileOpType::CopyFile,
                    source: PathBuf::from(source),
                    destination: dst.strip_prefix(target_path).unwrap_or(&dst).to_path_buf(),
                    priority,
                });
            }
            FileInstruction::Folder {
                source,
                destination,
                priority,
            } => {
                let dst = if destination.is_empty() {
                    target_path.to_path_buf()
                } else {
                    target_path.join(&destination)
                };

                operations.push(FileOperation {
                    op_type: FileOpType::CopyDir,
                    source: PathBuf::from(source),
                    destination: dst.strip_prefix(target_path).unwrap_or(&dst).to_path_buf(),
                    priority,
                });
            }
        }
    }

    Ok(operations)
}

/// Estimate installation size
fn estimate_install_size(
    operations: &[FileOperation],
    staging_path: &Path,
) -> anyhow::Result<(usize, u64)> {
    let mut total_files = 0;
    let mut total_bytes = 0u64;

    for op in operations {
        let source_path = staging_path.join(&op.source);

        match op.op_type {
            FileOpType::CopyFile => {
                if source_path.exists() && source_path.is_file() {
                    total_files += 1;
                    total_bytes += source_path.metadata()?.len();
                }
            }
            FileOpType::CopyDir => {
                if source_path.exists() && source_path.is_dir() {
                    let (files, bytes) = count_dir_size(&source_path)?;
                    total_files += files;
                    total_bytes += bytes;
                }
            }
            FileOpType::Skip => {
                // Skip operations don't count
            }
        }
    }

    Ok((total_files, total_bytes))
}

/// Recursively count files and size in a directory
fn count_dir_size(dir: &Path) -> anyhow::Result<(usize, u64)> {
    let mut file_count = 0;
    let mut total_size = 0u64;

    if !dir.is_dir() {
        return Ok((0, 0));
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            file_count += 1;
            total_size += entry.metadata()?.len();
        } else if path.is_dir() {
            let (sub_files, sub_size) = count_dir_size(&path)?;
            file_count += sub_files;
            total_size += sub_size;
        }
    }

    Ok((file_count, total_size))
}

/// Compute hash of module config for invalidation
fn compute_config_hash(config: &ModuleConfig) -> String {
    // Simple hash based on module name and step count
    // In production, would use proper hash of XML content
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    config.module_name.hash(&mut hasher);
    config.install_steps.steps.len().hash(&mut hasher);

    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_severity_ordering() {
        assert!((ConflictSeverity::Low as i32) < (ConflictSeverity::Medium as i32));
        assert!((ConflictSeverity::Medium as i32) < (ConflictSeverity::High as i32));
    }

    #[test]
    fn test_file_op_type() {
        let op = FileOperation {
            op_type: FileOpType::CopyFile,
            source: PathBuf::from("test.esp"),
            destination: PathBuf::from("test.esp"),
            priority: 0,
        };

        assert_eq!(op.op_type, FileOpType::CopyFile);
    }

    #[test]
    fn test_install_plan_validity() {
        let plan = InstallPlan {
            mod_name: "Test".to_string(),
            profile_id: None,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            config_hash: "abc123".to_string(),
            selected_options: vec![],
            flags_set: HashMap::new(),
            file_operations: vec![],
            estimated_file_count: 0,
            estimated_size_bytes: 0,
            conflicts: vec![],
        };

        assert!(plan.is_valid());

        let plan_with_high_conflict = InstallPlan {
            conflicts: vec![ConflictItem {
                path: PathBuf::from("test.esp"),
                existing_mod: Some("Other Mod".to_string()),
                severity: ConflictSeverity::High,
                description: "Critical conflict".to_string(),
            }],
            ..plan
        };

        assert!(!plan_with_high_conflict.is_valid());
    }
}
