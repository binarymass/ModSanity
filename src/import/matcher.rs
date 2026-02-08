//! NexusMods matching system with scoring

use anyhow::Result;
use crate::db::Database;
use crate::import::modlist_parser::PluginEntry;
use crate::nexus::{NexusClient, ModSearchParams, ModSearchResult, SortBy};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

/// Matcher for finding NexusMods entries for plugins
pub struct ModMatcher {
    game_id: String,
    game_domain: String,
    nexus_client: NexusClient,
    db: Option<Arc<Database>>,
}

impl ModMatcher {
    pub fn new(game_id: String, nexus_client: NexusClient) -> Self {
        Self::with_catalog(game_id, nexus_client, None)
    }

    pub fn with_catalog(game_id: String, nexus_client: NexusClient, db: Option<Arc<Database>>) -> Self {
        // Normalize game identifier for DB queries and Nexus domain.
        let (normalized_game_id, game_domain) = match game_id.as_str() {
            "skyrimse" | "skyrimspecialedition" => ("skyrimse", "skyrimspecialedition"),
            "skyrimvr" => ("skyrimvr", "skyrimspecialedition"),
            "skyrim" => ("skyrim", "skyrim"),
            "fallout4" => ("fallout4", "fallout4"),
            "fallout4vr" => ("fallout4vr", "fallout4"),
            "starfield" => ("starfield", "starfield"),
            "fallout3" => ("fallout3", "fallout3"),
            "falloutnv" => ("falloutnv", "falloutnv"),
            "oblivion" => ("oblivion", "oblivion"),
            "morrowind" => ("morrowind", "morrowind"),
            other => (other, other),
        };

        Self {
            game_id: normalized_game_id.to_string(),
            game_domain: game_domain.to_string(),
            nexus_client,
            db,
        }
    }

