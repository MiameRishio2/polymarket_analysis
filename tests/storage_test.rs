//! Storage module tests
//!
//! Tests unified SQLite storage functionality

use polymarket_analysis::menu::{Storage, CategoryData, Category};
use tempfile::TempDir;

#[test]
fn test_storage_new() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    assert!(!storage.has("menu").expect("Should check cache"));
}

#[test]
fn test_save_and_load_category() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    let data = CategoryData {
        sport: "menu".to_string(),
        categories: vec![
            Category {
                slug: "football".to_string(),
                name: "Football".to_string(),
                url: "/football".to_string(),
                category_type: None,
            },
            Category {
                slug: "basketball".to_string(),
                name: "Basketball".to_string(),
                url: "/basketball".to_string(),
                category_type: None,
            },
        ],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "https://www.oddsportal.com/".to_string(),
    };
    
    storage.save("menu", &data).expect("Should save data");
    
    let loaded = storage.load("menu").expect("Should load data");
    assert!(loaded.is_some());
    
    let loaded = loaded.unwrap();
    assert_eq!(loaded.categories.len(), 2);
    assert_eq!(loaded.source, "https://www.oddsportal.com/");
    assert_eq!(loaded.categories[0].slug, "football");
    assert_eq!(loaded.categories[1].name, "Basketball");
}

#[test]
fn test_load_empty_storage() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    let loaded = storage.load("nonexistent").expect("Should load data");
    assert!(loaded.is_none());
}

#[test]
fn test_overwrite_category() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    let data1 = CategoryData {
        sport: "menu".to_string(),
        categories: vec![Category {
            slug: "football".to_string(),
            name: "Football".to_string(),
            url: "/football".to_string(),
            category_type: None,
        }],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "source1".to_string(),
    };
    
    storage.save("menu", &data1).expect("Should save data1");
    
    let data2 = CategoryData {
        sport: "menu".to_string(),
        categories: vec![
            Category {
                slug: "basketball".to_string(),
                name: "Basketball".to_string(),
                url: "/basketball".to_string(),
                category_type: None,
            },
            Category {
                slug: "tennis".to_string(),
                name: "Tennis".to_string(),
                url: "/tennis".to_string(),
                category_type: None,
            },
        ],
        last_updated: "2026-06-06T13:00:00Z".to_string(),
        source: "source2".to_string(),
    };
    
    storage.save("menu", &data2).expect("Should save data2");
    
    let loaded = storage.load("menu").expect("Should load data");
    let loaded = loaded.unwrap();
    
    assert_eq!(loaded.categories.len(), 2);
    assert_eq!(loaded.source, "source2");
    assert_eq!(loaded.last_updated, "2026-06-06T13:00:00Z");
}

#[test]
fn test_has_category() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    assert!(!storage.has("menu").expect("Should check cache"));
    
    let data = CategoryData {
        sport: "menu".to_string(),
        categories: vec![Category {
            slug: "football".to_string(),
            name: "Football".to_string(),
            url: "/football".to_string(),
            category_type: None,
        }],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "default".to_string(),
    };
    
    storage.save("menu", &data).expect("Should save data");
    assert!(storage.has("menu").expect("Should check cache"));
}

#[test]
fn test_clear_category() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    let data = CategoryData {
        sport: "menu".to_string(),
        categories: vec![Category {
            slug: "football".to_string(),
            name: "Football".to_string(),
            url: "/football".to_string(),
            category_type: None,
        }],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "default".to_string(),
    };
    
    storage.save("menu", &data).expect("Should save data");
    assert!(storage.has("menu").expect("Should check cache"));
    
    storage.clear("menu").expect("Should clear data");
    assert!(!storage.has("menu").expect("Should check cache"));
}

#[test]
fn test_multiple_sports_storage() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    
    let storage = Storage::new(&db_path).expect("Should create storage");
    
    // Save menu
    let menu_data = CategoryData {
        sport: "menu".to_string(),
        categories: vec![Category {
            slug: "football".to_string(),
            name: "Football".to_string(),
            url: "/football".to_string(),
            category_type: None,
        }],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "default".to_string(),
    };
    storage.save("menu", &menu_data).expect("Should save menu");
    
    // Save football categories
    let football_data = CategoryData {
        sport: "football".to_string(),
        categories: vec![Category {
            slug: "england".to_string(),
            name: "England".to_string(),
            url: "/football/england".to_string(),
            category_type: Some("country".to_string()),
        }],
        last_updated: "2026-06-06T12:00:00Z".to_string(),
        source: "default".to_string(),
    };
    storage.save("football", &football_data).expect("Should save football");
    
    // Verify both exist
    assert!(storage.has("menu").expect("Should check menu"));
    assert!(storage.has("football").expect("Should check football"));
    
    // Load and verify
    let loaded_menu = storage.load("menu").expect("Should load menu");
    let loaded_football = storage.load("football").expect("Should load football");
    
    assert!(loaded_menu.is_some());
    assert!(loaded_football.is_some());
    assert_eq!(loaded_menu.unwrap().categories.len(), 1);
    assert_eq!(loaded_football.unwrap().categories.len(), 1);
}
