//! 存储模块
//!
//! 使用 SQLite 存储菜单数据

use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

use crate::menu_scraper::{MenuData, SportCategory};

/// 数据库存储错误类型
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Database not initialized")]
    NotInitialized,
}

/// 存储模块管理器
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    /// 创建存储实例
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = Connection::open(db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        
        storage.init_schema()?;
        tracing::info!("Storage initialized at {:?}", db_path);
        
        Ok(storage)
    }
    
    /// 初始化数据库表结构
    fn init_schema(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS menu_cache (
                id INTEGER PRIMARY KEY,
                sports_json TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_menu_updated ON menu_cache(updated_at DESC)",
            [],
        )?;
        Ok(())
    }
    
    /// 保存菜单数据
    pub fn save_menu(&self, menu: &MenuData) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let sports_json = serde_json::to_string(&menu.sports)?;
        
        // 先清空旧数据
        conn.execute("DELETE FROM menu_cache", [])?;
        
        // 插入新数据
        conn.execute(
            "INSERT INTO menu_cache (sports_json, last_updated, source, updated_at) VALUES (?1, ?2, ?3, datetime('now'))",
            params![sports_json, menu.last_updated, menu.source],
        )?;
        
        tracing::info!("Menu saved to storage, {} sports", menu.sports.len());
        Ok(())
    }
    
    /// 加载菜单数据
    pub fn load_menu(&self) -> Result<Option<MenuData>, StorageError> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT sports_json, last_updated, source FROM menu_cache ORDER BY updated_at DESC LIMIT 1"
        )?;
        
        let result = stmt.query_row([], |row| {
            let sports_json: String = row.get(0)?;
            let last_updated: String = row.get(1)?;
            let source: String = row.get(2)?;
            
            Ok((sports_json, last_updated, source))
        });
        
        match result {
            Ok((sports_json, last_updated, source)) => {
                let sports: Vec<SportCategory> = serde_json::from_str(&sports_json)?;
                Ok(Some(MenuData {
                    sports,
                    last_updated,
                    source,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::SqliteError(e)),
        }
    }
    
    /// 检查是否有缓存数据
    pub fn has_cache(&self) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM menu_cache",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
    
    /// 获取缓存更新时间
    pub fn get_cache_updated_at(&self) -> Result<Option<String>, StorageError> {
        let conn = self.conn.lock().unwrap();
        
        let result: Result<String, _> = conn.query_row(
            "SELECT updated_at FROM menu_cache ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        );
        
        match result {
            Ok(time) => Ok(Some(time)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::SqliteError(e)),
        }
    }
    
    /// 清除缓存
    pub fn clear_cache(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM menu_cache", [])?;
        tracing::info!("Menu cache cleared");
        Ok(())
    }
}

// 测试用的存储管理器（用于测试）
#[cfg(test)]
impl Storage {
    /// 使用内存数据库创建测试存储
    pub fn new_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_schema()?;
        Ok(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_storage_new_in_memory() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        assert!(!storage.has_cache().unwrap());
    }
    
    #[test]
    fn test_save_and_load_menu() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        
        let menu = MenuData {
            sports: vec![
                SportCategory {
                    slug: "football".to_string(),
                    name: "FOOTBALL".to_string(),
                    url: "/football".to_string(),
                },
                SportCategory {
                    slug: "basketball".to_string(),
                    name: "BASKETBALL".to_string(),
                    url: "/basketball".to_string(),
                },
            ],
            last_updated: "2026-06-06T12:00:00Z".to_string(),
            source: "https://www.oddsportal.com/".to_string(),
        };
        
        storage.save_menu(&menu).expect("Should save menu");
        
        let loaded = storage.load_menu().expect("Should load menu");
        assert!(loaded.is_some());
        
        let loaded = loaded.unwrap();
        assert_eq!(loaded.sports.len(), 2);
        assert_eq!(loaded.source, "https://www.oddsportal.com/");
        assert_eq!(loaded.sports[0].slug, "football");
    }
    
    #[test]
    fn test_load_empty_storage() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        
        let loaded = storage.load_menu().expect("Should load menu");
        assert!(loaded.is_none());
    }
    
    #[test]
    fn test_has_cache() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        
        assert!(!storage.has_cache().unwrap());
        
        let menu = MenuData {
            sports: vec![SportCategory {
                slug: "football".to_string(),
                name: "FOOTBALL".to_string(),
                url: "/football".to_string(),
            }],
            last_updated: "2026-06-06T12:00:00Z".to_string(),
            source: "default".to_string(),
        };
        
        storage.save_menu(&menu).expect("Should save menu");
        assert!(storage.has_cache().unwrap());
    }
    
    #[test]
    fn test_clear_cache() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        
        let menu = MenuData {
            sports: vec![SportCategory {
                slug: "football".to_string(),
                name: "FOOTBALL".to_string(),
                url: "/football".to_string(),
            }],
            last_updated: "2026-06-06T12:00:00Z".to_string(),
            source: "default".to_string(),
        };
        
        storage.save_menu(&menu).expect("Should save menu");
        assert!(storage.has_cache().unwrap());
        
        storage.clear_cache().expect("Should clear cache");
        assert!(!storage.has_cache().unwrap());
    }
    
    #[test]
    fn test_overwrite_menu() {
        let storage = Storage::new_in_memory().expect("Should create storage");
        
        let menu1 = MenuData {
            sports: vec![SportCategory {
                slug: "football".to_string(),
                name: "FOOTBALL".to_string(),
                url: "/football".to_string(),
            }],
            last_updated: "2026-06-06T12:00:00Z".to_string(),
            source: "source1".to_string(),
        };
        
        storage.save_menu(&menu1).expect("Should save menu1");
        
        let menu2 = MenuData {
            sports: vec![
                SportCategory {
                    slug: "basketball".to_string(),
                    name: "BASKETBALL".to_string(),
                    url: "/basketball".to_string(),
                },
                SportCategory {
                    slug: "tennis".to_string(),
                    name: "TENNIS".to_string(),
                    url: "/tennis".to_string(),
                },
            ],
            last_updated: "2026-06-06T13:00:00Z".to_string(),
            source: "source2".to_string(),
        };
        
        storage.save_menu(&menu2).expect("Should save menu2");
        
        let loaded = storage.load_menu().expect("Should load menu");
        let loaded = loaded.unwrap();
        
        assert_eq!(loaded.sports.len(), 2);
        assert_eq!(loaded.source, "source2");
    }
}
