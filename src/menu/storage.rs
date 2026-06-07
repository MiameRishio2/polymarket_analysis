//! Unified storage for menu and category data

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use super::models::CategoryData;

/// Storage error type
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Unified storage manager for all category data
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    /// Create storage instance
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let storage = Self { conn: Mutex::new(conn) };
        storage.init_schema()?;
        tracing::info!("Storage initialized at {:?}", db_path);
        Ok(storage)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS category_cache (
                sport TEXT PRIMARY KEY,
                categories_json TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_updated ON category_cache(updated_at DESC)",
            [],
        )?;
        Ok(())
    }

    /// Save category data for a sport
    pub fn save(&self, sport: &str, data: &CategoryData) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let categories_json = serde_json::to_string(&data.categories)?;
        conn.execute(
            "INSERT OR REPLACE INTO category_cache 
             (sport, categories_json, last_updated, source, updated_at) 
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![sport, categories_json, data.last_updated, data.source],
        )?;
        tracing::info!("Saved {} categories for '{}'", data.categories.len(), sport);
        Ok(())
    }

    /// Load category data for a sport
    pub fn load(&self, sport: &str) -> Result<Option<CategoryData>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT categories_json, last_updated, source FROM category_cache WHERE sport = ?1"
        )?;
        let result = stmt.query_row(params![sport], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        });
        match result {
            Ok((categories_json, last_updated, source)) => {
                let categories: Vec<super::models::Category> = serde_json::from_str(&categories_json)?;
                Ok(Some(CategoryData {
                    sport: sport.to_string(),
                    categories,
                    last_updated,
                    source,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Sqlite(e)),
        }
    }

    /// Load all sports with cached data
    pub fn get_all_sports(&self) -> Result<Vec<String>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT sport FROM category_cache ORDER BY updated_at DESC")?;
        let sports = stmt.query_map([], |row| row.get(0))?;
        Ok(sports.filter_map(|r| r.ok()).collect())
    }

    /// Check if cache has data for a sport
    pub fn has(&self, sport: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM category_cache WHERE sport = ?1",
            params![sport],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Clear cache for a sport
    pub fn clear(&self, sport: &str) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM category_cache WHERE sport = ?1", params![sport])?;
        Ok(())
    }

    /// Clear all cache
    pub fn clear_all(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM category_cache", [])?;
        Ok(())
    }
}

// Backward compatibility
pub type MenuStorage = Storage;
