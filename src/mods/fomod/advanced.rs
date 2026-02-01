//! Advanced FOMOD features
//!
//! Enhanced condition evaluation, validation, and utilities.

use super::{ModuleConfig, OptionGroup, Plugin};
use std::path::Path;

/// Advanced condition operators for version comparison
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    /// Equal to (==)
    Equal,
    /// Not equal to (!=)
    NotEqual,
    /// Greater than (>)
    GreaterThan,
    /// Greater than or equal (>=)
    GreaterOrEqual,
    /// Less than (<)
    LessThan,
    /// Less than or equal (<=)
    LessOrEqual,
}

impl ComparisonOperator {
    /// Parse operator from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "==" | "=" => Some(Self::Equal),
            "!=" => Some(Self::NotEqual),
            ">" => Some(Self::GreaterThan),
            ">=" => Some(Self::GreaterOrEqual),
            "<" => Some(Self::LessThan),
            "<=" => Some(Self::LessOrEqual),
            _ => None,
        }
    }

    /// Apply comparison operator
    pub fn compare<T: PartialOrd>(&self, left: T, right: T) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::GreaterThan => left > right,
            Self::GreaterOrEqual => left >= right,
            Self::LessThan => left < right,
            Self::LessOrEqual => left <= right,
        }
    }
}

/// Version comparison for FOMOD conditions
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Version {
    /// Parse version string (e.g., "1.2.3")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return None;
        }

        let major = parts.get(0)?.parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Compare versions using operator
    pub fn compare_with(&self, other: &Self, op: ComparisonOperator) -> bool {
        op.compare(self, other)
    }
}

/// Nested data root detection
pub struct DataRootDetector;

impl DataRootDetector {
    /// Common data directory indicators
    const DATA_INDICATORS: &'static [&'static str] = &[
        "meshes",
        "textures",
        "scripts",
        "interface",
        "sound",
        "music",
        "video",
        "strings",
        "seq",
        "shadersfx",
        "skse",
        "calientetools",
        "shapedata",
        "tools",
    ];

    /// Detect if path contains nested Data directory
    pub fn find_data_root(path: &Path) -> Option<std::path::PathBuf> {
        // Check if current directory has data indicators
        if Self::has_data_indicators(path) {
            return Some(path.to_path_buf());
        }

        // Check for "Data" subdirectory
        let data_dir = path.join("Data");
        if data_dir.exists() && Self::has_data_indicators(&data_dir) {
            return Some(data_dir);
        }

        // Check one level deep for common patterns
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.file_type().ok()?.is_dir() {
                    let subdir = entry.path();
                    if Self::has_data_indicators(&subdir) {
                        return Some(subdir);
                    }
                }
            }
        }

        None
    }

    /// Check if directory has data indicators
    fn has_data_indicators(path: &Path) -> bool {
        if !path.exists() || !path.is_dir() {
            return false;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_lowercase();

                // Check against indicators
                if Self::DATA_INDICATORS.contains(&name.as_str()) {
                    return true;
                }

                // Check for plugin files
                if name.ends_with(".esp") || name.ends_with(".esm") || name.ends_with(".esl") {
                    return true;
                }
            }
        }

        false
    }

    /// Strip "Data/" prefix from file paths
    pub fn normalize_path(path: &str) -> String {
        if path.starts_with("Data/") || path.starts_with("Data\\") {
            path[5..].to_string()
        } else {
            path.to_string()
        }
    }
}

/// FOMOD installer validation
pub struct InstallerValidator;