    /// Match a plugin to NexusMods entries
    pub async fn match_plugin(&self, plugin: &PluginEntry) -> Result<MatchResult> {
        let plugin_filename = extract_plugin_filename(&plugin.plugin_name);
        let mod_name = plugin.extract_mod_name();
        let mod_id_hint = plugin.extract_nexus_mod_id();

        // Skip very short names (likely to cause search errors)
        if mod_name.len() < 3 {
            tracing::debug!("Skipping search for very short name: {}", mod_name);
            return Ok(MatchResult::no_match(plugin.clone()));
        }

        // Stage 0a: Check installed mods first (fastest, most accurate)
        if let Some(ref db) = self.db {
            // Stage 0a: exact plugin filename index lookup (highest precision for plugin-only lists)
            if let Some(plugin_name) = &plugin_filename {
                let plugin_hits = db.find_mods_by_plugin_filename(&self.game_id, plugin_name)?;
                if let Some(hit) = plugin_hits.first() {
                    tracing::debug!(
                        "Matched plugin '{}' via installed plugin index -> mod '{}' (nexus_id={:?})",
                        plugin_name,
                        hit.mod_name,
                        hit.nexus_mod_id
                    );

                    if let Some(nexus_id) = hit.nexus_mod_id {
                        // Prefer catalog metadata when available for better user-facing details.
                        if let Some(catalog_hit) = db.get_catalog_mod_by_id(&self.game_domain, nexus_id)? {
                            let result = catalog_hit.to_search_result();
                            return Ok(MatchResult {
                                plugin: plugin.clone(),
                                mod_name: hit.mod_name.clone(),
                                best_match: Some(MatchedMod {
                                    mod_id: result.mod_id,
                                    name: result.name,
                                    author: result.author,
                                    summary: result.summary,
                                    downloads: result.downloads,
                                    version: result.version,
                                }),
                                alternatives: Vec::new(),
                                confidence: MatchConfidence::High(1.0),
                            });
                        }

                        return Ok(MatchResult {
                            plugin: plugin.clone(),
                            mod_name: hit.mod_name.clone(),
                            best_match: Some(MatchedMod {
                                mod_id: nexus_id,
                                name: hit.mod_name.clone(),
                                author: String::new(),
                                summary: format!("Matched by installed plugin: {}", hit.plugin_name),
                                downloads: 0,
                                version: hit.mod_version.clone(),
                            }),
                            alternatives: Vec::new(),
                            confidence: MatchConfidence::High(0.95),
                        });
                    }
                }
            }

            // If we can extract a Nexus mod ID from archive-style names, use it first.
            if let Some(mod_id) = mod_id_hint {
                if let Some(catalog_hit) = db.get_catalog_mod_by_id(&self.game_domain, mod_id)? {
                    tracing::debug!(
                        "Matched '{}' by extracted Nexus mod ID {} -> '{}'",
                        plugin.plugin_name,
                        mod_id,
                        catalog_hit.name
                    );
                    let result = catalog_hit.to_search_result();
                    return Ok(MatchResult {
                        plugin: plugin.clone(),
                        mod_name: mod_name.clone(),
                        best_match: Some(MatchedMod {
                            mod_id: result.mod_id,
                            name: result.name,
                            author: result.author,
                            summary: result.summary,
                            downloads: result.downloads,
                            version: result.version,
                        }),
                        alternatives: Vec::new(),
                        confidence: MatchConfidence::High(1.0),
                    });
                }

                // Archive-style entries with a Nexus ID should not fall back to fuzzy
                // text search; that is slow and tends to reduce precision.
                return Ok(MatchResult::no_match(plugin.clone()));
            }

            // Try exact name match
            match db.find_mod_by_name(&self.game_id, &mod_name) {
                Ok(Some(installed_mod)) => {
                    tracing::debug!("Found installed mod for '{}': {}", mod_name, installed_mod.name);
                    // Return as high-confidence match from installed library
                    if let Some(nexus_id) = installed_mod.nexus_mod_id {
                        return Ok(MatchResult {
                            plugin: plugin.clone(),
                            mod_name: mod_name.clone(),
                            best_match: Some(MatchedMod {
                                mod_id: nexus_id,
                                name: installed_mod.name,
                                author: installed_mod.author.unwrap_or_default(),
                                summary: "Already installed".to_string(),
                                downloads: 0,
                                version: installed_mod.version,
                            }),
                            alternatives: Vec::new(),
                            confidence: MatchConfidence::High(1.0), // Perfect match - already installed
                        });
                    }
                }
                Ok(None) => {
                    tracing::debug!("No installed mod found for '{}'", mod_name);
                }
                Err(e) => {
                    tracing::warn!("Error checking installed mods for '{}': {}", mod_name, e);
                }
            }

            // Try fuzzy match on installed mods
            match db.get_mods_for_game(&self.game_id) {
                Ok(all_mods) => {
                    // Score all installed mods and find best match
                    let mut best_score = 0.0f32;
                    let mut best_installed = None;

                    for installed in &all_mods {
                        let score = calculate_installed_match_score(&mod_name, &installed.name);
                        if score > best_score && score >= 0.6 {
                            best_score = score;
                            best_installed = Some(installed);
                        }
                    }

                    if let Some(installed) = best_installed {
                        tracing::debug!("Found fuzzy installed match for '{}': {} (score: {:.2})",
                            mod_name, installed.name, best_score);

                        if let Some(nexus_id) = installed.nexus_mod_id {
                            let confidence = if best_score >= 0.8 {
                                MatchConfidence::High(best_score)
                            } else {
                                MatchConfidence::Medium(best_score)
                            };

                            return Ok(MatchResult {
                                plugin: plugin.clone(),
                                mod_name: mod_name.clone(),
                                best_match: Some(MatchedMod {
                                    mod_id: nexus_id,
                                    name: installed.name.clone(),
                                    author: installed.author.clone().unwrap_or_default(),
                                    summary: "Already installed".to_string(),
                                    downloads: 0,
                                    version: installed.version.clone(),
                                }),
                                alternatives: Vec::new(),
                                confidence,
                            });
                        }
                    }

                    tracing::debug!("No fuzzy installed matches for '{}' (best score: {:.2})", mod_name, best_score);
                }
                Err(e) => {
                    tracing::warn!("Error getting installed mods for '{}': {}", mod_name, e);
                }
            }
        }

        // Stage 0b: Search local catalog (fast, no API call)
        if mod_id_hint.is_none() {
            if let Some(ref db) = self.db {
            match db.search_catalog(&self.game_domain, &mod_name, 10) {
                Ok(catalog_results) if !catalog_results.is_empty() => {
                    let search_results: Vec<ModSearchResult> = catalog_results
                        .iter()
                        .map(|r| r.to_search_result())
                        .collect();
                    let result = self.score_results(
                        plugin.clone(),
                        mod_name.clone(),
                        search_results,
                        mod_id_hint,
                    );
                    if result.confidence.score() >= 0.4 {
                        tracing::debug!("Catalog hit for '{}': score {:.2}", mod_name, result.confidence.score());
                        return Ok(result);
                    }
                    tracing::debug!("Catalog match for '{}' too low ({:.2}), falling back to API", mod_name, result.confidence.score());
                }
                Ok(_) => {
                    tracing::debug!("No catalog results for '{}'", mod_name);
                }
                Err(e) => {
                    tracing::warn!("Catalog search failed for '{}': {}", mod_name, e);
                }
            }
            }
        }

        // When local DB/catalog is available, keep matching local-only.
        // This avoids long API fallback loops and keeps results catalog-based.
        if self.db.is_some() {
            return Ok(MatchResult::no_match(plugin.clone()));
        }

        // Stage 1: Exact match search
        let exact_results = self.search_exact(&mod_name).await?;

        if !exact_results.is_empty() {
            return Ok(self.score_results(plugin.clone(), mod_name, exact_results, mod_id_hint));
        }

        // Stage 2: Fuzzy match (try variations)
        let fuzzy_results = self.search_fuzzy(&mod_name).await?;

        if !fuzzy_results.is_empty() {
            return Ok(self.score_results(plugin.clone(), mod_name, fuzzy_results, mod_id_hint));
        }

        // Stage 3: No matches found
        Ok(MatchResult::no_match(plugin.clone()))
    }

