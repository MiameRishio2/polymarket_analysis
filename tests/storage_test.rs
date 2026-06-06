//! 存储模块测试
//!
//! 测试 SQLite 存储功能

use polymarket_analysis::storage::Storage;
use polymarket_analysis::menu_scraper::{MenuData, SportCategory};
use tempfile::TempDir;

#[test]
fn test_storage_new() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    assert!(!storage.has_cache().expect("Should check cache"));
}

#[test]
fn test_save_and_load_menu() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
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
    assert_eq!(loaded.sports[1].name, "BASKETBALL");
}

#[test]
fn test_load_empty_storage() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    let loaded = storage.load_menu().expect("Should load menu");
    assert!(loaded.is_none());
}

#[test]
fn test_overwrite_menu() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
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
    assert_eq!(loaded.last_updated, "2026-06-06T13:00:00Z");
}

#[test]
fn test_has_cache() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    assert!(!storage.has_cache().expect("Should check cache"));
    
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
    assert!(storage.has_cache().expect("Should check cache"));
}

#[test]
fn test_clear_cache() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
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
    assert!(storage.has_cache().expect("Should check cache"));
    
    storage.clear_cache().expect("Should clear cache");
    assert!(!storage.has_cache().expect("Should check cache"));
}