impl InstallerValidator {
    /// Validate entire FOMOD configuration
    pub fn validate(config: &ModuleConfig) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Check module name
        if config.module_name.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Metadata,
                message: "Module name is empty".to_string(),
                location: None,
            });
        }

        // Validate steps
        if config.install_steps.steps.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Info,
                category: IssueCategory::Structure,
                message: "No installation steps defined (simple install)".to_string(),
                location: None,
            });
        }

        for (step_idx, step) in config.install_steps.steps.iter().enumerate() {
            // Check step name
            if step.name.is_empty() {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    category: IssueCategory::Metadata,
                    message: format!("Step {} has no name", step_idx),
                    location: Some(format!("step[{}]", step_idx)),
                });
            }

            // Validate groups
            for (group_idx, group) in step.groups.groups.iter().enumerate() {
                issues.extend(Self::validate_group(group, step_idx, group_idx));
            }
        }

        issues
    }

    /// Validate a single option group
    fn validate_group(group: &OptionGroup, step_idx: usize, group_idx: usize) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let location = format!("step[{}].group[{}]", step_idx, group_idx);

        // Check group name
        if group.name.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Metadata,
                message: "Group has no name".to_string(),
                location: Some(location.clone()),
            });
        }

        // Check group type
        let valid_types = ["SelectExactlyOne", "SelectAtMostOne", "SelectAtLeastOne", "SelectAll", "SelectAny"];
        if !valid_types.contains(&group.group_type.as_str()) {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                category: IssueCategory::Structure,
                message: format!("Invalid group type: '{}'", group.group_type),
                location: Some(location.clone()),
            });
        }

        // Check for empty groups
        if group.plugins.plugins.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                category: IssueCategory::Structure,
                message: "Group has no options".to_string(),
                location: Some(location.clone()),
            });
        }

        // Validate plugins
        for (plugin_idx, plugin) in group.plugins.plugins.iter().enumerate() {
            issues.extend(Self::validate_plugin(plugin, step_idx, group_idx, plugin_idx));
        }

        issues
    }

    /// Validate a single plugin
    fn validate_plugin(
        plugin: &Plugin,
        step_idx: usize,
        group_idx: usize,
        plugin_idx: usize,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let location = format!("step[{}].group[{}].plugin[{}]", step_idx, group_idx, plugin_idx);

        // Check plugin name
        if plugin.name.is_empty() {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Metadata,
                message: "Plugin has no name".to_string(),
                location: Some(location.clone()),
            });
        }

        // Check if plugin has files
        if plugin.files.is_none()
            || (plugin.files.as_ref().unwrap().files.is_empty()
                && plugin.files.as_ref().unwrap().folders.is_empty())
        {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                category: IssueCategory::Files,
                message: "Plugin has no files or folders to install".to_string(),
                location: Some(location),
            });
        }

        issues
    }
}

/// Validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub message: String,
    pub location: Option<String>,
}

/// Issue severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    /// Informational message
    Info,
    /// Warning that should be reviewed
    Warning,
    /// Error that may cause problems
    Error,
}

/// Issue category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    /// Metadata issues (names, descriptions)
    Metadata,
    /// Structure issues (missing groups, invalid types)
    Structure,
    /// File reference issues
    Files,
    /// Dependency issues
    Dependencies,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparison_operators() {
        assert_eq!(ComparisonOperator::from_str("=="), Some(ComparisonOperator::Equal));
        assert_eq!(ComparisonOperator::from_str(">="), Some(ComparisonOperator::GreaterOrEqual));
        assert_eq!(ComparisonOperator::from_str("invalid"), None);

        assert!(ComparisonOperator::Equal.compare(5, 5));
        assert!(!ComparisonOperator::Equal.compare(5, 6));
        assert!(ComparisonOperator::GreaterThan.compare(6, 5));
        assert!(ComparisonOperator::LessOrEqual.compare(5, 5));
    }

    #[test]
    fn test_version_parsing() {
        assert_eq!(Version::parse("1.2.3"), Some(Version { major: 1, minor: 2, patch: 3 }));
        assert_eq!(Version::parse("1.2"), Some(Version { major: 1, minor: 2, patch: 0 }));
        assert_eq!(Version::parse("1"), Some(Version { major: 1, minor: 0, patch: 0 }));
        assert_eq!(Version::parse("invalid"), None);
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("1.2.3").unwrap();
        let v2 = Version::parse("1.2.4").unwrap();
        let v3 = Version::parse("2.0.0").unwrap();

        assert!(v1.compare_with(&v2, ComparisonOperator::LessThan));
        assert!(v2.compare_with(&v1, ComparisonOperator::GreaterThan));
        assert!(v1.compare_with(&v1, ComparisonOperator::Equal));
        assert!(v3.compare_with(&v1, ComparisonOperator::GreaterThan));
    }

    #[test]
    fn test_data_root_normalization() {
        assert_eq!(DataRootDetector::normalize_path("Data/meshes/test.nif"), "meshes/test.nif");
        assert_eq!(DataRootDetector::normalize_path("Data\\meshes\\test.nif"), "meshes\\test.nif");
        assert_eq!(DataRootDetector::normalize_path("meshes/test.nif"), "meshes/test.nif");
    }

    #[test]
    fn test_issue_severity_ordering() {
        assert!(IssueSeverity::Info < IssueSeverity::Warning);
        assert!(IssueSeverity::Warning < IssueSeverity::Error);
    }
}
