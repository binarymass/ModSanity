//! Database record types

use rusqlite::Row;

/// Mod database record
#[derive(Debug, Clone)]
pub struct ModRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub name: String,
    pub version: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub nexus_mod_id: Option<i64>,
    pub nexus_file_id: Option<i64>,
    pub install_path: String,
    pub enabled: bool,
    pub priority: i32,
    pub file_count: i32,
    pub installed_at: String,
    pub updated_at: String,
    pub category_id: Option<i64>,
}

impl ModRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            game_id: row.get(1)?,
            name: row.get(2)?,
            version: row.get(3)?,
            author: row.get(4)?,
            description: row.get(5)?,
            nexus_mod_id: row.get(6)?,
            nexus_file_id: row.get(7)?,
            install_path: row.get(8)?,
            enabled: row.get::<_, i32>(9)? != 0,
            priority: row.get(10)?,
            file_count: row.get(11)?,
            installed_at: row.get(12)?,
            updated_at: row.get(13)?,
            category_id: row.get(14).ok(),
        })
    }
}

/// Mod file database record
#[derive(Debug, Clone)]
pub struct ModFileRecord {
    pub id: Option<i64>,
    pub mod_id: i64,
    pub relative_path: String,
    pub hash: Option<String>,
    pub size: Option<i64>,
}

impl ModFileRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            mod_id: row.get(1)?,
            relative_path: row.get(2)?,
            hash: row.get(3)?,
            size: row.get(4)?,
        })
    }
}

/// Profile database record
#[derive(Debug, Clone)]
pub struct ProfileRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ProfileRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            game_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    }
}

/// Plugin database record
#[derive(Debug, Clone)]
pub struct PluginRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub filename: String,
    pub enabled: bool,
    pub load_order: i32,
    pub mod_id: Option<i64>,
}

impl PluginRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            game_id: row.get(1)?,
            filename: row.get(2)?,
            enabled: row.get::<_, i32>(3)? != 0,
            load_order: row.get(4)?,
            mod_id: row.get(5)?,
        })
    }
}

/// Download record
#[derive(Debug, Clone)]
pub struct DownloadRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub nexus_mod_id: i64,
    pub nexus_file_id: Option<i64>,
    pub name: String,
    pub filename: Option<String>,
    pub url: Option<String>,
    pub size: Option<i64>,
    pub downloaded: i64,
    pub status: String,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

/// Category database record
#[derive(Debug, Clone)]
pub struct CategoryRecord {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub display_order: i32,
    pub color: Option<String>,
    pub parent_id: Option<i64>,
}

impl CategoryRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            description: row.get(2)?,
            display_order: row.get(3)?,
            color: row.get(4)?,
            parent_id: row.get(5).ok(),
        })
    }
}

/// File conflict between mods
#[derive(Debug, Clone)]
pub struct FileConflict {
    pub path: String,
    pub mod1: String,
    pub mod2: String,
    pub priority1: i32,
    pub priority2: i32,
}

impl FileConflict {
    /// Get the winning mod (higher priority wins)
    pub fn winner(&self) -> &str {
        if self.priority1 > self.priority2 {
            &self.mod1
        } else {
            &self.mod2
        }
    }
}

/// Installed file information (for conflict detection)
#[derive(Debug, Clone)]
pub struct InstalledFile {
    pub path: String,
    pub mod_name: String,
    pub mod_id: i64,
}

/// Download queue entry (extended downloads record)
#[derive(Debug, Clone)]
pub struct DownloadQueueEntry {
    pub id: Option<i64>,
    pub game_id: String,
    pub nexus_mod_id: i64,
    pub nexus_file_id: Option<i64>,
    pub name: String,
    pub filename: Option<String>,
    pub status: String,
    pub queue_position: Option<i32>,
    pub plugin_name: Option<String>,
    pub match_confidence: Option<f32>,
    pub import_batch_id: Option<String>,
    pub selected_file_id: Option<i64>,
    pub auto_install: bool,
    pub downloaded: i64,
    pub size: Option<i64>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
}

