//! SQLite database for mod tracking

mod schema;

pub use schema::*;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

/// Database wrapper with thread-safe access
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the given path
    pub fn open(path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).context("Failed to open database")?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.init_schema()?;
        db.migrate_downloads_for_import()?;
        db.migrate_categories()?;
        db.init_default_categories()?;
        db.restore_category_mappings()?;
        db.migrate_nexus_catalog()?;
        db.migrate_modlists()?;
        db.migrate_mod_plugin_index()?;
        Ok(db)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            -- Installed mods
            CREATE TABLE IF NOT EXISTS mods (
                id INTEGER PRIMARY KEY,
                game_id TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                author TEXT,
                description TEXT,
                nexus_mod_id INTEGER,
                nexus_file_id INTEGER,
                install_path TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                file_count INTEGER NOT NULL DEFAULT 0,
                installed_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(game_id, name)
            );

            -- Mod files (for conflict detection)
            CREATE TABLE IF NOT EXISTS mod_files (
                id INTEGER PRIMARY KEY,
                mod_id INTEGER NOT NULL,
                relative_path TEXT NOT NULL,
                hash TEXT,
                size INTEGER,
                FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
                UNIQUE(mod_id, relative_path)
            );

            -- Profiles
            CREATE TABLE IF NOT EXISTS profiles (
                id INTEGER PRIMARY KEY,
                game_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(game_id, name)
            );

            -- Profile mod associations
            CREATE TABLE IF NOT EXISTS profile_mods (
                id INTEGER PRIMARY KEY,
                profile_id INTEGER NOT NULL,
                mod_id INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                priority INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
                FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
                UNIQUE(profile_id, mod_id)
            );

            -- Plugin load order
            CREATE TABLE IF NOT EXISTS plugins (
                id INTEGER PRIMARY KEY,
                game_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                load_order INTEGER NOT NULL DEFAULT 0,
                mod_id INTEGER,
                FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE SET NULL,
                UNIQUE(game_id, filename)
            );

            -- Downloads (queue and history)
            CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY,
                game_id TEXT NOT NULL,
                nexus_mod_id INTEGER NOT NULL,
                nexus_file_id INTEGER,
                name TEXT NOT NULL,
                filename TEXT,
                url TEXT,
                size INTEGER,
                downloaded INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'pending',
                error TEXT,
                started_at TEXT,
                completed_at TEXT,
                created_at TEXT NOT NULL
            );

            -- Categories for organizing mods
            CREATE TABLE IF NOT EXISTS categories (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                display_order INTEGER NOT NULL DEFAULT 0,
                color TEXT,
                parent_id INTEGER,
                FOREIGN KEY (parent_id) REFERENCES categories(id) ON DELETE CASCADE
            );

            -- Add category_id column to mods table if it doesn't exist
            "#,
        )?;

        // Check if parent_id column exists in categories, if not add it
        let has_parent_column: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('categories') WHERE name='parent_id'",
            [],
            |row| row.get(0),
        )?;

        if !has_parent_column {
            conn.execute("ALTER TABLE categories ADD COLUMN parent_id INTEGER", [])?;
        }

        // Check if category_id column exists, if not add it
        let has_category_column: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('mods') WHERE name='category_id'",
            [],
            |row| row.get(0),
        )?;

        if !has_category_column {
            conn.execute("ALTER TABLE mods ADD COLUMN category_id INTEGER", [])?;
        }

        conn.execute_batch(
            r#"
            -- FOMOD installation choices (for re-run support)
            CREATE TABLE IF NOT EXISTS fomod_choices (
                id INTEGER PRIMARY KEY,
                mod_id INTEGER NOT NULL,
                profile_id INTEGER,
                config_hash TEXT NOT NULL,
                install_plan_json TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
                FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE,
                UNIQUE(mod_id, profile_id)
            );

            -- Create indices for performance
            CREATE INDEX IF NOT EXISTS idx_mods_category ON mods(category_id);
            CREATE INDEX IF NOT EXISTS idx_mods_game_category ON mods(game_id, category_id);
            CREATE INDEX IF NOT EXISTS idx_fomod_choices_mod ON fomod_choices(mod_id);
            CREATE INDEX IF NOT EXISTS idx_fomod_choices_profile ON fomod_choices(profile_id);

            -- Create indexes
            CREATE INDEX IF NOT EXISTS idx_mods_game ON mods(game_id);
            CREATE INDEX IF NOT EXISTS idx_mod_files_mod ON mod_files(mod_id);
            CREATE INDEX IF NOT EXISTS idx_mod_files_path ON mod_files(relative_path);
            CREATE INDEX IF NOT EXISTS idx_profiles_game ON profiles(game_id);
            CREATE INDEX IF NOT EXISTS idx_plugins_game ON plugins(game_id);
            CREATE INDEX IF NOT EXISTS idx_downloads_game ON downloads(game_id);

            -- Migration version tracking
            CREATE TABLE IF NOT EXISTS schema_version (
                migration_name TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            "#,
        )
        .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Migrate downloads table for import system
    fn migrate_downloads_for_import(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if migration already applied
        let migration_name = "downloads_import_columns_v1";
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE migration_name = ?1",
                params![migration_name],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if already_applied {
            return Ok(());
        }

        tracing::info!("Applying downloads table migration for import system");

        // Add new columns to downloads table
        let columns_to_add = vec![
            ("queue_position", "INTEGER"),
            ("plugin_name", "TEXT"),
            ("match_confidence", "REAL"),
            ("import_batch_id", "TEXT"),
            ("selected_file_id", "INTEGER"),
            ("auto_install", "INTEGER DEFAULT 1"),
        ];

        for (column_name, column_type) in columns_to_add {
            let has_column: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('downloads') WHERE name=?1",
                    params![column_name],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if !has_column {
                conn.execute(
                    &format!(
                        "ALTER TABLE downloads ADD COLUMN {} {}",
                        column_name, column_type
                    ),
                    [],
                )?;
                tracing::info!("Added column '{}' to downloads table", column_name);
            }
        }

        // Create download_match_alternatives table
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS download_match_alternatives (
                id INTEGER PRIMARY KEY,
                download_id INTEGER NOT NULL,
                nexus_mod_id INTEGER NOT NULL,
                mod_name TEXT NOT NULL,
                match_score REAL NOT NULL,
                summary TEXT,
                downloads_count INTEGER,
                thumbnail_url TEXT,
                FOREIGN KEY (download_id) REFERENCES downloads(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_downloads_queue ON downloads(game_id, status, queue_position);
            CREATE INDEX IF NOT EXISTS idx_downloads_batch ON downloads(import_batch_id);
            CREATE INDEX IF NOT EXISTS idx_match_alternatives_download ON download_match_alternatives(download_id);
            "#,
        )?;

        // Mark migration as complete
        conn.execute(
            "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
            params![migration_name],
        )?;

        tracing::info!("Downloads table migration completed successfully");
        Ok(())
    }

    // ========== Mod Operations ==========

    /// Insert a new mod
    pub fn insert_mod(&self, m: &ModRecord) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO mods (game_id, name, version, author, description, nexus_mod_id,
                              nexus_file_id, install_path, enabled, priority, file_count,
                              installed_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                m.game_id,
                m.name,
                m.version,
                m.author,
                m.description,
                m.nexus_mod_id,
                m.nexus_file_id,
                m.install_path,
                m.enabled as i32,
                m.priority,
                m.file_count,
                m.installed_at,
                m.updated_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get a mod by game and name
    pub fn get_mod(&self, game_id: &str, name: &str) -> Result<Option<ModRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM mods WHERE game_id = ?1 AND name = ?2",
            params![game_id, name],
            |row| ModRecord::from_row(row),
        )
        .optional()
        .context("Failed to query mod")
    }

    /// Get all mods for a game
    pub fn get_mods_for_game(&self, game_id: &str) -> Result<Vec<ModRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM mods WHERE game_id = ?1 ORDER BY priority ASC, name ASC")?;

        let mods = stmt
            .query_map(params![game_id], |row| ModRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }

    /// Update mod enabled status
    pub fn set_mod_enabled(&self, mod_id: i64, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE mods SET enabled = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![enabled as i32, mod_id],
        )?;
        Ok(())
    }

    /// Update mod priority
    pub fn set_mod_priority(&self, mod_id: i64, priority: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE mods SET priority = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![priority, mod_id],
        )?;
        Ok(())
    }

    /// Update mod display name.
    pub fn set_mod_name(&self, mod_id: i64, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE mods SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![name, mod_id],
        )?;
        Ok(())
    }

    /// Delete a mod
    pub fn delete_mod(&self, mod_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM mods WHERE id = ?1", params![mod_id])?;
        Ok(())
    }

    /// Get a mod by ID
    pub fn get_mod_by_id(&self, mod_id: i64) -> Result<Option<ModRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT * FROM mods WHERE id = ?1", params![mod_id], |row| {
            ModRecord::from_row(row)
        })
        .optional()
        .map_err(Into::into)
    }

    /// Update a mod record
    pub fn update_mod(&self, m: &ModRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE mods SET
                game_id = ?2,
                name = ?3,
                version = ?4,
                author = ?5,
                description = ?6,
                nexus_mod_id = ?7,
                nexus_file_id = ?8,
                install_path = ?9,
                enabled = ?10,
                priority = ?11,
                file_count = ?12,
                updated_at = ?13,
                category_id = ?14
            WHERE id = ?1
            "#,
            params![
                m.id.unwrap(),
                m.game_id,
                m.name,
                m.version,
                m.author,
                m.description,
                m.nexus_mod_id,
                m.nexus_file_id,
                m.install_path,
                m.enabled,
                m.priority,
                m.file_count,
                m.updated_at,
                m.category_id,
            ],
        )?;
        Ok(())
    }

    // ========== Mod Files Operations ==========

    /// Insert mod files
    pub fn insert_mod_files(&self, mod_id: i64, files: &[ModFileRecord]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "INSERT INTO mod_files (mod_id, relative_path, hash, size) VALUES (?1, ?2, ?3, ?4)",
        )?;

        for f in files {
            stmt.execute(params![mod_id, f.relative_path, f.hash, f.size])?;
        }

        // Update file count
        conn.execute(
            "UPDATE mods SET file_count = ?1 WHERE id = ?2",
            params![files.len() as i32, mod_id],
        )?;

        Ok(())
    }

    /// Get files for a mod
    pub fn get_mod_files(&self, mod_id: i64) -> Result<Vec<ModFileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM mod_files WHERE mod_id = ?1")?;

        let files = stmt
            .query_map(params![mod_id], |row| ModFileRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    /// Delete all file records for a mod
    pub fn delete_mod_files(&self, mod_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM mod_files WHERE mod_id = ?1", params![mod_id])?;
        Ok(())
    }

    /// Replace indexed plugin filename mappings for a mod.
    pub fn replace_mod_plugins(
        &self,
        mod_id: i64,
        game_id: &str,
        plugin_names: &[String],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM mod_plugins WHERE mod_id = ?1", params![mod_id])?;

        let mut stmt = tx.prepare(
            "INSERT INTO mod_plugins (mod_id, game_id, plugin_name, plugin_name_lower)
             VALUES (?1, ?2, ?3, ?4)",
        )?;

        for plugin_name in plugin_names {
            let trimmed = plugin_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            stmt.execute(params![mod_id, game_id, trimmed, trimmed.to_lowercase()])?;
        }

        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    /// Find installed mods associated with a plugin filename.
    pub fn find_mods_by_plugin_filename(
        &self,
        game_id: &str,
        plugin_name: &str,
    ) -> Result<Vec<PluginModLookupHit>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT m.id, m.name, m.version, m.nexus_mod_id, mp.plugin_name
            FROM mod_plugins mp
            JOIN mods m ON m.id = mp.mod_id
            WHERE mp.game_id = ?1
              AND mp.plugin_name_lower = ?2
            ORDER BY
                CASE WHEN m.nexus_mod_id IS NULL THEN 1 ELSE 0 END,
                m.priority DESC,
                m.updated_at DESC
            "#,
        )?;

        let lower = plugin_name.to_lowercase();
        let hits = stmt
            .query_map(params![game_id, lower], |row| {
                Ok(PluginModLookupHit {
                    mod_id: row.get(0)?,
                    mod_name: row.get(1)?,
                    mod_version: row.get(2)?,
                    nexus_mod_id: row.get(3)?,
                    plugin_name: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(hits)
    }

    /// Find conflicting files between mods
    pub fn find_conflicts(&self, game_id: &str) -> Result<Vec<FileConflict>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT f1.relative_path, m1.name as mod1, m2.name as mod2, m1.priority as p1, m2.priority as p2
            FROM mod_files f1
            JOIN mod_files f2 ON f1.relative_path = f2.relative_path AND f1.mod_id < f2.mod_id
            JOIN mods m1 ON f1.mod_id = m1.id
            JOIN mods m2 ON f2.mod_id = m2.id
            WHERE m1.game_id = ?1 AND m2.game_id = ?1 AND m1.enabled = 1 AND m2.enabled = 1
            ORDER BY f1.relative_path
            "#,
        )?;

        let conflicts = stmt
            .query_map(params![game_id], |row| {
                Ok(FileConflict {
                    path: row.get(0)?,
                    mod1: row.get(1)?,
                    mod2: row.get(2)?,
                    priority1: row.get(3)?,
                    priority2: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(conflicts)
    }

    /// Get all installed files for a game
    pub fn get_all_files(&self, game_id: &str) -> Result<Vec<InstalledFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT f.relative_path, m.name, m.id
            FROM mod_files f
            JOIN mods m ON f.mod_id = m.id
            WHERE m.game_id = ?1 AND m.enabled = 1
            ORDER BY f.relative_path
            "#,
        )?;

        let files = stmt
            .query_map(params![game_id], |row| {
                Ok(InstalledFile {
                    path: row.get(0)?,
                    mod_name: row.get(1)?,
                    mod_id: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    // ========== Profile Operations ==========

    /// Insert a new profile
    pub fn insert_profile(&self, p: &ProfileRecord) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO profiles (game_id, name, description, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![p.game_id, p.name, p.description, p.created_at, p.updated_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get profiles for a game
    pub fn get_profiles_for_game(&self, game_id: &str) -> Result<Vec<ProfileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM profiles WHERE game_id = ?1 ORDER BY name")?;

        let profiles = stmt
            .query_map(params![game_id], |row| ProfileRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }

    /// Delete a profile
    pub fn delete_profile(&self, profile_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM profiles WHERE id = ?1", params![profile_id])?;
        Ok(())
    }

    // ========== Category Operations ==========

    /// Insert a new category
    pub fn insert_category(&self, c: &CategoryRecord) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO categories (name, description, display_order, color, parent_id)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![c.name, c.description, c.display_order, c.color, c.parent_id],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all categories ordered by display_order
    pub fn get_all_categories(&self) -> Result<Vec<CategoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM categories ORDER BY display_order ASC")?;

        let categories = stmt
            .query_map([], |row| CategoryRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(categories)
    }

    /// Get a category by ID
    pub fn get_category(&self, category_id: i64) -> Result<Option<CategoryRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM categories WHERE id = ?1",
            params![category_id],
            |row| CategoryRecord::from_row(row),
        )
        .optional()
        .context("Failed to query category")
    }

    /// Get a category by name
    pub fn get_category_by_name(&self, name: &str) -> Result<Option<CategoryRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM categories WHERE name = ?1",
            params![name],
            |row| CategoryRecord::from_row(row),
        )
        .optional()
        .context("Failed to query category")
    }

    /// Update mod's category
    pub fn update_mod_category(&self, mod_id: i64, category_id: Option<i64>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE mods SET category_id = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![category_id, mod_id],
        )?;
        Ok(())
    }

    /// Get mods for a game filtered by category
    pub fn get_mods_by_category(
        &self,
        game_id: &str,
        category_id: Option<i64>,
    ) -> Result<Vec<ModRecord>> {
        let conn = self.conn.lock().unwrap();

        let query = match category_id {
            Some(_) => "SELECT * FROM mods WHERE game_id = ?1 AND category_id = ?2 ORDER BY priority ASC, name ASC",
            None => "SELECT * FROM mods WHERE game_id = ?1 AND category_id IS NULL ORDER BY priority ASC, name ASC",
        };

        let mut stmt = conn.prepare(query)?;

        let mods = if let Some(cat_id) = category_id {
            stmt.query_map(params![game_id, cat_id], |row| ModRecord::from_row(row))?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![game_id], |row| ModRecord::from_row(row))?
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(mods)
    }

    /// Migrate old category names to the updated naming scheme.
    /// Completely rebuilds the category table while preserving mod associations.
    fn migrate_categories(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if this migration has already been applied
        let migration_name = "category_rebuild_v1";
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE migration_name = ?1",
                params![migration_name],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if already_applied {
            // Migration already completed, skip
            return Ok(());
        }

        // Check if we have any categories that need migration
        let category_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .unwrap_or(0);

        if category_count == 0 {
            // No categories exist yet, mark migration as complete and return
            conn.execute(
                "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
                params![migration_name],
            )?;
            return Ok(());
        }

        tracing::info!(
            "Migrating {} existing categories to new structure",
            category_count
        );

        // Create temporary table to store mod->category mappings
        conn.execute_batch(
            r#"
            CREATE TEMPORARY TABLE IF NOT EXISTS temp_mod_categories (
                mod_id INTEGER,
                old_category_name TEXT,
                new_category_name TEXT
            );
            "#,
        )?;

        // Map old category names to new ones
        let category_mapping = [
            ("Structure/UI", "Structure and UI Mods"),
            ("Content Correction", "Mission and Content Correction"),
            ("Difficulty/Level", "Difficulty/Level List Mods"),
            ("Environmental", "Environmental Mods"),
            ("Global Mesh", "Global Mesh Mods"),
            ("Foliage", "Foliage Mods"),
            ("Sound", "Sound Mods"),
            ("Robust Gameplay", "Robust Gameplay Changes"),
            ("Crafting", "Crafting Mods"),
            ("Appearance", "Appearance Mods"),
            ("Hair Mods", "Hairdo Mods"),
            ("Body Mods", "Body Mesh Mods"),
            ("Eye Mods", "Natural Eyes"),
            ("Textures", "Texture Mods"),
            ("Performance Patches", "Performance/Disable Patches"),
            // Categories that stay the same
            ("Bug Fixes", "Bug Fixes"),
            ("Missions/Quests", "Missions/Quests"),
            ("Buildings", "Buildings"),
            ("Items", "Items"),
            ("Gameplay", "Gameplay"),
            ("NPCs", "NPCs"),
            ("Patches", "Patches"),
            ("Texture Mods", "Texture Mods"),
            // Subcategories that exist in new schema
            ("Overhauls", "Overhauls"),
            (
                "Mission and Content Correction",
                "Mission and Content Correction",
            ),
            ("Difficulty/Level List Mods", "Difficulty/Level List Mods"),
            ("Race Mods", "Race Mods"),
            ("Perk Mods", "Perk Mods"),
            ("UI Mods", "UI Mods"),
            ("Cheat Mods", "Cheat Mods"),
            ("Global Mesh Mods", "Global Mesh Mods"),
            ("Weather/Lighting", "Weather/Lighting"),
            ("Foliage Mods", "Foliage Mods"),
            ("Sound Mods", "Sound Mods"),
            ("Distributed Content", "Distributed Content"),
            ("Settlements", "Settlements"),
            ("Individual Buildings", "Individual Buildings"),
            ("Building Interiors", "Building Interiors"),
            ("Item Packs", "Item Packs"),
            ("Individual Items", "Individual Items"),
            ("AI Mods", "AI Mods"),
            ("Robust Gameplay Changes", "Robust Gameplay Changes"),
            ("Expanded Armor", "Expanded Armor"),
            ("Crafting Mods", "Crafting Mods"),
            ("Other Gameplay", "Other Gameplay"),
            ("NPC Overhauls", "NPC Overhauls"),
            ("Populated Series", "Populated Series"),
            ("Other NPC Additions", "Other NPC Additions"),
            ("Hairdo Mods", "Hairdo Mods"),
            ("Adorable Females", "Adorable Females"),
            ("Face Mods", "Face Mods"),
            ("Body Mesh Mods", "Body Mesh Mods"),
            ("Natural Eyes", "Natural Eyes"),
            ("Other Appearance", "Other Appearance"),
            ("Compatibility Patches", "Compatibility Patches"),
            ("Content Patches", "Content Patches"),
            ("Performance/Disable Patches", "Performance/Disable Patches"),
        ];

        // Save mod category associations with name mapping
        for (old_name, new_name) in &category_mapping {
            conn.execute(
                r#"
                INSERT INTO temp_mod_categories (mod_id, old_category_name, new_category_name)
                SELECT m.id, c.name, ?1
                FROM mods m
                JOIN categories c ON m.category_id = c.id
                WHERE c.name = ?2
                "#,
                params![new_name, old_name],
            )?;
        }

        // Also save any categories that don't have a mapping (will become uncategorized)
        let saved_mods: i64 =
            conn.query_row("SELECT COUNT(*) FROM temp_mod_categories", [], |row| {
                row.get(0)
            })?;

        tracing::info!("Saved {} mod category associations", saved_mods);

        // Clear all mod category associations
        let cleared = conn.execute("UPDATE mods SET category_id = NULL", [])?;
        tracing::info!("Cleared {} mod category assignments", cleared);

        // Delete all existing categories
        let deleted = conn.execute("DELETE FROM categories", [])?;
        tracing::info!("Deleted {} old categories", deleted);

        // Mark migration as complete
        conn.execute(
            "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
            params![migration_name],
        )?;
        tracing::info!("Category migration completed successfully");

        Ok(())
    }

    /// Restore mod category associations after rebuilding categories.
    /// Called after init_default_categories() has recreated the category structure.
    fn restore_category_mappings(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if temp table exists (means we did a migration)
        let temp_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='temp_mod_categories'",
                [],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if !temp_exists {
            // No migration happened, nothing to restore
            return Ok(());
        }

        // Restore mod category associations using new category IDs
        let restored = conn.execute(
            r#"
            UPDATE mods
            SET category_id = (
                SELECT c.id
                FROM categories c
                JOIN temp_mod_categories tmc ON c.name = tmc.new_category_name
                WHERE tmc.mod_id = mods.id
            )
            WHERE id IN (SELECT mod_id FROM temp_mod_categories)
            "#,
            [],
        )?;

        tracing::info!("Restored {} mod category associations", restored);

        // Clean up temporary table
        conn.execute("DROP TABLE IF EXISTS temp_mod_categories", [])?;

        Ok(())
    }

    /// Initialize default categories based on the standard 11-category mod organization
    pub fn init_default_categories(&self) -> Result<()> {
        // 11 Parent categories (name, description, order, color)
        let parent_categories = vec![
            (
                "Bug Fixes",
                "Critical bug fixes and unofficial patches (e.g. USSEP)",
                0,
                "#FF5555",
            ),
            (
                "Structure and UI Mods",
                "Overhauls, UI improvements, frameworks, and system mods",
                1,
                "#55FF55",
            ),
            (
                "Missions/Quests",
                "New quests, questlines, and mission content",
                2,
                "#FFFF55",
            ),
            (
                "Environmental Mods",
                "Meshes, weather, foliage, and sound mods",
                3,
                "#55FFFF",
            ),
            (
                "Buildings",
                "Architecture, settlements, and structure mods",
                4,
                "#AAFF55",
            ),
            (
                "Items",
                "Weapons, armor, equipment, and item additions",
                5,
                "#55AAFF",
            ),
            (
                "Gameplay",
                "Gameplay mechanics, AI, balance, and system changes",
                6,
                "#AA55FF",
            ),
            (
                "NPCs",
                "NPC additions, overhauls, and follower mods",
                7,
                "#FFAAAA",
            ),
            (
                "Appearance Mods",
                "Character appearance, faces, hair, bodies, and cosmetics",
                8,
                "#AAFFAA",
            ),
            (
                "Texture Mods",
                "Texture overhauls, retextures, and visual improvements",
                9,
                "#AAAAFF",
            ),
            (
                "Patches",
                "Compatibility patches and load order fixes",
                10,
                "#AAAAAA",
            ),
        ];

        for (name, description, display_order, color) in parent_categories {
            if self.get_category_by_name(name)?.is_none() {
                self.insert_category(&CategoryRecord {
                    id: None,
                    name: name.to_string(),
                    description: Some(description.to_string()),
                    display_order,
                    color: Some(color.to_string()),
                    parent_id: None,
                })?;
            }
        }

        // Look up parent IDs for subcategories
        let structure_ui_id = self
            .get_category_by_name("Structure and UI Mods")?
            .and_then(|c| c.id);
        let environmental_id = self
            .get_category_by_name("Environmental Mods")?
            .and_then(|c| c.id);
        let buildings_id = self.get_category_by_name("Buildings")?.and_then(|c| c.id);
        let items_id = self.get_category_by_name("Items")?.and_then(|c| c.id);
        let gameplay_id = self.get_category_by_name("Gameplay")?.and_then(|c| c.id);
        let npcs_id = self.get_category_by_name("NPCs")?.and_then(|c| c.id);
        let appearance_id = self
            .get_category_by_name("Appearance Mods")?
            .and_then(|c| c.id);
        let patches_id = self.get_category_by_name("Patches")?.and_then(|c| c.id);

        // Subcategories (name, description, order, color, parent_id)
        let mut subcategories: Vec<(&str, &str, i32, &str, Option<i64>)> = vec![];

        // 2. Structure and UI Mods subcategories (A-G)
        if let Some(pid) = structure_ui_id {
            subcategories.extend_from_slice(&[
                (
                    "Overhauls",
                    "Major gameplay and system overhauls (e.g. Campfire, Frostfall)",
                    100,
                    "#5555FF",
                    Some(pid),
                ),
                (
                    "Mission and Content Correction",
                    "Mission and content correction (e.g. Cutting Room Floor)",
                    101,
                    "#5577FF",
                    Some(pid),
                ),
                (
                    "Difficulty/Level List Mods",
                    "Difficulty and level list mods",
                    102,
                    "#5599FF",
                    Some(pid),
                ),
                (
                    "Race Mods",
                    "Race additions and modifications",
                    103,
                    "#55BBFF",
                    Some(pid),
                ),
                (
                    "Perk Mods",
                    "Perk system modifications",
                    104,
                    "#55DDFF",
                    Some(pid),
                ),
                (
                    "UI Mods",
                    "User interface enhancements",
                    105,
                    "#55FFFF",
                    Some(pid),
                ),
                (
                    "Cheat Mods",
                    "Cheat and convenience mods",
                    106,
                    "#77FFFF",
                    Some(pid),
                ),
            ]);
        }

        // 4. Environmental Mods subcategories (A-D)
        if let Some(pid) = environmental_id {
            subcategories.extend_from_slice(&[
                (
                    "Global Mesh Mods",
                    "Global mesh improvements (e.g. SMIM)",
                    200,
                    "#FF55FF",
                    Some(pid),
                ),
                (
                    "Weather/Lighting",
                    "Weather and lighting overhauls",
                    201,
                    "#FFAA55",
                    Some(pid),
                ),
                (
                    "Foliage Mods",
                    "Trees, grass, and plant mods",
                    202,
                    "#77FF77",
                    Some(pid),
                ),
                (
                    "Sound Mods",
                    "Audio, music, and sound effects",
                    203,
                    "#FFAAFF",
                    Some(pid),
                ),
            ]);
        }

        // 5. Buildings subcategories (A-D)
        if let Some(pid) = buildings_id {
            subcategories.extend_from_slice(&[
                (
                    "Distributed Content",
                    "Distributed or worldwide content (e.g. Dolmen Ruins, Oblivion Gates)",
                    300,
                    "#AAFF55",
                    Some(pid),
                ),
                (
                    "Settlements",
                    "Settlement additions and expansions",
                    301,
                    "#BBFF66",
                    Some(pid),
                ),
                (
                    "Individual Buildings",
                    "Individual building additions",
                    302,
                    "#CCFF77",
                    Some(pid),
                ),
                (
                    "Building Interiors",
                    "Building interior modifications",
                    303,
                    "#DDFF88",
                    Some(pid),
                ),
            ]);
        }

        // 6. Items subcategories (A-B)
        if let Some(pid) = items_id {
            subcategories.extend_from_slice(&[
                (
                    "Item Packs",
                    "Collections and packs of items",
                    400,
                    "#5599FF",
                    Some(pid),
                ),
                (
                    "Individual Items",
                    "Single item additions",
                    401,
                    "#66AAFF",
                    Some(pid),
                ),
            ]);
        }

        // 7. Gameplay subcategories (A-E)
        if let Some(pid) = gameplay_id {
            subcategories.extend_from_slice(&[
                (
                    "AI Mods",
                    "AI improvements (e.g. Immersive Citizens)",
                    500,
                    "#AA55FF",
                    Some(pid),
                ),
                (
                    "Robust Gameplay Changes",
                    "Major gameplay changes (e.g. Marriage All, Alternate Start)",
                    501,
                    "#BB66FF",
                    Some(pid),
                ),
                (
                    "Expanded Armor",
                    "Expanded armor and equipment (e.g. Magic Books, Pouches)",
                    502,
                    "#CC77FF",
                    Some(pid),
                ),
                (
                    "Crafting Mods",
                    "Crafting system modifications",
                    503,
                    "#DD88FF",
                    Some(pid),
                ),
                (
                    "Other Gameplay",
                    "Other gameplay mods (e.g. Rich Merchants, Faster Greatswords)",
                    504,
                    "#EE99FF",
                    Some(pid),
                ),
            ]);
        }

        // 8. NPCs subcategories (A-C)
        if let Some(pid) = npcs_id {
            subcategories.extend_from_slice(&[
                (
                    "NPC Overhauls",
                    "NPC overhauls (e.g. Diverse Dragons)",
                    600,
                    "#FFAAAA",
                    Some(pid),
                ),
                (
                    "Populated Series",
                    "Populated series mods",
                    601,
                    "#FFBBBB",
                    Some(pid),
                ),
                (
                    "Other NPC Additions",
                    "Other NPC additions",
                    602,
                    "#FFCCCC",
                    Some(pid),
                ),
            ]);
        }

        // 9. Appearance Mods subcategories (A-F)
        if let Some(pid) = appearance_id {
            subcategories.extend_from_slice(&[
                (
                    "Hairdo Mods",
                    "Hairstyle additions",
                    700,
                    "#AAFFAA",
                    Some(pid),
                ),
                (
                    "Adorable Females",
                    "Female beauty and attractiveness mods",
                    701,
                    "#99FF99",
                    Some(pid),
                ),
                (
                    "Face Mods",
                    "Facial appearance modifications",
                    702,
                    "#BBFFBB",
                    Some(pid),
                ),
                (
                    "Body Mesh Mods",
                    "Body mesh and texture mods (e.g. Seraphim, CBBE, Dimon99)",
                    703,
                    "#CCFFCC",
                    Some(pid),
                ),
                (
                    "Natural Eyes",
                    "Eye textures and modifications",
                    704,
                    "#DDFFDD",
                    Some(pid),
                ),
                (
                    "Other Appearance",
                    "Other appearance modifications",
                    705,
                    "#EEFFEE",
                    Some(pid),
                ),
            ]);
        }

        // 11. Patches subcategories (A-C)
        if let Some(pid) = patches_id {
            subcategories.extend_from_slice(&[
                (
                    "Compatibility Patches",
                    "Patches for earlier mods (e.g. Apocalypse-Ordinator Compatibility Patch)",
                    800,
                    "#AAAAAA",
                    Some(pid),
                ),
                (
                    "Content Patches",
                    "Patches that alter content",
                    801,
                    "#BBBBBB",
                    Some(pid),
                ),
                (
                    "Performance/Disable Patches",
                    "Patches that disable content or improve performance",
                    802,
                    "#CCCCCC",
                    Some(pid),
                ),
            ]);
        }

        // Create subcategories (skip if already exists)
        for (name, description, display_order, color, parent_id) in subcategories {
            if self.get_category_by_name(name)?.is_none() {
                self.insert_category(&CategoryRecord {
                    id: None,
                    name: name.to_string(),
                    description: Some(description.to_string()),
                    display_order,
                    color: Some(color.to_string()),
                    parent_id,
                })?;
            }
        }

        Ok(())
    }

    // ========== FOMOD Choice Operations ==========

    /// Save a FOMOD installation choice
    pub fn save_fomod_choice(
        &self,
        mod_id: i64,
        profile_id: Option<i64>,
        config_hash: &str,
        install_plan_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Use INSERT OR REPLACE to handle both new and existing choices
        conn.execute(
            r#"
            INSERT OR REPLACE INTO fomod_choices (mod_id, profile_id, config_hash, install_plan_json, installed_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![mod_id, profile_id, config_hash, install_plan_json, now],
        )?;

        Ok(())
    }

    /// Get a FOMOD choice for a mod and profile
    pub fn get_fomod_choice(
        &self,
        mod_id: i64,
        profile_id: Option<i64>,
    ) -> Result<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            r#"
            SELECT config_hash, install_plan_json
            FROM fomod_choices
            WHERE mod_id = ?1 AND (profile_id = ?2 OR (profile_id IS NULL AND ?2 IS NULL))
            "#,
            params![mod_id, profile_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .context("Failed to query FOMOD choice")
    }

    /// Delete a FOMOD choice
    pub fn delete_fomod_choice(&self, mod_id: i64, profile_id: Option<i64>) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            r#"
            DELETE FROM fomod_choices
            WHERE mod_id = ?1 AND (profile_id = ?2 OR (profile_id IS NULL AND ?2 IS NULL))
            "#,
            params![mod_id, profile_id],
        )?;

        Ok(())
    }

    /// Get all FOMOD choices for a profile
    pub fn get_profile_fomod_choices(&self, profile_id: i64) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT mod_id, config_hash, install_plan_json
            FROM fomod_choices
            WHERE profile_id = ?1
            "#,
        )?;

        let choices = stmt
            .query_map(params![profile_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(choices)
    }

    /// Get all FOMOD choices for a mod across all profiles
    pub fn get_mod_fomod_choices(&self, mod_id: i64) -> Result<Vec<(Option<i64>, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT profile_id, config_hash, install_plan_json
            FROM fomod_choices
            WHERE mod_id = ?1
            "#,
        )?;

        let choices = stmt
            .query_map(params![mod_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(choices)
    }

    // ========== Download Queue Operations ==========

    /// Add a download to the queue
    pub fn insert_download_queue_entry(&self, entry: &DownloadQueueEntry) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO downloads (
                game_id, nexus_mod_id, nexus_file_id, name, filename, status,
                queue_position, plugin_name, match_confidence, import_batch_id,
                selected_file_id, auto_install, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
            "#,
            params![
                entry.game_id,
                entry.nexus_mod_id,
                entry.nexus_file_id,
                entry.name,
                entry.filename,
                entry.status,
                entry.queue_position,
                entry.plugin_name,
                entry.match_confidence,
                entry.import_batch_id,
                entry.selected_file_id,
                entry.auto_install as i32,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all queue entries for a batch
    pub fn get_queue_entries(&self, batch_id: &str) -> Result<Vec<DownloadQueueEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT * FROM downloads
            WHERE import_batch_id = ?1
            ORDER BY queue_position ASC
            "#,
        )?;

        let entries = stmt
            .query_map(params![batch_id], |row| DownloadQueueEntry::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Resolve a queue entry by assigning/modifying its Nexus mod target and setting status.
    pub fn resolve_queue_entry(
        &self,
        download_id: i64,
        nexus_mod_id: i64,
        mod_name: &str,
        status: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE downloads
            SET nexus_mod_id = ?1,
                name = ?2,
                status = ?3,
                error = NULL
            WHERE id = ?4
            "#,
            params![nexus_mod_id, mod_name, status, download_id],
        )?;
        Ok(())
    }

    /// List queue batches with per-status summary counts
    pub fn list_queue_batches(&self, game_id: Option<&str>) -> Result<Vec<QueueBatchSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut query = String::from(
            r#"
            SELECT
                import_batch_id,
                game_id,
                COUNT(*) as total,
                SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending,
                SUM(CASE WHEN status = 'matched' THEN 1 ELSE 0 END) as matched,
                SUM(CASE WHEN status = 'needs_review' THEN 1 ELSE 0 END) as needs_review,
                SUM(CASE WHEN status = 'needs_manual' THEN 1 ELSE 0 END) as needs_manual,
                SUM(CASE WHEN status = 'downloading' THEN 1 ELSE 0 END) as downloading,
                SUM(CASE WHEN status = 'installing' THEN 1 ELSE 0 END) as installing,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed,
                MAX(created_at) as created_at
            FROM downloads
            WHERE import_batch_id IS NOT NULL
            "#,
        );

        if game_id.is_some() {
            query.push_str(" AND game_id = ?1");
        }

        query.push_str(
            r#"
            GROUP BY import_batch_id, game_id
            ORDER BY MAX(created_at) DESC
            "#,
        );

        let mut stmt = conn.prepare(&query)?;

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<QueueBatchSummary> {
            Ok(QueueBatchSummary {
                batch_id: row.get(0)?,
                game_id: row.get(1)?,
                total: row.get(2)?,
                pending: row.get(3)?,
                matched: row.get(4)?,
                needs_review: row.get(5)?,
                needs_manual: row.get(6)?,
                downloading: row.get(7)?,
                installing: row.get(8)?,
                completed: row.get(9)?,
                failed: row.get(10)?,
                created_at: row.get(11)?,
            })
        };

        let rows = if let Some(id) = game_id {
            stmt.query_map(params![id], map_row)?
        } else {
            stmt.query_map([], map_row)?
        };

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Get batch IDs that currently have failed entries
    pub fn get_failed_batches(&self, game_id: Option<&str>) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut query = String::from(
            r#"
            SELECT DISTINCT import_batch_id
            FROM downloads
            WHERE import_batch_id IS NOT NULL
              AND status = 'failed'
            "#,
        );

        if game_id.is_some() {
            query.push_str(" AND game_id = ?1");
        }

        query.push_str(" ORDER BY import_batch_id ASC");

        let mut stmt = conn.prepare(&query)?;

        if let Some(id) = game_id {
            let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        } else {
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        }
    }

    /// Reset failed entries to pending for a specific batch
    pub fn retry_failed_in_batch(&self, batch_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            r#"
            UPDATE downloads
            SET status = 'pending',
                error = NULL,
                started_at = NULL,
                completed_at = NULL,
                downloaded = 0,
                size = NULL
            WHERE import_batch_id = ?1
              AND status = 'failed'
            "#,
            params![batch_id],
        )?;
        Ok(affected)
    }

    /// Update download status
    pub fn update_download_status(
        &self,
        download_id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        if status == "downloading" {
            conn.execute(
                "UPDATE downloads SET status = ?1, started_at = datetime('now') WHERE id = ?2",
                params![status, download_id],
            )?;
        } else if status == "completed" || status == "failed" {
            conn.execute(
                "UPDATE downloads SET status = ?1, error = ?2, completed_at = datetime('now') WHERE id = ?3",
                params![status, error, download_id],
            )?;
        } else {
            conn.execute(
                "UPDATE downloads SET status = ?1, error = ?2 WHERE id = ?3",
                params![status, error, download_id],
            )?;
        }

        Ok(())
    }

    /// Update download progress
    pub fn update_download_progress(
        &self,
        download_id: i64,
        downloaded: i64,
        size: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if let Some(total_size) = size {
            conn.execute(
                "UPDATE downloads SET downloaded = ?1, size = ?2 WHERE id = ?3",
                params![downloaded, total_size, download_id],
            )?;
        } else {
            conn.execute(
                "UPDATE downloads SET downloaded = ?1 WHERE id = ?2",
                params![downloaded, download_id],
            )?;
        }
        Ok(())
    }

    /// Update queue entry display name without changing status.
    pub fn update_download_name(&self, download_id: i64, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET name = ?1 WHERE id = ?2",
            params![name, download_id],
        )?;
        Ok(())
    }

    /// Insert match alternatives for a download
    pub fn insert_match_alternatives(
        &self,
        download_id: i64,
        alternatives: &[MatchAlternativeRecord],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            INSERT INTO download_match_alternatives (
                download_id, nexus_mod_id, mod_name, match_score, summary, downloads_count, thumbnail_url
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )?;

        for alt in alternatives {
            stmt.execute(params![
                download_id,
                alt.nexus_mod_id,
                alt.mod_name,
                alt.match_score,
                alt.summary,
                alt.downloads_count,
                alt.thumbnail_url,
            ])?;
        }

        Ok(())
    }

    /// Get match alternatives for a download
    pub fn get_match_alternatives(&self, download_id: i64) -> Result<Vec<MatchAlternativeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM download_match_alternatives WHERE download_id = ?1 ORDER BY match_score DESC",
        )?;

        let alternatives = stmt
            .query_map(params![download_id], |row| {
                MatchAlternativeRecord::from_row(row)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(alternatives)
    }

    /// Delete a download queue entry
    pub fn delete_download(&self, download_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![download_id])?;
        Ok(())
    }

    /// Clear all downloads for a batch
    pub fn clear_batch(&self, batch_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM downloads WHERE import_batch_id = ?1",
            params![batch_id],
        )?;
        Ok(())
    }

    // ========== Nexus Catalog Operations ==========

    /// Migrate database schema for Nexus catalog
    fn migrate_nexus_catalog(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Check if migration already applied
        let migration_name = "nexus_catalog_v1";
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE migration_name = ?1",
                params![migration_name],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if already_applied {
            return Ok(());
        }

        tracing::info!("Applying Nexus catalog migration");

        conn.execute_batch(
            r#"
            -- Nexus catalog (locally cached mod listings)
            CREATE TABLE IF NOT EXISTS nexus_catalog (
                game_domain TEXT NOT NULL,
                mod_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                summary TEXT,
                description TEXT,
                author TEXT,
                updated_time INTEGER,
                synced_at TEXT NOT NULL,
                PRIMARY KEY (game_domain, mod_id)
            );

            -- Catalog sync state (checkpoint for resume)
            CREATE TABLE IF NOT EXISTS catalog_sync_state (
                game_domain TEXT PRIMARY KEY,
                current_page INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0,
                last_sync TEXT,
                last_error TEXT
            );

            -- Indexes for efficient searches
            CREATE INDEX IF NOT EXISTS idx_catalog_game ON nexus_catalog(game_domain);
            CREATE INDEX IF NOT EXISTS idx_catalog_name ON nexus_catalog(game_domain, name);
            CREATE INDEX IF NOT EXISTS idx_catalog_updated ON nexus_catalog(game_domain, updated_time);
            "#,
        )?;

        // Mark migration as complete
        conn.execute(
            "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
            params![migration_name],
        )?;

        tracing::info!("Nexus catalog migration completed successfully");
        Ok(())
    }

    /// Get sync state for a game domain
    pub fn get_sync_state(&self, game_domain: &str) -> Result<CatalogSyncState> {
        let conn = self.conn.lock().unwrap();

        match conn
            .query_row(
                "SELECT * FROM catalog_sync_state WHERE game_domain = ?1",
                params![game_domain],
                |row| CatalogSyncState::from_row(row),
            )
            .optional()?
        {
            Some(state) => Ok(state),
            None => {
                // Initialize new state (API pagination starts at 1)
                conn.execute(
                    r#"
                    INSERT INTO catalog_sync_state (game_domain, current_page, completed)
                    VALUES (?1, 1, 0)
                    "#,
                    params![game_domain],
                )?;
                Ok(CatalogSyncState {
                    game_domain: game_domain.to_string(),
                    current_page: 1,
                    completed: false,
                    last_sync: None,
                    last_error: None,
                })
            }
        }
    }

    /// Reset sync state for a game domain
    pub fn reset_sync_state(&self, game_domain: &str) -> Result<CatalogSyncState> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            r#"
            INSERT OR REPLACE INTO catalog_sync_state (game_domain, current_page, completed, last_sync, last_error)
            VALUES (?1, 1, 0, NULL, NULL)
            "#,
            params![game_domain],
        )?;

        Ok(CatalogSyncState {
            game_domain: game_domain.to_string(),
            current_page: 1,
            completed: false,
            last_sync: None,
            last_error: None,
        })
    }

    /// Update current page checkpoint
    pub fn update_sync_page(&self, game_domain: &str, page: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE catalog_sync_state SET current_page = ?1 WHERE game_domain = ?2",
            params![page, game_domain],
        )?;
        Ok(())
    }

    /// Mark sync as complete
    pub fn mark_sync_complete(&self, game_domain: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE catalog_sync_state
            SET completed = 1, last_sync = datetime('now'), last_error = NULL
            WHERE game_domain = ?1
            "#,
            params![game_domain],
        )?;
        Ok(())
    }

    /// Update sync error
    pub fn update_sync_error(&self, game_domain: &str, error: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE catalog_sync_state SET last_error = ?1 WHERE game_domain = ?2",
            params![error, game_domain],
        )?;
        Ok(())
    }

    /// Upsert a page of catalog mods (insert or update)
    pub fn upsert_catalog_page(
        &self,
        game_domain: &str,
        mods: &[NexusCatalogRecord],
    ) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut inserted = 0i64;
        let mut updated = 0i64;

        let tx = conn.unchecked_transaction()?;

        for mod_record in mods {
            // Check if exists
            let exists: bool = tx.query_row(
                "SELECT COUNT(*) FROM nexus_catalog WHERE game_domain = ?1 AND mod_id = ?2",
                params![game_domain, mod_record.mod_id],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )?;

            if exists {
                tx.execute(
                    r#"
                    UPDATE nexus_catalog
                    SET name = ?3, summary = ?4, description = ?5, author = ?6,
                        updated_time = ?7, synced_at = datetime('now')
                    WHERE game_domain = ?1 AND mod_id = ?2
                    "#,
                    params![
                        game_domain,
                        mod_record.mod_id,
                        mod_record.name,
                        mod_record.summary,
                        mod_record.description,
                        mod_record.author,
                        mod_record.updated_time,
                    ],
                )?;
                updated += 1;
            } else {
                tx.execute(
                    r#"
                    INSERT INTO nexus_catalog
                    (game_domain, mod_id, name, summary, description, author, updated_time, synced_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
                    "#,
                    params![
                        game_domain,
                        mod_record.mod_id,
                        mod_record.name,
                        mod_record.summary,
                        mod_record.description,
                        mod_record.author,
                        mod_record.updated_time,
                    ],
                )?;
                inserted += 1;
            }
        }

        tx.commit()?;
        Ok((inserted, updated))
    }

    /// Count catalog mods for a game domain
    pub fn count_catalog_mods(&self, game_domain: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM nexus_catalog WHERE game_domain = ?1",
            params![game_domain],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Search the local catalog for mods matching a query string
    /// Searches across name, summary, and description fields with token-based matching
    pub fn search_catalog(
        &self,
        game_domain: &str,
        query: &str,
        limit: i32,
    ) -> Result<Vec<NexusCatalogRecord>> {
        let conn = self.conn.lock().unwrap();

        // Normalize query: remove punctuation, split into tokens
        let query_normalized = query
            .to_lowercase()
            .replace('+', " ")
            .replace('-', " ")
            .replace('_', " ")
            .replace(':', " ");

        let tokens: Vec<&str> = query_normalized
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
            .filter(|t| t.len() >= 2) // Ignore single characters
            .collect();

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        // Build flexible query that searches name, summary, and description
        // Use multiple search strategies for better recall:
        // 1. Exact name match (highest priority)
        // 2. Name contains all tokens
        // 3. Name contains any token
        // 4. Summary or description contains tokens
        let mut sql = String::from(
            "SELECT DISTINCT game_domain, mod_id, name, summary, description, author, updated_time, synced_at, \
             CASE \
               WHEN LOWER(name) = ?2 THEN 1000 \
               WHEN LOWER(REPLACE(REPLACE(REPLACE(name, '-', ' '), '_', ' '), ':', ' ')) = ?2 THEN 900 "
        );

        // Add scoring for each token match in name
        for (i, _) in tokens.iter().enumerate() {
            sql.push_str(&format!(
                "WHEN LOWER(name) LIKE '%' || ?{} || '%' THEN {} ",
                i + 3,
                800_usize.saturating_sub(i * 50)
            ));
        }

        // Add scoring for summary/description matches
        for (i, _) in tokens.iter().enumerate() {
            sql.push_str(&format!(
                "WHEN LOWER(summary) LIKE '%' || ?{} || '%' OR LOWER(description) LIKE '%' || ?{} || '%' THEN {} ",
                i + 3,
                i + 3,
                400_usize.saturating_sub(i * 50)
            ));
        }

        sql.push_str(
            "ELSE 0 \
             END as relevance \
             FROM nexus_catalog \
             WHERE game_domain = ?1 AND (",
        );

        // Build WHERE clause: match any token in name, summary, or description
        let mut conditions = Vec::new();
        for (i, _) in tokens.iter().enumerate() {
            conditions.push(format!(
                "(LOWER(name) LIKE '%' || ?{} || '%' OR \
                  LOWER(summary) LIKE '%' || ?{} || '%' OR \
                  LOWER(description) LIKE '%' || ?{} || '%')",
                i + 3,
                i + 3,
                i + 3
            ));
        }
        sql.push_str(&conditions.join(" OR "));

        let limit_param_idx = tokens.len() + 3;
        sql.push_str(&format!(
            ") \
             ORDER BY relevance DESC, updated_time DESC \
             LIMIT ?{}",
            limit_param_idx
        ));

        let mut stmt = conn.prepare(&sql)?;

        // Build params: ?1=game_domain, ?2=query_normalized, ?3..=tokens, last=limit
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(game_domain.to_string()));
        params_vec.push(Box::new(query_normalized.clone()));

        for token in &tokens {
            params_vec.push(Box::new(token.to_string()));
        }
        params_vec.push(Box::new(limit));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(&param_refs[..], |row| NexusCatalogRecord::from_row(row))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }

        Ok(results)
    }

    /// Get a single catalog mod by exact game domain + Nexus mod ID.
    pub fn get_catalog_mod_by_id(
        &self,
        game_domain: &str,
        mod_id: i64,
    ) -> Result<Option<NexusCatalogRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            r#"
            SELECT game_domain, mod_id, name, summary, description, author, updated_time, synced_at
            FROM nexus_catalog
            WHERE game_domain = ?1 AND mod_id = ?2
            "#,
            params![game_domain, mod_id],
            |row| NexusCatalogRecord::from_row(row),
        )
        .optional()
        .context("Failed to query catalog mod by id")
    }

    // ========== Library Check Operations ==========

    /// Batch lookup of installed mods by nexus_mod_id, chunked to avoid SQL limits
    pub fn find_mods_by_nexus_ids(
        &self,
        game_id: &str,
        nexus_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, ModRecord>> {
        use std::collections::HashMap;

        let conn = self.conn.lock().unwrap();
        let mut result = HashMap::new();

        // Process in chunks of 500 to stay within SQLite variable limits
        for chunk in nexus_ids.chunks(500) {
            let placeholders: Vec<String> = chunk
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 2))
                .collect();
            let query = format!(
                "SELECT * FROM mods WHERE game_id = ?1 AND nexus_mod_id IN ({})",
                placeholders.join(", ")
            );

            let mut stmt = conn.prepare(&query)?;

            // Build params: game_id first, then all nexus IDs
            let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            params_vec.push(Box::new(game_id.to_string()));
            for id in chunk {
                params_vec.push(Box::new(*id));
            }

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), |row| ModRecord::from_row(row))?;

            for row in rows {
                let record = row?;
                if let Some(nexus_id) = record.nexus_mod_id {
                    result.insert(nexus_id, record);
                }
            }
        }

        Ok(result)
    }

    /// Find a mod by case-insensitive exact name match
    pub fn find_mod_by_name(&self, game_id: &str, name: &str) -> Result<Option<ModRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM mods WHERE game_id = ?1 AND name = ?2 COLLATE NOCASE",
            params![game_id, name],
            |row| ModRecord::from_row(row),
        )
        .optional()
        .context("Failed to query mod by name")
    }

    /// Migrate database schema for indexed plugin -> mod mappings.
    fn migrate_mod_plugin_index(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let migration_name = "mod_plugin_index_v1";

        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE migration_name = ?1",
                params![migration_name],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if already_applied {
            return Ok(());
        }

        tracing::info!("Applying mod plugin index migration");

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS mod_plugins (
                id INTEGER PRIMARY KEY,
                mod_id INTEGER NOT NULL,
                game_id TEXT NOT NULL,
                plugin_name TEXT NOT NULL,
                plugin_name_lower TEXT NOT NULL,
                FOREIGN KEY (mod_id) REFERENCES mods(id) ON DELETE CASCADE,
                UNIQUE(mod_id, plugin_name_lower)
            );

            CREATE INDEX IF NOT EXISTS idx_mod_plugins_lookup
                ON mod_plugins(game_id, plugin_name_lower);
            CREATE INDEX IF NOT EXISTS idx_mod_plugins_mod
                ON mod_plugins(mod_id);
            "#,
        )?;

        // Backfill plugin index from existing mod_files so prior installs are searchable.
        let mut select_stmt = conn.prepare(
            r#"
            SELECT m.id, m.game_id, f.relative_path
            FROM mods m
            JOIN mod_files f ON f.mod_id = m.id
            "#,
        )?;
        let rows = select_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut insert_stmt = conn.prepare(
            "INSERT OR IGNORE INTO mod_plugins (mod_id, game_id, plugin_name, plugin_name_lower)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for row in rows {
            let (mod_id, game_id, relative_path) = row?;
            if let Some(name) = Path::new(&relative_path)
                .file_name()
                .and_then(|n| n.to_str())
            {
                let lower = name.to_lowercase();
                if lower.ends_with(".esp") || lower.ends_with(".esm") || lower.ends_with(".esl") {
                    insert_stmt.execute(params![mod_id, game_id, name, lower])?;
                }
            }
        }
        drop(insert_stmt);
        drop(select_stmt);

        conn.execute(
            "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
            params![migration_name],
        )?;

        tracing::info!("Mod plugin index migration completed successfully");
        Ok(())
    }

    // ========== Modlist Operations ==========

    /// Migrate database schema for persistent modlists
    fn migrate_modlists(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        let migration_name = "modlists_v1";
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE migration_name = ?1",
                params![migration_name],
                |row| {
                    let count: i64 = row.get(0)?;
                    Ok(count > 0)
                },
            )
            .unwrap_or(false);

        if already_applied {
            return Ok(());
        }

        tracing::info!("Applying modlists migration");

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS modlists (
                id INTEGER PRIMARY KEY,
                game_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                source_file TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(game_id, name)
            );

            CREATE TABLE IF NOT EXISTS modlist_entries (
                id INTEGER PRIMARY KEY,
                modlist_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                nexus_mod_id INTEGER,
                plugin_name TEXT,
                match_confidence REAL,
                position INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1,
                author TEXT,
                version TEXT,
                FOREIGN KEY (modlist_id) REFERENCES modlists(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_modlists_game ON modlists(game_id);
            CREATE INDEX IF NOT EXISTS idx_modlist_entries_modlist ON modlist_entries(modlist_id);
            "#,
        )?;

        conn.execute(
            "INSERT INTO schema_version (migration_name, applied_at) VALUES (?1, datetime('now'))",
            params![migration_name],
        )?;

        tracing::info!("Modlists migration completed successfully");
        Ok(())
    }

    /// Create a new modlist
    pub fn create_modlist(
        &self,
        game_id: &str,
        name: &str,
        description: Option<&str>,
        source_file: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO modlists (game_id, name, description, source_file, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))
            "#,
            params![game_id, name, description, source_file],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Add modlist entries in a batch (transaction-wrapped)
    pub fn add_modlist_entries_batch(
        &self,
        modlist_id: i64,
        entries: &[ModlistEntryRecord],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let mut stmt = tx.prepare(
            r#"
            INSERT INTO modlist_entries (modlist_id, name, nexus_mod_id, plugin_name, match_confidence, position, enabled, author, version)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )?;

        for entry in entries {
            stmt.execute(params![
                modlist_id,
                entry.name,
                entry.nexus_mod_id,
                entry.plugin_name,
                entry.match_confidence,
                entry.position,
                entry.enabled as i32,
                entry.author,
                entry.version,
            ])?;
        }

        drop(stmt);
        tx.commit()?;
        Ok(())
    }

    /// Create or replace a modlist by (game_id, name), replacing all entries when it already exists.
    pub fn upsert_modlist_with_entries(
        &self,
        game_id: &str,
        name: &str,
        description: Option<&str>,
        source_file: Option<&str>,
        entries: &[ModlistEntryRecord],
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM modlists WHERE game_id = ?1 AND name = ?2",
                params![game_id, name],
                |row| row.get(0),
            )
            .optional()?;

        let modlist_id = if let Some(id) = existing_id {
            tx.execute(
                "UPDATE modlists
                 SET description = ?1,
                     source_file = ?2,
                     updated_at = datetime('now')
                 WHERE id = ?3",
                params![description, source_file, id],
            )?;
            tx.execute(
                "DELETE FROM modlist_entries WHERE modlist_id = ?1",
                params![id],
            )?;
            id
        } else {
            tx.execute(
                r#"
                INSERT INTO modlists (game_id, name, description, source_file, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))
                "#,
                params![game_id, name, description, source_file],
            )?;
            tx.last_insert_rowid()
        };

        let mut stmt = tx.prepare(
            r#"
            INSERT INTO modlist_entries (modlist_id, name, nexus_mod_id, plugin_name, match_confidence, position, enabled, author, version)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )?;

        for entry in entries {
            stmt.execute(params![
                modlist_id,
                &entry.name,
                entry.nexus_mod_id,
                &entry.plugin_name,
                entry.match_confidence,
                entry.position,
                entry.enabled as i32,
                &entry.author,
                &entry.version,
            ])?;
        }
        drop(stmt);

        tx.commit()?;
        Ok(modlist_id)
    }

    /// Get all modlists for a game
    pub fn get_modlists_for_game(&self, game_id: &str) -> Result<Vec<ModlistRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM modlists WHERE game_id = ?1 ORDER BY updated_at DESC")?;

        let lists = stmt
            .query_map(params![game_id], |row| ModlistRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(lists)
    }

    /// Get entries for a modlist
    pub fn get_modlist_entries(&self, modlist_id: i64) -> Result<Vec<ModlistEntryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM modlist_entries WHERE modlist_id = ?1 ORDER BY position ASC")?;

        let entries = stmt
            .query_map(params![modlist_id], |row| ModlistEntryRecord::from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Get entry count for a modlist
    pub fn get_modlist_entry_count(&self, modlist_id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM modlist_entries WHERE modlist_id = ?1",
            params![modlist_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Update modlist entry enabled status
    pub fn update_modlist_entry_enabled(&self, entry_id: i64, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE modlist_entries SET enabled = ?1 WHERE id = ?2",
            params![enabled as i32, entry_id],
        )?;
        Ok(())
    }

    /// Update modlist entry position
    pub fn update_modlist_entry_position(&self, entry_id: i64, position: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE modlist_entries SET position = ?1 WHERE id = ?2",
            params![position, entry_id],
        )?;
        Ok(())
    }

    /// Delete a modlist and all its entries (CASCADE)
    pub fn delete_modlist(&self, modlist_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // Manually delete entries first since SQLite CASCADE requires foreign_keys pragma
        conn.execute(
            "DELETE FROM modlist_entries WHERE modlist_id = ?1",
            params![modlist_id],
        )?;
        conn.execute("DELETE FROM modlists WHERE id = ?1", params![modlist_id])?;
        Ok(())
    }

    /// Delete a single modlist entry
    pub fn delete_modlist_entry(&self, entry_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM modlist_entries WHERE id = ?1",
            params![entry_id],
        )?;
        Ok(())
    }

    /// Rename a modlist
    pub fn rename_modlist(&self, modlist_id: i64, new_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE modlists SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![new_name, modlist_id],
        )?;
        Ok(())
    }

    /// List catalog mods with pagination (ordered by updated_time DESC)
    pub fn list_catalog_mods(
        &self,
        game_domain: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<NexusCatalogRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT game_domain, mod_id, name, summary, description, author, updated_time, synced_at \
             FROM nexus_catalog WHERE game_domain = ?1 \
             ORDER BY updated_time DESC \
             LIMIT ?2 OFFSET ?3",
        )?;

        let mods = stmt
            .query_map(params![game_domain, limit, offset], |row| {
                NexusCatalogRecord::from_row(row)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(mods)
    }
}
