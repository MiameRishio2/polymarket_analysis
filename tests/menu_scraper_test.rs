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
fn test_get_category_or_default_menu() {
    let data = get_category_or_default("menu");
    
    assert_eq!(data.sport, "menu");
    assert!(!data.categories.is_empty(), "Default menu should have categories");
    assert!(data.last_updated.len() > 0, "Should have last_updated timestamp");
    assert_eq!(data.source, "default");
}

#[test]
fn test_get_category_or_default_football() {
    let data = get_category_or_default("football");
    
    assert_eq!(data.sport, "football");
    assert!(!data.categories.is_empty(), "Default football should have categories");
    assert_eq!(data.source, "default");
}

#[test]
fn test_default_menu_contains_football() {
    let data = get_category_or_default("menu");
    
    let football = data.categories.iter().find(|c| c.slug == "football");
    assert!(football.is_some(), "Default menu should include football");
    assert_eq!(football.unwrap().name, "Football");
}

#[test]
fn test_default_football_contains_countries() {
    let data = get_category_or_default("football");
    
    assert!(data.categories.iter().any(|c| c.slug == "england"), "Should contain england");
    assert!(data.categories.iter().any(|c| c.category_type.as_deref() == Some("country")), "Should have country types");
}
