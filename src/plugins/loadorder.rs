//! Load order management (plugins.txt and loadorder.txt)

use crate::games::Game;
use anyhow::{Context, Result};
use std::path::Path;

/// Read plugins.txt and return list of enabled plugins (lowercase)
pub fn read_plugins_txt(game: &Game) -> Result<Vec<String>> {
    let path = match &game.plugins_txt_path {
        Some(p) => p.clone(),
        None => return Ok(Vec::new()),
    };

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path).context("Failed to read plugins.txt")?;

    let plugins: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            // Handle *PluginName.esp format (asterisk means enabled)
            let name = line.trim_start_matches('*');
            // Only return if it was marked as enabled (with asterisk) or no asterisk system
            if line.starts_with('*') || !content.contains('*') {
                Some(name.to_lowercase())
            } else {
                None
            }
        })
        .collect();

    Ok(plugins)
}

/// Read loadorder.txt and return ordered list of plugins
pub fn read_loadorder_txt(game: &Game) -> Result<Vec<String>> {
    let path = match &game.loadorder_txt_path {
        Some(p) => p.clone(),
        None => return Ok(Vec::new()),
    };

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path).context("Failed to read loadorder.txt")?;

    let plugins: Vec<String> = content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            Some(line.to_string())
        })
        .collect();

    Ok(plugins)
}

/// Write plugins.txt with enabled plugins
pub fn write_plugins_txt(game: &Game, enabled_plugins: &[String]) -> Result<()> {
    let path = match &game.plugins_txt_path {
        Some(p) => p.clone(),
        None => anyhow::bail!("plugins.txt path not configured"),
    };

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Build content with asterisk format and Windows line endings
    let content: String = enabled_plugins
        .iter()
        .map(|p| format!("*{}", p))
        .collect::<Vec<_>>()
        .join("\r\n");

    std::fs::write(&path, content)?;

    Ok(())
}

/// Write loadorder.txt
pub fn write_loadorder_txt(game: &Game, plugins: &[String]) -> Result<()> {
    let path = match &game.loadorder_txt_path {
        Some(p) => p.clone(),
        None => anyhow::bail!("loadorder.txt path not configured"),
    };

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = plugins.join("\r\n");
    std::fs::write(&path, content)?;

    Ok(())
}

/// Check for missing masters in enabled plugins
pub fn check_missing_masters(
    plugins: &[super::PluginInfo],
) -> Vec<(String, Vec<String>)> {
    let enabled_names: std::collections::HashSet<_> = plugins
        .iter()
        .filter(|p| p.enabled)
        .map(|p| p.filename.to_lowercase())
        .collect();

    let mut missing = Vec::new();

    for plugin in plugins.iter().filter(|p| p.enabled) {
        let plugin_missing: Vec<String> = plugin
            .masters
            .iter()
            .filter(|m| !enabled_names.contains(&m.to_lowercase()))
            .cloned()
            .collect();

        if !plugin_missing.is_empty() {
            missing.push((plugin.filename.clone(), plugin_missing));
        }
    }

    missing
}

/// Validate load order (masters before dependents)
pub fn validate_load_order(plugins: &[super::PluginInfo]) -> Vec<String> {
    let mut issues = Vec::new();
    let enabled: Vec<_> = plugins.iter().filter(|p| p.enabled).collect();

    // Build index map
    let index_map: std::collections::HashMap<String, usize> = enabled
        .iter()
        .enumerate()
        .map(|(i, p)| (p.filename.to_lowercase(), i))
        .collect();

    for (i, plugin) in enabled.iter().enumerate() {
        for master in &plugin.masters {
            if let Some(&master_idx) = index_map.get(&master.to_lowercase()) {
                if master_idx > i {
                    issues.push(format!(
                        "{} loads before its master {}",
                        plugin.filename, master
                    ));
                }
            }
        }
    }

    issues
}
