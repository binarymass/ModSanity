//! Helper utilities for FOMOD operations

use super::{FomodInstaller, ModuleConfig, WizardState};
use std::path::Path;

/// FOMOD helper utilities
pub struct FomodHelpers;

impl FomodHelpers {
    /// Check if a mod archive likely contains a FOMOD installer
    pub fn is_likely_fomod(archive_path: &Path) -> bool {
        // Check file name patterns that suggest FOMOD content
        if let Some(name) = archive_path.file_name() {
            let name_lower = name.to_string_lossy().to_lowercase();

            // Common FOMOD indicators in filename
            if name_lower.contains("fomod")
                || name_lower.contains("installer")
                || name_lower.contains("options") {
                return true;
            }
        }

        false
    }

    /// Estimate complexity of a FOMOD installer
    pub fn estimate_complexity(installer: &FomodInstaller) -> ComplexityLevel {
        let step_count = installer.steps().len();
        let mut total_options = 0;
        let mut has_conditions = false;

        for step in installer.steps() {
            for group in &step.groups.groups {
                total_options += group.plugins.plugins.len();

                // Check for conditional visibility
                for plugin in &group.plugins.plugins {
                    if plugin.type_descriptor.is_some() {
                        has_conditions = true;
                    }
                }
            }
        }

        if step_count == 0 {
            ComplexityLevel::Simple
        } else if step_count == 1 && total_options <= 5 && !has_conditions {
            ComplexityLevel::Basic
        } else if step_count <= 3 && total_options <= 20 {
            ComplexityLevel::Moderate
        } else if has_conditions || step_count > 5 || total_options > 50 {
            ComplexityLevel::Complex
        } else {
            ComplexityLevel::Advanced
        }
    }

    /// Get a summary description of the installer
    pub fn get_installer_summary(installer: &FomodInstaller) -> String {
        let step_count = installer.steps().len();
        let mut total_options = 0;
        let mut total_groups = 0;

        for step in installer.steps() {
            total_groups += step.groups.groups.len();
            for group in &step.groups.groups {
                total_options += group.plugins.plugins.len();
            }
        }

        format!(
            "{} step(s), {} group(s), {} option(s)",
            step_count, total_groups, total_options
        )
    }

    /// Check if wizard state is complete and valid
    pub fn is_wizard_complete(wizard: &WizardState, config: &ModuleConfig) -> bool {
        // Check all required groups have valid selections
        for (step_idx, step) in config.install_steps.steps.iter().enumerate() {
            for (group_idx, group) in step.groups.groups.iter().enumerate() {
                let selections = wizard.get_selections(step_idx, group_idx);

                match group.group_type.as_str() {
                    "SelectExactlyOne" => {
                        if selections.len() != 1 {
                            return false;
                        }
                    }
                    "SelectAtLeastOne" => {
                        if selections.is_empty() {
                            return false;
                        }
                    }
                    "SelectAll" => {
                        if selections.len() != group.plugins.plugins.len() {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
        }

        true
    }

    /// Get total number of files that will be installed
    pub fn count_install_files(wizard: &WizardState, config: &ModuleConfig) -> usize {
        let instructions = wizard.get_files_to_install(config);
        instructions.len()
    }

    /// Format file size in human-readable format
    pub fn format_file_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} bytes", bytes)
        }
    }
}

/// Complexity level of FOMOD installer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComplexityLevel {
    /// No options (simple install)
    Simple,
    /// Single step with few options
    Basic,
    /// Multiple steps or moderate options
    Moderate,
    /// Many steps/options
    Advanced,
    /// Complex conditions and dependencies
    Complex,
}

impl ComplexityLevel {
    pub fn description(&self) -> &'static str {
        match self {
            Self::Simple => "Simple installation (no options)",
            Self::Basic => "Basic options",
            Self::Moderate => "Moderate complexity",
            Self::Advanced => "Advanced options",
            Self::Complex => "Complex with conditions",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Simple => Color::Green,
            Self::Basic => Color::Cyan,
            Self::Moderate => Color::Yellow,
            Self::Advanced => Color::Magenta,
            Self::Complex => Color::Red,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_fomod_detection() {
        assert!(FomodHelpers::is_likely_fomod(&PathBuf::from("test-fomod-installer.7z")));
        assert!(FomodHelpers::is_likely_fomod(&PathBuf::from("ModName-Options.zip")));
        assert!(!FomodHelpers::is_likely_fomod(&PathBuf::from("simple-mod.7z")));
    }

    #[test]
    fn test_file_size_formatting() {
        assert_eq!(FomodHelpers::format_file_size(512), "512 bytes");
        assert_eq!(FomodHelpers::format_file_size(1024), "1.00 KB");
        assert_eq!(FomodHelpers::format_file_size(1024 * 1024), "1.00 MB");
        assert_eq!(FomodHelpers::format_file_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_complexity_ordering() {
        assert!(ComplexityLevel::Simple < ComplexityLevel::Basic);
        assert!(ComplexityLevel::Basic < ComplexityLevel::Moderate);
        assert!(ComplexityLevel::Moderate < ComplexityLevel::Advanced);
        assert!(ComplexityLevel::Advanced < ComplexityLevel::Complex);
    }

    #[test]
    fn test_complexity_descriptions() {
        assert!(!ComplexityLevel::Simple.description().is_empty());
        assert!(!ComplexityLevel::Complex.description().is_empty());
    }
}