    /// Search with exact mod name
    async fn search_exact(&self, mod_name: &str) -> Result<Vec<ModSearchResult>> {
        let params = ModSearchParams {
            game_domain: Some(self.game_domain.clone()),
            query: Some(mod_name.to_string()),
            sort_by: SortBy::Relevance,
            limit: Some(10),
            ..Default::default()
        };

        let page = self.nexus_client.search_mods(params).await?;
        Ok(page.results)
    }

    /// Search with fuzzy variations
    async fn search_fuzzy(&self, mod_name: &str) -> Result<Vec<ModSearchResult>> {
        let mut all_results = Vec::new();

        // Try without "SE" suffix
        if mod_name.ends_with(" SE") {
            let without_se = mod_name.strip_suffix(" SE").unwrap();
            if without_se.len() >= 3 {
                let params = ModSearchParams {
                    game_domain: Some(self.game_domain.clone()),
                    query: Some(without_se.to_string()),
                    sort_by: SortBy::Relevance,
                    limit: Some(5),
                    ..Default::default()
                };

                if let Ok(page) = self.nexus_client.search_mods(params).await {
                    all_results.extend(page.results);
                }
            }
        }

        // Try wildcard search only if name is long enough
        // NexusMods requires at least 2 characters for wildcard, so we need 3+ to be safe
        if mod_name.len() >= 4 {
            let wildcard_query = format!("*{}*", mod_name);
            let params = ModSearchParams {
                game_domain: Some(self.game_domain.clone()),
                query: Some(wildcard_query),
                sort_by: SortBy::Downloads,
                limit: Some(5),
                ..Default::default()
            };

            if let Ok(page) = self.nexus_client.search_mods(params).await {
                all_results.extend(page.results);
            }
        }

        Ok(all_results)
    }