impl DownloadQueueEntry {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        // Column order: id, game_id, nexus_mod_id, nexus_file_id, name, filename, url, size,
        // downloaded, status, error, started_at, completed_at, created_at,
        // queue_position, plugin_name, match_confidence, import_batch_id, selected_file_id, auto_install
        Ok(Self {
            id: Some(row.get(0)?),
            game_id: row.get(1)?,
            nexus_mod_id: row.get(2)?,
            nexus_file_id: row.get(3)?,
            name: row.get(4)?,
            filename: row.get(5)?,
            // skip url at position 6
            size: row.get(7)?,
            downloaded: row.get(8)?,
            status: row.get(9)?,
            error: row.get(10)?,
            started_at: row.get(11)?,
            completed_at: row.get(12)?,
            created_at: row.get(13)?,
            queue_position: row.get(14).ok(),
            plugin_name: row.get(15).ok(),
            match_confidence: row.get(16).ok(),
            import_batch_id: row.get(17).ok(),
            selected_file_id: row.get(18).ok(),
            auto_install: row.get::<_, Option<i32>>(19).ok().flatten().map(|v| v != 0).unwrap_or(true),
        })
    }
}

/// Queue batch summary for CLI/TUI listing
#[derive(Debug, Clone)]
pub struct QueueBatchSummary {
    pub batch_id: String,
    pub game_id: String,
    pub total: i64,
    pub pending: i64,
    pub matched: i64,
    pub needs_review: i64,
    pub needs_manual: i64,
    pub downloading: i64,
    pub installing: i64,
    pub completed: i64,
    pub failed: i64,
    pub created_at: String,
}

/// Match alternative record
#[derive(Debug, Clone)]
pub struct MatchAlternativeRecord {
    pub id: Option<i64>,
    pub download_id: i64,
    pub nexus_mod_id: i64,
    pub mod_name: String,
    pub match_score: f32,
    pub summary: Option<String>,
    pub downloads_count: Option<i64>,
    pub thumbnail_url: Option<String>,
}

impl MatchAlternativeRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            download_id: row.get(1)?,
            nexus_mod_id: row.get(2)?,
            mod_name: row.get(3)?,
            match_score: row.get(4)?,
            summary: row.get(5)?,
            downloads_count: row.get(6)?,
            thumbnail_url: row.get(7)?,
        })
    }
}

/// Nexus catalog record (locally cached mod listing)
#[derive(Debug, Clone)]
pub struct NexusCatalogRecord {
    pub game_domain: String,
    pub mod_id: i64,
    pub name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub updated_time: Option<i64>,
    pub synced_at: String,
}

impl NexusCatalogRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            game_domain: row.get(0)?,
            mod_id: row.get(1)?,
            name: row.get(2)?,
            summary: row.get(3)?,
            description: row.get(4)?,
            author: row.get(5)?,
            updated_time: row.get(6)?,
            synced_at: row.get(7)?,
        })
    }

    /// Convert to a ModSearchResult for use in the matcher scoring pipeline
    pub fn to_search_result(&self) -> crate::nexus::ModSearchResult {
        crate::nexus::ModSearchResult {
            mod_id: self.mod_id,
            name: self.name.clone(),
            summary: self.summary.clone().unwrap_or_default(),
            version: String::new(),
            author: self.author.clone().unwrap_or_default(),
            category: String::new(),
            downloads: 0,
            endorsements: 0,
            picture_url: None,
            thumbnail_url: None,
            updated_at: self.updated_time.map(|t| t.to_string()).unwrap_or_default(),
            created_at: String::new(),
        }
    }
}

/// Modlist database record (persistent modlist)
#[derive(Debug, Clone)]
pub struct ModlistRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ModlistRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            game_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            source_file: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }
}

/// Modlist entry record (single entry in a modlist)
#[derive(Debug, Clone)]
pub struct ModlistEntryRecord {
    pub id: Option<i64>,
    pub modlist_id: i64,
    pub name: String,
    pub nexus_mod_id: Option<i64>,
    pub plugin_name: Option<String>,
    pub match_confidence: Option<f32>,
    pub position: i32,
    pub enabled: bool,
    pub author: Option<String>,
    pub version: Option<String>,
}

impl ModlistEntryRecord {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: Some(row.get(0)?),
            modlist_id: row.get(1)?,
            name: row.get(2)?,
            nexus_mod_id: row.get(3)?,
            plugin_name: row.get(4)?,
            match_confidence: row.get(5)?,
            position: row.get(6)?,
            enabled: row.get::<_, i32>(7)? != 0,
            author: row.get(8)?,
            version: row.get(9)?,
        })
    }
}

/// Catalog sync state (checkpoint for resume)
#[derive(Debug, Clone)]
pub struct CatalogSyncState {
    pub game_domain: String,
    pub current_page: i32,
    pub completed: bool,
    pub last_sync: Option<String>,
    pub last_error: Option<String>,
}

impl CatalogSyncState {
    pub fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            game_domain: row.get(0)?,
            current_page: row.get(1)?,
            completed: row.get::<_, i32>(2)? != 0,
            last_sync: row.get(3)?,
            last_error: row.get(4)?,
        })
    }
}
