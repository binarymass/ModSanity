//! Native Rust implementation of plugin load order optimization
//! Based on LOOT principles but simplified for direct integration

use super::PluginInfo;
use super::masterlist::{load_masterlist, build_metadata_map, get_load_after_rules, get_group};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Sort plugins using dependency-based topological sort
/// This ensures:
/// 1. Base game official masters load first (game-aware)
/// 2. Skyrim AE content loads after base game (Skyrim-specific)
/// 3. Mod masters load after official content
/// 4. Plugins load after their masters (dependencies)
/// 5. Light plugins (.esl) are handled correctly
/// 6. LOOT masterlist rules are applied (load_after rules and groups)
/// 7. Plugins without dependencies are ordered alphabetically for consistency
pub fn optimize_load_order(plugins: &mut [PluginInfo], game_id: &str) -> Result<()> {
    // Try to load the masterlist (optional)
    let metadata_map = load_masterlist_if_exists();

    // Build dependency graph (includes masterlist rules if available)
    let graph = build_dependency_graph(plugins, metadata_map.as_ref());

    // Perform topological sort
    let sorted_indices = topological_sort(&graph, plugins, metadata_map.as_ref(), game_id)?;

    // Reorder the plugins slice based on sorted indices
    let mut sorted_plugins: Vec<PluginInfo> = sorted_indices
        .into_iter()
        .map(|idx| plugins[idx].clone())
        .collect();

    // Update load order indices and move back to original slice
    for (i, plugin) in sorted_plugins.iter_mut().enumerate() {
        plugin.load_order = i;
    }

    // Move sorted plugins back to original slice
    for (i, plugin) in sorted_plugins.into_iter().enumerate() {
        plugins[i] = plugin;
    }

    Ok(())
}

/// Try to load the masterlist from common locations
fn load_masterlist_if_exists() -> Option<HashMap<String, super::masterlist::PluginMetadata>> {
    // Try common locations for the masterlist
    let possible_paths = [
        "masterlist.yaml",
        "loot-master/masterlist.yaml",
        "./masterlist.yaml",
    ];

    for path in &possible_paths {
        if let Ok(masterlist) = load_masterlist(Path::new(path)) {
            tracing::info!("Loaded LOOT masterlist from {}", path);
            return Some(build_metadata_map(&masterlist));
        }
    }

    tracing::debug!("No masterlist found, using basic dependency-only sorting");
    None
}

/// Build a dependency graph where each plugin points to its dependencies
/// Includes both master dependencies and LOOT masterlist load_after rules
fn build_dependency_graph(
    plugins: &[PluginInfo],
    metadata_map: Option<&HashMap<String, super::masterlist::PluginMetadata>>,
) -> HashMap<usize, Vec<usize>> {
    let mut graph: HashMap<usize, Vec<usize>> = HashMap::new();

    // Create a name-to-index mapping for quick lookups
    let name_to_index: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .map(|(i, p)| (p.filename.to_lowercase(), i))
        .collect();

    // Build dependency edges
    for (i, plugin) in plugins.iter().enumerate() {
        let mut dependencies = Vec::new();

        // Add master dependencies
        for master in &plugin.masters {
            if let Some(&master_idx) = name_to_index.get(&master.to_lowercase()) {
                dependencies.push(master_idx);
            }
        }

        // Add LOOT masterlist load_after rules if available
        if let Some(map) = metadata_map {
            let load_after = get_load_after_rules(&plugin.filename, map);
            for after_plugin in load_after {
                if let Some(&after_idx) = name_to_index.get(&after_plugin) {
                    // Only add if not already a dependency
                    if !dependencies.contains(&after_idx) {
                        dependencies.push(after_idx);
                    }
                }
            }
        }

        graph.insert(i, dependencies);
    }

    graph
}

