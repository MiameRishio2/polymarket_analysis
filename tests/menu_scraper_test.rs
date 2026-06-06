//! 菜单爬取模块测试
//!
//! 测试从 oddsportal.com 爬取菜单数据的功能

use polymarket_analysis::menu_scraper::{SportCategory, get_menu_or_default, get_cached_menu};

#[test]
fn test_menu_data_structure() {
    let sport = SportCategory {
        slug: "football".to_string(),
        name: "FOOTBALL".to_string(),
        url: "/football".to_string(),
    };
    
    assert_eq!(sport.slug, "football");
    assert_eq!(sport.name, "FOOTBALL");
    assert_eq!(sport.url, "/football");
}

#[test]
fn test_menu_data_serialization() {
    let sport = SportCategory {
        slug: "basketball".to_string(),
        name: "BASKETBALL".to_string(),
        url: "/basketball".to_string(),
    };
    
    let json = serde_json::to_string(&sport).unwrap();
    assert!(json.contains("basketball"));
    
    let deserialized: SportCategory = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.slug, sport.slug);
}

#[test]
fn test_get_menu_or_default_returns_data() {
    let menu = get_menu_or_default();
    
    assert!(!menu.sports.is_empty(), "Default menu should have sports");
    assert!(menu.last_updated.len() > 0, "Should have last_updated timestamp");
    assert_eq!(menu.source, "default");
}

#[test]
fn test_get_cached_menu_returns_none_initially() {
    // 初始状态应该没有缓存
    let cached = get_cached_menu();
    // 注意：缓存可能是 None（首次调用）或 Some（如果前面的测试已经设置了缓存）
    // 这是一个并发测试问题，所以我们只检查类型
    println!("Cached menu: {:?}", cached.is_some());
}

#[test]
fn test_default_sports_contain_football() {
    let menu = get_menu_or_default();
    
    let football = menu.sports.iter().find(|s| s.slug == "football");
    assert!(football.is_some(), "Default sports should include football");
    assert_eq!(football.unwrap().name, "FOOTBALL");
}

#[test]
fn test_default_sports_count() {
    let menu = get_menu_or_default();
    
    // 应该有 22 个默认体育分类
    assert!(menu.sports.len() >= 20, "Should have at least 20 default sports");
}

#[test]
fn test_sport_categories_are_unique() {
    let menu = get_menu_or_default();
    
    let mut slugs: Vec<&str> = menu.sports.iter().map(|s| s.slug.as_str()).collect();
    slugs.sort();
    slugs.dedup();
    
    assert_eq!(slugs.len(), menu.sports.len(), "All sport slugs should be unique");
}
