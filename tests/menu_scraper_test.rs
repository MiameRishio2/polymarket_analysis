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
