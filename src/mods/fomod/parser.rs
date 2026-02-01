//! FOMOD ModuleConfig.xml parser

use anyhow::{Context, Result};
use quick_xml::de::from_str;
use serde::Deserialize;

/// Root element of ModuleConfig.xml
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "config")]
pub struct ModuleConfig {
    #[serde(rename = "moduleName", default)]
    pub module_name: String,

    #[serde(rename = "moduleImage", default)]
    pub module_image: Option<ModuleImage>,

    #[serde(rename = "moduleDependencies", default)]
    pub dependencies: Option<Dependencies>,

    #[serde(rename = "requiredInstallFiles", default)]
    pub required_files: Option<FileList>,

    #[serde(rename = "installSteps", default)]
    pub install_steps: InstallSteps,

    #[serde(rename = "conditionalFileInstalls", default)]
    pub conditional_installs: Option<ConditionalInstalls>,
}

/// Module image for the installer UI
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleImage {
    #[serde(rename = "@path")]
    pub path: String,
}

/// Dependency list
#[derive(Debug, Clone, Deserialize)]
pub struct Dependencies {
    #[serde(rename = "fileDependency", default)]
    pub file_dependencies: Vec<FileDependency>,

    #[serde(rename = "flagDependency", default)]
    pub flag_dependencies: Vec<FlagDependency>,
}

/// File dependency
#[derive(Debug, Clone, Deserialize)]
pub struct FileDependency {
    #[serde(rename = "@file")]
    pub file: String,

    #[serde(rename = "@state")]
    pub state: String, // Active, Inactive, Missing
}

/// Flag dependency
#[derive(Debug, Clone, Deserialize)]
pub struct FlagDependency {
    #[serde(rename = "@flag")]
    pub flag: String,

    #[serde(rename = "@value")]
    pub value: String,
}

/// Install steps wrapper
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstallSteps {
    #[serde(rename = "@order", default)]
    pub order: String,

    #[serde(rename = "installStep", default)]
    pub steps: Vec<InstallStep>,
}

/// Installation step
#[derive(Debug, Clone, Deserialize)]
pub struct InstallStep {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "optionalFileGroups", default)]
    pub groups: OptionGroups,
}

/// Groups of optional files
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OptionGroups {
    #[serde(rename = "@order", default)]
    pub order: String,

    #[serde(rename = "group", default)]
    pub groups: Vec<OptionGroup>,
}

/// A group of related options
#[derive(Debug, Clone, Deserialize)]
pub struct OptionGroup {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "@type")]
    pub group_type: String, // SelectExactlyOne, SelectAtMostOne, SelectAny, SelectAll

    #[serde(rename = "plugins", default)]
    pub plugins: PluginList,
}

/// List of plugin options
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginList {
    #[serde(rename = "@order", default)]
    pub order: String,

    #[serde(rename = "plugin", default)]
    pub plugins: Vec<Plugin>,
}

/// A single plugin option
#[derive(Debug, Clone, Deserialize)]
pub struct Plugin {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "description", default)]
    pub description: String,

    #[serde(rename = "image", default)]
    pub image: Option<PluginImage>,

    #[serde(rename = "files", default)]
    pub files: Option<FileList>,

    #[serde(rename = "conditionFlags", default)]
    pub condition_flags: Option<ConditionFlags>,

    #[serde(rename = "typeDescriptor", default)]
    pub type_descriptor: Option<TypeDescriptor>,
}

/// Plugin image
#[derive(Debug, Clone, Deserialize)]
pub struct PluginImage {
    #[serde(rename = "@path")]
    pub path: String,
}

/// File list
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileList {
    #[serde(rename = "file", default)]
    pub files: Vec<FileItem>,

    #[serde(rename = "folder", default)]
    pub folders: Vec<FolderItem>,
}

/// A file to install
#[derive(Debug, Clone, Deserialize)]
pub struct FileItem {
    #[serde(rename = "@source")]
    pub source: String,

    #[serde(rename = "@destination", default)]
    pub destination: String,

    #[serde(rename = "@priority", default)]
    pub priority: i32,
}

/// A folder to install
#[derive(Debug, Clone, Deserialize)]
pub struct FolderItem {
    #[serde(rename = "@source")]
    pub source: String,

    #[serde(rename = "@destination", default)]
    pub destination: String,

    #[serde(rename = "@priority", default)]
    pub priority: i32,
}

/// Condition flags to set
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConditionFlags {
    #[serde(rename = "flag", default)]
    pub flags: Vec<Flag>,
}

/// A condition flag
#[derive(Debug, Clone, Deserialize)]
pub struct Flag {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "$text", default)]
    pub value: String,
}

/// Type descriptor for option state
#[derive(Debug, Clone, Deserialize)]
pub struct TypeDescriptor {
    #[serde(rename = "type", default)]
    pub default_type: Option<DefaultType>,

    #[serde(rename = "dependencyType", default)]
    pub dependency_type: Option<DependencyType>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefaultType {
    #[serde(rename = "@name")]
    pub name: String, // Required, Recommended, Optional, NotUsable, CouldBeUsable
}

#[derive(Debug, Clone, Deserialize)]
pub struct DependencyType {
    #[serde(rename = "defaultType")]
    pub default_type: DefaultType,

    #[serde(rename = "patterns", default)]
    pub patterns: Option<Patterns>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Patterns {
    #[serde(rename = "pattern", default)]
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pattern {
    #[serde(rename = "dependencies")]
    pub dependencies: Option<Dependencies>,

    #[serde(rename = "type")]
    pub pattern_type: DefaultType,
}

/// Conditional file installations
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConditionalInstalls {
    #[serde(rename = "patterns", default)]
    pub patterns: Option<ConditionalPatterns>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConditionalPatterns {
    #[serde(rename = "pattern", default)]
    pub patterns: Vec<ConditionalPattern>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConditionalPattern {
    #[serde(rename = "dependencies")]
    pub dependencies: Option<Dependencies>,

    #[serde(rename = "files")]
    pub files: Option<FileList>,
}

/// Parse ModuleConfig.xml content
pub fn parse_module_config(xml: &str) -> Result<ModuleConfig> {
    // Pre-process XML to handle common issues
    let xml = xml
        .trim_start_matches('\u{feff}') // Remove BOM
        .trim();

    // Try to parse and provide detailed error on failure
    match from_str(xml) {
        Ok(config) => Ok(config),
        Err(e) => {
            // Log first 500 chars of XML for debugging
            let preview = if xml.len() > 500 {
                &xml[..500]
            } else {
                xml
            };
            tracing::error!("XML parsing failed: {}", e);
            tracing::debug!("XML preview: {}", preview);
            Err(e).context("Failed to parse ModuleConfig.xml")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_config() {
        let xml = r#"
            <config>
                <moduleName>Test Mod</moduleName>
                <installSteps>
                    <installStep name="Choose Options">
                        <optionalFileGroups>
                            <group name="Main Files" type="SelectExactlyOne">
                                <plugins>
                                    <plugin name="Option A">
                                        <description>Description A</description>
                                        <files>
                                            <folder source="Option A" destination="" priority="0"/>
                                        </files>
                                    </plugin>
                                    <plugin name="Option B">
                                        <description>Description B</description>
                                        <files>
                                            <folder source="Option B" destination="" priority="0"/>
                                        </files>
                                    </plugin>
                                </plugins>
                            </group>
                        </optionalFileGroups>
                    </installStep>
                </installSteps>
            </config>
        "#;

        let config = parse_module_config(xml).unwrap();
        assert_eq!(config.module_name, "Test Mod");
        assert_eq!(config.install_steps.steps.len(), 1);
    }
}
