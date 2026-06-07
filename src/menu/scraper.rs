//! Unified scraper for menu and category data

use crate::menu::models::{Category, CategoryData};
use std::time::{SystemTime, UNIX_EPOCH};

/// Scraper error type
#[derive(Debug, thiserror::Error)]
pub enum ScraperError {
    #[error("Error: {0}")]
    Generic(String),
}

/// Get current timestamp
fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

/// Get category data (returns default data)
pub fn get_category_or_default(sport: &str) -> CategoryData {
    if sport == "menu" {
        CategoryData {
            sport: "menu".to_string(),
            categories: default_sports(),
            last_updated: get_timestamp(),
            source: "default".to_string(),
        }
    } else {
        CategoryData {
            sport: sport.to_string(),
            categories: default_football_categories(),
            last_updated: get_timestamp(),
            source: "default".to_string(),
        }
    }
}

/// Refresh category data (alias for get_category_or_default)
pub fn refresh_category(_sport: &str) -> Result<CategoryData, ScraperError> {
    Ok(get_category_or_default(_sport))
}

/// Get cached category data (placeholder - returns None for non-existent sports)
/// In a full implementation, this would check the storage/cache
#[allow(dead_code)]
pub fn get_cached_category(_sport: &str) -> Option<CategoryData> {
    // Currently returns None to indicate no cached data
    // Real implementation would check storage
    None
}

fn default_sports() -> Vec<Category> {
    vec![
        Category { slug: "football".to_string(), name: "Football".to_string(), url: "/football/".to_string(), category_type: None },
        Category { slug: "basketball".to_string(), name: "Basketball".to_string(), url: "/basketball/".to_string(), category_type: None },
        Category { slug: "tennis".to_string(), name: "Tennis".to_string(), url: "/tennis/".to_string(), category_type: None },
        Category { slug: "baseball".to_string(), name: "Baseball".to_string(), url: "/baseball/".to_string(), category_type: None },
        Category { slug: "volleyball".to_string(), name: "Volleyball".to_string(), url: "/volleyball/".to_string(), category_type: None },
        Category { slug: "boxing".to_string(), name: "Boxing".to_string(), url: "/boxing/".to_string(), category_type: None },
        Category { slug: "mma".to_string(), name: "MMA".to_string(), url: "/mma/".to_string(), category_type: None },
        Category { slug: "hockey".to_string(), name: "Hockey".to_string(), url: "/hockey/".to_string(), category_type: None },
        Category { slug: "handball".to_string(), name: "Handball".to_string(), url: "/handball/".to_string(), category_type: None },
        Category { slug: "futsal".to_string(), name: "Futsal".to_string(), url: "/futsal/".to_string(), category_type: None },
        Category { slug: "rugby-union".to_string(), name: "Rugby Union".to_string(), url: "/rugby-union/".to_string(), category_type: None },
        Category { slug: "rugby-league".to_string(), name: "Rugby League".to_string(), url: "/rugby-league/".to_string(), category_type: None },
        Category { slug: "aussie-rules".to_string(), name: "Aussie Rules".to_string(), url: "/aussie-rules/".to_string(), category_type: None },
        Category { slug: "bandy".to_string(), name: "Bandy".to_string(), url: "/bandy/".to_string(), category_type: None },
        Category { slug: "american-football".to_string(), name: "American Football".to_string(), url: "/american-football/".to_string(), category_type: None },
        Category { slug: "table-tennis".to_string(), name: "Table Tennis".to_string(), url: "/table-tennis/".to_string(), category_type: None },
        Category { slug: "badminton".to_string(), name: "Badminton".to_string(), url: "/badminton/".to_string(), category_type: None },
        Category { slug: "snooker".to_string(), name: "Snooker".to_string(), url: "/snooker/".to_string(), category_type: None },
        Category { slug: "darts".to_string(), name: "Darts".to_string(), url: "/darts/".to_string(), category_type: None },
        Category { slug: "water-polo".to_string(), name: "Water Polo".to_string(), url: "/water-polo/".to_string(), category_type: None },
        Category { slug: "esports".to_string(), name: "eSports".to_string(), url: "/esports/".to_string(), category_type: None },
        Category { slug: "cricket".to_string(), name: "Cricket".to_string(), url: "/cricket/".to_string(), category_type: None },
        Category { slug: "floorball".to_string(), name: "Floorball".to_string(), url: "/floorball/".to_string(), category_type: None },
        Category { slug: "beach-volleyball".to_string(), name: "Beach Volleyball".to_string(), url: "/beach-volleyball/".to_string(), category_type: None },
    ]
}

fn default_football_categories() -> Vec<Category> {
    vec![
        Category { slug: "england".to_string(), name: "England".to_string(), url: "/football/england/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "spain".to_string(), name: "Spain".to_string(), url: "/football/spain/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "italy".to_string(), name: "Italy".to_string(), url: "/football/italy/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "germany".to_string(), name: "Germany".to_string(), url: "/football/germany/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "france".to_string(), name: "France".to_string(), url: "/football/france/".to_string(), category_type: Some("country".to_string()) },
    ]
}
