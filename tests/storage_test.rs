//! Storage module tests
//!
//! Tests unified SQLite storage functionality

use polymarket_analysis::menu::{Category, CategoryData, NewScheduledMatch, Storage};
use rusqlite::{params, Connection};
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
        refreshed_at: "2026-06-06T12:30:00Z".to_string(),
        source: "https://www.oddsportal.com/".to_string(),
    };

    storage.save("menu", &data).expect("Should save data");

    let loaded = storage.load("menu").expect("Should load data");
    assert!(loaded.is_some());

    let loaded = loaded.unwrap();
    assert_eq!(loaded.categories.len(), 2);
    assert_eq!(loaded.source, "https://www.oddsportal.com/");
    assert_eq!(loaded.refreshed_at, "2026-06-06T12:30:00Z");
    assert_eq!(loaded.categories[0].slug, "football");
    assert_eq!(loaded.categories[1].name, "Basketball");
}

#[test]
fn test_existing_cache_rows_default_to_epoch_refresh_time() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("legacy.db");

    {
        let conn = Connection::open(&db_path).expect("Should open legacy db");
        conn.execute(
            "CREATE TABLE category_cache (
                sport TEXT PRIMARY KEY,
                categories_json TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .expect("Should create legacy table");
        conn.execute(
            "INSERT INTO category_cache
             (sport, categories_json, last_updated, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![
                "menu",
                r#"[{"slug":"football","name":"Football","url":"/football","category_type":null}]"#,
                "2026-06-06T12:00:00Z",
                "legacy"
            ],
        )
        .expect("Should insert legacy row");
    }

    let storage = Storage::new(&db_path).expect("Should migrate storage");
    let loaded = storage
        .load("menu")
        .expect("Should load migrated row")
        .expect("Should find migrated row");

    assert_eq!(loaded.refreshed_at, "1970-01-01T00:00:00Z");
}

#[test]
fn test_raw_json_round_trips_refresh_time() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let storage = Storage::new(&db_path).expect("Should create storage");

    storage
        .save_raw_json(
            "events_football_world_world-championship-2026",
            r#"[{"matchup":"Mexico VS South Africa"}]"#,
            "2026-06-09T00:00:00Z",
            "cache-test",
            "2026-06-09T01:00:00Z",
        )
        .expect("Should save raw JSON");

    let (_, _, _, refreshed_at) = storage
        .load_raw_json("events_football_world_world-championship-2026")
        .expect("Should load raw JSON")
        .expect("Raw JSON should exist");

    assert_eq!(refreshed_at, "2026-06-09T01:00:00Z");
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
        refreshed_at: "2026-06-06T12:00:00Z".to_string(),
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
        refreshed_at: "2026-06-06T13:00:00Z".to_string(),
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
        refreshed_at: "2026-06-06T12:00:00Z".to_string(),
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
        refreshed_at: "2026-06-06T12:00:00Z".to_string(),
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
        refreshed_at: "2026-06-06T12:00:00Z".to_string(),
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
        refreshed_at: "2026-06-06T12:00:00Z".to_string(),
        source: "default".to_string(),
    };
    storage
        .save("football", &football_data)
        .expect("Should save football");

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

#[test]
fn test_scheduler_upsert_defaults_monitoring_to_false() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let storage = Storage::new(&db_path).expect("Should create storage");

    let input = scheduler_match("Mexico VS South Africa", "2026-06-18T03:00:00Z");
    let saved = storage
        .upsert_scheduled_match(&input)
        .expect("Should upsert scheduler match");

    assert!(!saved.monitoring_started);
    assert_eq!(saved.matchup, "Mexico VS South Africa");
    assert_eq!(saved.home_team.as_deref(), Some("Mexico"));
    assert_eq!(saved.away_team.as_deref(), Some("South Africa"));

    let matches = storage
        .list_scheduled_matches()
        .expect("Should list scheduler matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, saved.id);
}

#[test]
fn test_scheduler_duplicate_upsert_updates_single_row() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let storage = Storage::new(&db_path).expect("Should create storage");

    let mut input = scheduler_match("Mexico VS South Africa", "2026-06-18T03:00:00Z");
    let first = storage
        .upsert_scheduled_match(&input)
        .expect("Should insert scheduler match");

    input.polymarket_url = Some("https://polymarket.com/updated".to_string());
    input.monitoring_started = Some(true);
    let second = storage
        .upsert_scheduled_match(&input)
        .expect("Should update scheduler match");

    let matches = storage
        .list_scheduled_matches()
        .expect("Should list scheduler matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(first.id, second.id);
    assert!(second.monitoring_started);
    assert_eq!(
        matches[0].polymarket_url.as_deref(),
        Some("https://polymarket.com/updated")
    );
}

#[test]
fn test_scheduler_monitoring_toggle_and_delete() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let storage = Storage::new(&db_path).expect("Should create storage");

    let saved = storage
        .upsert_scheduled_match(&scheduler_match(
            "Mexico VS South Africa",
            "2026-06-18T03:00:00Z",
        ))
        .expect("Should insert scheduler match");

    let updated = storage
        .set_scheduled_match_monitoring(&saved.id, true)
        .expect("Should toggle monitoring")
        .expect("Scheduler match should exist");
    assert!(updated.monitoring_started);

    assert!(storage
        .delete_scheduled_match(&saved.id)
        .expect("Should delete scheduler match"));
    assert!(storage
        .list_scheduled_matches()
        .expect("Should list scheduler matches")
        .is_empty());
}

#[test]
fn test_scheduler_list_ordering() {
    let temp_dir = TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let storage = Storage::new(&db_path).expect("Should create storage");

    let mut active = scheduler_match("Alpha VS Beta", "2026-06-18T04:00:00Z");
    active.monitoring_started = Some(true);
    storage
        .upsert_scheduled_match(&active)
        .expect("Should insert active match");
    storage
        .upsert_scheduled_match(&scheduler_match("Earlier VS Match", "2026-06-18T01:00:00Z"))
        .expect("Should insert earlier match");
    storage
        .upsert_scheduled_match(&scheduler_match("Later VS Match", "2026-06-18T05:00:00Z"))
        .expect("Should insert later match");

    let matches = storage
        .list_scheduled_matches()
        .expect("Should list scheduler matches");
    let names = matches
        .iter()
        .map(|item| item.matchup.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["Alpha VS Beta", "Earlier VS Match", "Later VS Match"]
    );
}

fn scheduler_match(matchup: &str, start_time: &str) -> NewScheduledMatch {
    let teams = matchup.split(" VS ").collect::<Vec<_>>();
    NewScheduledMatch {
        matchup: matchup.to_string(),
        home_team: teams.first().map(|value| (*value).to_string()),
        away_team: teams.get(1).map(|value| (*value).to_string()),
        start_time: Some(start_time.to_string()),
        oddsportal_url: Some(format!(
            "https://www.oddsportal.com/football/h2h/{}/",
            matchup.to_lowercase().replace(' ', "-")
        )),
        polymarket_url: Some("https://polymarket.com/event".to_string()),
        source_page: Some("/menu/football/world/world-championship-2026/".to_string()),
        monitoring_started: None,
    }
}
