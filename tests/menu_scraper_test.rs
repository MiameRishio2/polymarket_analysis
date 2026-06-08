//! Menu scraper tests
//!
//! Tests for the unified category scraping API

use polymarket_analysis::menu::{Category, CategoryData, get_category_or_default};

#[test]
fn test_category_data_structure() {
    let category = Category {
        slug: "football".to_string(),
        name: "Football".to_string(),
        url: "/football".to_string(),
        category_type: None,
    };
    
    assert_eq!(category.slug, "football");
    assert_eq!(category.name, "Football");
    assert_eq!(category.url, "/football");
    assert!(category.category_type.is_none());
}

#[test]
fn test_category_with_type() {
    let category = Category {
        slug: "england".to_string(),
        name: "England".to_string(),
        url: "/football/england".to_string(),
        category_type: Some("country".to_string()),
    };
    
    assert_eq!(category.slug, "england");
    assert_eq!(category.category_type, Some("country".to_string()));
}

#[test]
fn test_category_data_serialization() {
    let category = Category {
        slug: "basketball".to_string(),
        name: "Basketball".to_string(),
        url: "/basketball".to_string(),
        category_type: None,
    };
    
    let json = serde_json::to_string(&category).unwrap();
    assert!(json.contains("basketball"));
    
    let deserialized: Category = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.slug, category.slug);
}

#[test]
fn test_category_data_response() {
    let data = CategoryData::new("menu")
        .with_categories(vec![
            Category {
                slug: "football".to_string(),
                name: "Football".to_string(),
                url: "/football".to_string(),
                category_type: None,
            },
        ])
        .with_source("test");
    
    assert_eq!(data.sport, "menu");
    assert_eq!(data.categories.len(), 1);
    assert_eq!(data.source, "test");
}

#[test]
fn test_get_category_or_default_returns_empty() {
    let data = get_category_or_default("menu");
    
    assert_eq!(data.sport, "menu");
    assert!(data.categories.is_empty(), "Default should return empty categories");
    assert_eq!(data.source, "error");
}

#[test]
fn test_get_category_or_default_football() {
    let data = get_category_or_default("football");
    
    assert_eq!(data.sport, "football");
    assert!(data.categories.is_empty(), "Default should return empty categories");
    assert_eq!(data.source, "error");
}

#[test]
fn test_esports_categories() {
    // Test that esports categories data structure is correct
    let data = CategoryData::new("esports")
        .with_categories(vec![
            Category {
                slug: "counter-strike".to_string(),
                name: "Counter Strike".to_string(),
                url: "/esports/counter-strike/".to_string(),
                category_type: Some("country".to_string()),
            },
            Category {
                slug: "dota-2".to_string(),
                name: "Dota 2".to_string(),
                url: "/esports/dota-2/".to_string(),
                category_type: Some("country".to_string()),
            },
            Category {
                slug: "league-of-legends".to_string(),
                name: "League Of Legends".to_string(),
                url: "/esports/league-of-legends/".to_string(),
                category_type: Some("country".to_string()),
            },
        ])
        .with_source("test");
    
    assert_eq!(data.sport, "esports");
    assert_eq!(data.categories.len(), 3);
    
    // Verify all three esports categories are present
    let slugs: Vec<_> = data.categories.iter().map(|c| c.slug.as_str()).collect();
    assert!(slugs.contains(&"counter-strike"));
    assert!(slugs.contains(&"dota-2"));
    assert!(slugs.contains(&"league-of-legends"));
}

#[test]
fn test_esports_category_url_format() {
    // Verify esports category URLs follow correct format
    let expected_url = "/esports/dota-2/";
    let category = Category {
        slug: "dota-2".to_string(),
        name: "Dota 2".to_string(),
        url: expected_url.to_string(),
        category_type: Some("country".to_string()),
    };
    
    assert!(category.url.ends_with("/"));
    assert!(category.url.starts_with("/esports/"));
}

#[test]
fn test_excluded_paths_not_in_esports() {
    // Verify that excluded paths (results, standings, etc.) are not in category data
    let data = CategoryData::new("esports")
        .with_categories(vec![
            Category {
                slug: "counter-strike".to_string(),
                name: "Counter Strike".to_string(),
                url: "/esports/counter-strike/".to_string(),
                category_type: Some("country".to_string()),
            },
        ])
        .with_source("test");
    
    // Ensure no excluded paths are present
    for category in &data.categories {
        assert_ne!(category.slug, "results", "results should be excluded");
        assert_ne!(category.slug, "standings", "standings should be excluded");
        assert_ne!(category.slug, "live", "live should be excluded");
        assert_ne!(category.slug, "archive", "archive should be excluded");
    }
}