/// Perform topological sort using Kahn's algorithm
/// Returns indices in sorted order
fn topological_sort(
    graph: &HashMap<usize, Vec<usize>>,
    plugins: &[PluginInfo],
    metadata_map: Option<&HashMap<String, super::masterlist::PluginMetadata>>,
    game_id: &str,
) -> Result<Vec<usize>> {
    let n = plugins.len();
    let mut in_degree = vec![0usize; n];
    let mut sorted = Vec::new();

    // Build reverse graph: reverse_graph[dep] = list of nodes that depend on dep
    // graph[i] = nodes that i depends on, so if i depends on dep, dep must come first
    let mut reverse_graph: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&i, deps) in graph.iter() {
        in_degree[i] = deps.len();
        for &dep in deps {
            reverse_graph.entry(dep).or_default().push(i);
        }
    }

    // Create priority groups with LOOT group integration:
    // Priority 0: Official base masters/content (game-aware)
    // Priority 1: Skyrim Anniversary Edition content
    // Priority 2-4: Early loaders group (from LOOT)
    // Priority 5: Mod masters (.esm files from mods) - default group
    // Priority 6: Light plugins (.esl) - default group
    // Priority 7: Regular plugins (.esp) - default group
    // Priority 8-10: Late loaders group (from LOOT)
    let get_priority = |plugin: &PluginInfo| -> u8 {
        use super::PluginType;
        if is_official_master(game_id, &plugin.filename) {
            return 0;
        }

        // Skyrim-specific AE content handling
        if is_skyrim_ae_content(game_id, &plugin.filename) {
            return 1;
        }

        // Check LOOT group if masterlist is available
        if let Some(map) = metadata_map {
            let group = get_group(&plugin.filename, map);
            match group.as_str() {
                "early loaders" => return 2,
                "late loaders" => return 8,
                _ => {} // Continue to default priority based on type
            }
        }

        // Now check plugin type for default group
        match plugin.plugin_type {
            PluginType::Master => 5, // Mod masters
            PluginType::Light => 6,  // Light plugins
            PluginType::Plugin => if plugin.is_light { 6 } else { 7 }, // Regular or ESL-flagged
        }
    };

    // Start with nodes that have no dependencies (in_degree == 0)
    let mut queue: Vec<usize> = (0..n)
        .filter(|&i| in_degree[i] == 0)
        .collect();

    // Sort queue by priority (descending) so lowest priority pops last from the end
    queue.sort_by(|&a, &b| {
        let priority_cmp = get_priority(&plugins[b]).cmp(&get_priority(&plugins[a]));
        if priority_cmp == std::cmp::Ordering::Equal {
            plugins[b].filename.to_lowercase().cmp(&plugins[a].filename.to_lowercase())
        } else {
            priority_cmp
        }
    });

    while let Some(current) = queue.pop() {
        sorted.push(current);

        // Process nodes that depend on current (current is now resolved)
        if let Some(dependents) = reverse_graph.get(&current) {
            for &dependent in dependents {
                in_degree[dependent] -= 1;
                if in_degree[dependent] == 0 {
                    // Insert in sorted position (descending priority so pop gives lowest)
                    let pos = queue
                        .binary_search_by(|&probe| {
                            let priority_cmp = get_priority(&plugins[probe])
                                .cmp(&get_priority(&plugins[dependent]))
                                .reverse();
                            if priority_cmp == std::cmp::Ordering::Equal {
                                plugins[probe]
                                    .filename
                                    .to_lowercase()
                                    .cmp(&plugins[dependent].filename.to_lowercase())
                                    .reverse()
                            } else {
                                priority_cmp
                            }
                        })
                        .unwrap_or_else(|e| e);
                    queue.insert(pos, dependent);
                }
            }
        }
    }

    // Check for cycles
    if sorted.len() != n {
        anyhow::bail!("Circular dependency detected in plugin load order");
    }

    Ok(sorted)
}

fn is_official_master(game_id: &str, filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    match game_id {
        "skyrimse" | "skyrimvr" => {
            use crate::games::skyrimse::SkyrimSE;
            SkyrimSE::is_base_master(&lower)
        }
        "fallout4" | "fallout4vr" => matches!(
            lower.as_str(),
            "fallout4.esm"
                | "dlcrobot.esm"
                | "dlcworkshop01.esm"
                | "dlccoast.esm"
                | "dlcworkshop02.esm"
                | "dlcworkshop03.esm"
                | "dlcnukaworld.esm"
                | "dlcultrahighresolution.esm"
        ),
        "starfield" => matches!(
            lower.as_str(),
            "starfield.esm"
                | "constellation.esm"
                | "oldmars.esm"
                | "blueprintships-starfield.esm"
                | "sfbgs003.esm"
                | "sfbgs004.esm"
                | "sfbgs006.esm"
                | "sfbgs007.esm"
                | "sfbgs008.esm"
                | "sfbgs021.esm"
                | "sfbgs023.esm"
                | "sfbgs024.esm"
                | "sfbgs026.esm"
                | "sfbgs028.esm"
                | "sfbgs029.esm"
        ),
        _ => false,
    }
}

fn is_skyrim_ae_content(game_id: &str, filename: &str) -> bool {
    if !matches!(game_id, "skyrimse" | "skyrimvr") {
        return false;
    }
    use crate::games::skyrimse::SkyrimSE;
    SkyrimSE::is_ae_content(filename)
}

/// Validate that the current load order satisfies all dependencies
pub fn validate_load_order(plugins: &[PluginInfo], _game_id: &str) -> Vec<String> {
    let mut issues = Vec::new();

    // Build index map
    let index_map: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .map(|(i, p)| (p.filename.to_lowercase(), i))
        .collect();

    for (i, plugin) in plugins.iter().enumerate() {
        for master in &plugin.masters {
            let master_lower = master.to_lowercase();

            // Check if master exists
            if let Some(&master_idx) = index_map.get(&master_lower) {
                // Check if master loads before dependent
                if master_idx > i {
                    issues.push(format!(
                        "{} loads at position {} but its master {} loads at position {}",
                        plugin.filename, i, master, master_idx
                    ));
                }
            } else {
                // Master not found
                issues.push(format!(
                    "{} requires missing master: {}",
                    plugin.filename, master
                ));
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::PluginType;
    use std::path::PathBuf;

    fn create_test_plugin(filename: &str, plugin_type: PluginType, masters: Vec<String>) -> PluginInfo {
        PluginInfo {
            filename: filename.to_string(),
            path: PathBuf::from(filename),
            plugin_type,
            enabled: true,
            load_order: 0,
            masters,
            is_light: plugin_type == PluginType::Light,
            description: None,
            author: None,
        }
    }

    #[test]
    fn test_simple_dependency_sort() {
        let mut plugins = vec![
            create_test_plugin("Plugin.esp", PluginType::Plugin, vec!["Skyrim.esm".to_string()]),
            create_test_plugin("Skyrim.esm", PluginType::Master, vec![]),
        ];

        optimize_load_order(&mut plugins, "skyrimse").unwrap();

        assert_eq!(plugins[0].filename, "Skyrim.esm");
        assert_eq!(plugins[1].filename, "Plugin.esp");
    }

    #[test]
    fn test_validation() {
        let plugins = vec![
            create_test_plugin("Plugin.esp", PluginType::Plugin, vec!["Skyrim.esm".to_string()]),
            create_test_plugin("Skyrim.esm", PluginType::Master, vec![]),
        ];

        let issues = validate_load_order(&plugins, "skyrimse");
        assert!(!issues.is_empty()); // Plugin loads before its master
    }
}