    /// Score search results and select best match
    fn score_results(
        &self,
        plugin: PluginEntry,
        mod_name: String,
        results: Vec<ModSearchResult>,
        mod_id_hint: Option<i64>,
    ) -> MatchResult {
        let mut scored: Vec<_> = results
            .into_iter()
            .map(|r| {
                let score = calculate_match_score(&mod_name, &r, mod_id_hint);
                (r, score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Check if we have a clear winner
        if let Some((best, best_score)) = scored.first() {
            let confidence = if *best_score > 0.8 && scored.len() == 1 {
                MatchConfidence::High(*best_score)
            } else if *best_score > 0.6 {
                MatchConfidence::Medium(*best_score)
            } else {
                MatchConfidence::Low(*best_score)
            };

            let alternatives = scored.iter()
                .skip(1)
                .map(|(r, s)| MatchAlternative {
                    mod_id: r.mod_id,
                    name: r.name.clone(),
                    summary: r.summary.clone(),
                    author: r.author.clone(),
                    downloads: r.downloads,
                    score: *s,
                })
                .collect();

            MatchResult {
                plugin,
                mod_name,
                best_match: Some(MatchedMod {
                    mod_id: best.mod_id,
                    name: best.name.clone(),
                    author: best.author.clone(),
                    summary: best.summary.clone(),
                    downloads: best.downloads,
                    version: best.version.clone(),
                }),
                alternatives,
                confidence,
            }
        } else {
            MatchResult::no_match(plugin)
        }
    }
}


/// Normalize a mod name for comparison
/// Handles punctuation, spacing, edition suffixes, etc.
fn normalize_name(name: &str) -> String {
    let mut normalized = name.to_lowercase();

    // Normalize punctuation to spaces
    normalized = normalized
        .replace('-', " ")
        .replace('_', " ")
        .replace('\'', "")
        .replace('\"', "")
        .replace(':', "")
        .replace('(', " ")
        .replace(')', " ")
        .replace('[', " ")
        .replace(']', " ");

    // Remove common edition markers for comparison
    normalized = normalized
        .replace(" se ", " ")
        .replace(" ae ", " ")
        .replace(" vr ", " ")
        .replace(" sse ", " ");

    // Clean up multiple spaces
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract significant words for matching (remove articles, common words)
fn extract_significant_words(name: &str) -> HashSet<String> {
    let stop_words = ["the", "a", "an", "and", "or", "for", "of", "in", "on", "at", "to", "by"];

    normalize_name(name)
        .split_whitespace()
        .filter(|w| w.len() > 2 && !stop_words.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Calculate Levenshtein distance between two strings (edit distance)
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[len1][len2]
}

/// Calculate similarity ratio based on Levenshtein distance (0.0 to 1.0)
fn similarity_ratio(s1: &str, s2: &str) -> f32 {
    let distance = levenshtein_distance(s1, s2);
    let max_len = s1.len().max(s2.len());

    if max_len == 0 {
        return 1.0;
    }

    1.0 - (distance as f32 / max_len as f32)
}

/// Calculate match score between query and installed mod name
fn calculate_installed_match_score(query: &str, installed_name: &str) -> f32 {
    let mut score = 0.0;

    let query_lower = query.to_lowercase();
    let name_lower = installed_name.to_lowercase();

    let query_normalized = normalize_name(query);
    let name_normalized = normalize_name(installed_name);

    // Exact match (normalized): +0.7 (highest priority for installed mods)
    if query_normalized == name_normalized {
        score += 0.7;
    } else if query_lower == name_lower {
        score += 0.65; // Exact match without normalization
    }

    // Substring match: +0.3
    if name_normalized.contains(&query_normalized) || query_normalized.contains(&name_normalized) {
        score += 0.3;
    }

    // Significant word overlap (ignore articles and common words): +0.25
    let query_words = extract_significant_words(query);
    let name_words = extract_significant_words(installed_name);

    if !query_words.is_empty() && !name_words.is_empty() {
        let intersection = query_words.intersection(&name_words).count();
        let union = query_words.union(&name_words).count();

        if union > 0 {
            let jaccard = intersection as f32 / union as f32;
            score += jaccard * 0.25;
        }
    }

    // Fuzzy similarity bonus: +0.2 max
    let fuzzy_score = similarity_ratio(&query_normalized, &name_normalized);
    if fuzzy_score > 0.7 {
        score += (fuzzy_score - 0.7) * 0.667; // Scale 0.7-1.0 to 0-0.2
    }

    score.min(1.0)
}

/// Calculate match score between query and result
fn calculate_match_score(query: &str, result: &ModSearchResult, mod_id_hint: Option<i64>) -> f32 {
    let mut score = 0.0;

    // Strongly prefer exact Nexus ID when present in parsed names.
    if let Some(hint) = mod_id_hint {
        if hint == result.mod_id {
            return 1.0;
        }
    }

    let query_lower = query.to_lowercase();
    let name_lower = result.name.to_lowercase();

    let query_normalized = normalize_name(query);
    let name_normalized = normalize_name(&result.name);

    // Exact match (normalized): +0.6
    if query_normalized == name_normalized {
        score += 0.6;
    } else if query_lower == name_lower {
        score += 0.55; // Exact match without normalization
    }

    // Substring match: +0.3
    if name_normalized.contains(&query_normalized) || query_normalized.contains(&name_normalized) {
        score += 0.3;
    }

    // Significant word overlap (ignore articles and common words): +0.25
    let query_words = extract_significant_words(query);
    let name_words = extract_significant_words(&result.name);

    if !query_words.is_empty() && !name_words.is_empty() {
        let intersection = query_words.intersection(&name_words).count();
        let union = query_words.union(&name_words).count();

        if union > 0 {
            let jaccard = intersection as f32 / union as f32;
            score += jaccard * 0.25;
        }
    }

    // Fuzzy similarity bonus: +0.15 max
    let fuzzy_score = similarity_ratio(&query_normalized, &name_normalized);
    if fuzzy_score > 0.7 {
        score += (fuzzy_score - 0.7) * 0.5; // Scale 0.7-1.0 to 0-0.15
    }

    // Summary relevance bonus: +0.05
    let summary_normalized = normalize_name(&result.summary);
    if summary_normalized.contains(&query_normalized) {
        score += 0.05;
    }

    // High downloads (>100k): +0.05
    if result.downloads > 100_000 {
        score += 0.05;
    }

    score.min(1.0)
}

fn extract_plugin_filename(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Path::new(trimmed)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

/// Result of matching a plugin to NexusMods
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub plugin: PluginEntry,
    pub mod_name: String,
    pub best_match: Option<MatchedMod>,
    pub alternatives: Vec<MatchAlternative>,
    pub confidence: MatchConfidence,
}

impl MatchResult {
    pub fn no_match(plugin: PluginEntry) -> Self {
        let mod_name = plugin.extract_mod_name();
        Self {
            plugin,
            mod_name,
            best_match: None,
            alternatives: Vec::new(),
            confidence: MatchConfidence::None,
        }
    }
}

/// A matched mod from NexusMods
#[derive(Debug, Clone)]
pub struct MatchedMod {
    pub mod_id: i64,
    pub name: String,
    pub author: String,
    pub summary: String,
    pub downloads: i64,
    pub version: String,
}

/// An alternative match candidate
#[derive(Debug, Clone)]
pub struct MatchAlternative {
    pub mod_id: i64,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub downloads: i64,
    pub score: f32,
}

/// Match confidence level
#[derive(Debug, Clone, Copy)]
pub enum MatchConfidence {
    High(f32),   // > 0.8 with single result - auto-select
    Medium(f32), // 0.6-0.8 or multiple results - needs review
    Low(f32),    // < 0.6 - needs review
    None,        // No matches found
}

impl MatchConfidence {
    pub fn is_high(&self) -> bool {
        matches!(self, MatchConfidence::High(_))
    }

    pub fn needs_review(&self) -> bool {
        matches!(self, MatchConfidence::Medium(_) | MatchConfidence::Low(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, MatchConfidence::None)
    }

    pub fn score(&self) -> f32 {
        match self {
            MatchConfidence::High(s) | MatchConfidence::Medium(s) | MatchConfidence::Low(s) => *s,
            MatchConfidence::None => 0.0,
        }
    }
}
