//! Plugin filtering system
//!
//! Filters out base game, DLC, and Creation Club content

use std::collections::HashSet;

/// Filter for skipping base game, DLC, and CC plugins
pub struct PluginFilter {
    skip_patterns: HashSet<String>,
    skipped: std::sync::atomic::AtomicUsize,
}

impl PluginFilter {
    /// Create a filter for the specified game
    pub fn for_game(game_id: &str) -> Self {
        let skip_patterns = match game_id {
            "skyrimse" | "skyrimvr" | "skyrimspecialedition" | "skyrim" => skyrim_skip_patterns(),
            "fallout4" | "fallout4vr" => fallout4_skip_patterns(),
            "fallout3" | "falloutnv" => fallout3_skip_patterns(),
            "oblivion" => oblivion_skip_patterns(),
            "morrowind" => morrowind_skip_patterns(),
            _ => HashSet::new(),
        };

        Self {
            skip_patterns,
            skipped: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Check if a plugin should be skipped
    pub fn should_skip(&self, plugin_name: &str) -> bool {
        let lower = plugin_name.to_lowercase();

        // Check exact matches
        if self.skip_patterns.contains(&lower) {
            self.skipped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return true;
        }

        // Check Creation Club prefix (all games)
        if lower.starts_with("cc") && (lower.ends_with(".esm") || lower.ends_with(".esl")) {
            self.skipped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return true;
        }

        false
    }

    /// Get count of skipped plugins
    pub fn skipped_count(&self) -> usize {
        self.skipped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Skyrim base game and DLC plugins to skip
fn skyrim_skip_patterns() -> HashSet<String> {
    let patterns = vec![
        // Base game
        "skyrim.esm",
        "update.esm",
        // DLCs
        "dawnguard.esm",
        "hearthfires.esm",
        "dragonborn.esm",
        // Special Edition additions
        "skyrim.exe",
        "tesv.exe",
    ];

    patterns.into_iter().map(|s| s.to_string()).collect()
}

/// Fallout 4 base game and DLC plugins
fn fallout4_skip_patterns() -> HashSet<String> {
    let patterns = vec![
        // Base game
        "fallout4.esm",
        "fallout4.exe",
        // DLCs
        "dlcrobot.esm",
        "dlcworkshop01.esm",
        "dlccoast.esm",
        "dlcworkshop02.esm",
        "dlcworkshop03.esm",
        "dlcnukaworld.esm",
        "dlcultrahighresolution.esm",
    ];

    patterns.into_iter().map(|s| s.to_string()).collect()
}

/// Fallout 3/NV base game and DLC plugins
fn fallout3_skip_patterns() -> HashSet<String> {
    let patterns = vec![
        // Fallout 3
        "fallout3.esm",
        "anchorage.esm",
        "thepitt.esm",
        "brokensteel.esm",
        "pointlookout.esm",
        "zeta.esm",
        // Fallout NV
        "falloutnv.esm",
        "deadmoney.esm",
        "honesthearts.esm",
        "oldworldblues.esm",
        "lonesomeroad.esm",
        "gunrunnersarsenal.esm",
        "caravanpack.esm",
        "classicpack.esm",
        "mercenarypack.esm",
        "tribalpack.esm",
    ];

    patterns.into_iter().map(|s| s.to_string()).collect()
}

/// Oblivion base game and DLC plugins
fn oblivion_skip_patterns() -> HashSet<String> {
    let patterns = vec![
        "oblivion.esm",
        "knights.esp",
        "dlcshiveringisles.esp",
        "dlcfrostcrag.esp",
        "dlcvilelair.esp",
        "dlcmehrunesrazor.esp",
        "dlcspelltomes.esp",
        "dlcthievesd.esp",
        "dlcorrery.esp",
        "dlchorsearmorpack.esp",
        "dlcbattlehorncastle.esp",
    ];

    patterns.into_iter().map(|s| s.to_string()).collect()
}

/// Morrowind base game and DLC plugins
fn morrowind_skip_patterns() -> HashSet<String> {
    let patterns = vec![
        "morrowind.esm",
        "tribunal.esm",
        "bloodmoon.esm",
    ];

    patterns.into_iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skyrim_base_game() {
        let filter = PluginFilter::for_game("skyrimspecialedition");
        assert!(filter.should_skip("Skyrim.esm"));
        assert!(filter.should_skip("Update.esm"));
        assert!(filter.should_skip("Dawnguard.esm"));
    }

    #[test]
    fn test_creation_club() {
        let filter = PluginFilter::for_game("skyrimspecialedition");
        assert!(filter.should_skip("ccBGSSSE001-Fish.esm"));
        assert!(filter.should_skip("ccQDRSSE001-SurvivalMode.esl"));
    }

    #[test]
    fn test_mod_plugins() {
        let filter = PluginFilter::for_game("skyrimspecialedition");
        assert!(!filter.should_skip("SkyUI_SE.esp"));
        assert!(!filter.should_skip("USSEP.esp"));
    }
}
