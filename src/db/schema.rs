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
