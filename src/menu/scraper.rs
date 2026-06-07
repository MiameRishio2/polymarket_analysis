//! Unified scraper for menu and category data

use std::collections::HashSet;

use crate::http::{HttpClient, HttpClientError};
use crate::menu::models::{Category, CategoryData};

/// Scraper error type
#[derive(Debug, thiserror::Error)]
pub enum ScraperError {
    #[error("HTTP error: {0}")]
    Http(#[from] HttpClientError),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Global storage instance (using std sync instead of tokio for simplicity)
use std::sync::{Arc, Mutex};
static STORAGE: once_cell::sync::Lazy<Mutex<Option<Arc<super::storage::Storage>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

/// Initialize storage
pub fn init_storage(storage: Option<Arc<super::storage::Storage>>) -> Result<(), super::storage::StorageError> {
    let mut guard = STORAGE.lock().unwrap();
    *guard = storage;
    Ok(())
}

/// Get storage instance
pub fn get_storage() -> Option<Arc<super::storage::Storage>> {
    STORAGE.lock().ok()?.clone()
}

/// Scrape category data for a sport
pub async fn scrape_category(client: &HttpClient, sport: &str) -> Result<CategoryData, ScraperError> {
    let base = client.oddsportal_url.trim_end_matches('/');
    
    // For "menu", scrape from root URL to get sports list
    let url = if sport == "menu" {
        format!("{}/", base)
    } else {
        format!("{}/{}/", base, sport)
    };
    
    tracing::info!("Scraping {} from: {}", sport, url);
    
    let html = client.get_with_retry(&url, 2).await?;
    
    let categories = if sport == "menu" {
        parse_sports_menu(&html)
    } else {
        parse_category_html(&html, sport)
    };
    
    Ok(CategoryData {
        sport: sport.to_string(),
        categories,
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: url,
    })
}

/// Parse sports menu page (returns sport links)
fn parse_sports_menu(html: &str) -> Vec<Category> {
    let mut categories = Vec::new();
    let mut seen_slugs = HashSet::new();
    
    let re = match regex::Regex::new(r#"<a\s+href="(/\w+(?:-\w+)*/)"[^>]*>(?:[\s\S]*?)</a>"#) {
        Ok(r) => r,
        Err(_) => return categories,
    };
    
    for cap in re.captures_iter(html) {
        if let (Some(url_match), Some(slug_match)) = (cap.get(1), cap.get(2)) {
            let url = url_match.as_str();
            let slug = slug_match.as_str().trim_start_matches('/').trim_end_matches('/');
            
            // Skip invalid paths
            if !is_valid_sport_path(slug) {
                continue;
            }
            
            // Skip duplicates
            if seen_slugs.contains(slug) {
                continue;
            }
            seen_slugs.insert(slug.to_string());
            
            // Extract display name
            let link_html = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let name = extract_link_text(link_html).unwrap_or_else(|| slug.replace('-', " "));
            
            categories.push(Category {
                slug: slug.to_string(),
                name,
                url: url.to_string(),
                category_type: None,
            });
        }
    }
    
    // Add default sports if parsing failed
    if categories.is_empty() {
        tracing::warn!("Could not parse sports from oddsportal, using default list");
        return default_sports();
    }
    
    categories.sort_by(|a, b| a.slug.cmp(&b.slug));
    categories.dedup_by(|a, b| a.slug == b.slug);
    
    tracing::info!("Parsed {} sports from oddsportal", categories.len());
    categories
}

/// Parse category page (like football categories)
fn parse_category_html(html: &str, sport: &str) -> Vec<Category> {
    let mut categories = Vec::new();
    let mut seen_slugs = HashSet::new();
    
    // Match links for this sport
    let pattern = format!(r#"<a\s+href="(/{}/([^/]+)/)"[^>]*>[\s\S]*?</a>"#, regex::escape(sport));
    let re = match regex::Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return categories,
    };
    
    // Extract text from nested tags
    let text_re = regex::Regex::new(r"<span[^>]*>([^<]+)</span>").ok();
    
    for cap in re.captures_iter(html) {
        if let (Some(_url), Some(slug)) = (cap.get(1), cap.get(2)) {
            let url_str = _url.as_str();
            let slug_str = slug.as_str();
            
            // Skip multi-level paths (e.g., england/premier-league)
            if url_str.matches('/').count() > 2 {
                continue;
            }
            
            // Skip special pages
            let exclude = ["results", "standings", "search", "archive", "api", "tools"];
            if exclude.contains(&slug_str) {
                continue;
            }
            
            // Skip navigation elements
            let link_html = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let name = text_re
                .as_ref()
                .and_then(|r| r.captures(link_html))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or(slug_str);
            
            if is_navigation_element(name) {
                continue;
            }
            
            // Skip duplicates
            if seen_slugs.contains(slug_str) {
                continue;
            }
            seen_slugs.insert(slug_str.to_string());
            
            // Determine category type
            let category_type = if slug_str == "world" || slug_str == "world-championship" {
                Some("global".to_string())
            } else {
                Some("country".to_string())
            };
            
            let name = normalize_name(name);
            
            categories.push(Category {
                slug: slug_str.to_string(),
                name,
                url: url_str.to_string(),
                category_type,
            });
        }
    }
    
    categories.sort_by(|a, b| a.name.cmp(&b.name));
    tracing::info!("Parsed {} {} categories", categories.len(), sport);
    categories
}

/// Extract display text from link HTML
fn extract_link_text(link_html: &str) -> Option<String> {
    // Try to find text content (prioritize text in <span> tags)
    let span_re = regex::Regex::new(r"<span[^>]*>([^<]+)</span>").ok()?;
    if let Some(cap) = span_re.captures(link_html) {
        if let Some(m) = cap.get(1) {
            let text = m.as_str().trim();
            if !text.is_empty() && !is_navigation_element(text) {
                return Some(text.to_string());
            }
        }
    }
    
    // Fallback to plain text extraction
    let text = link_html
        .chars()
        .skip_while(|c| *c != '>')
        .take_while(|c| *c != '<')
        .collect::<String>()
        .trim()
        .to_string();
    
    if text.is_empty() { None } else { Some(text) }
}

/// Check if text is a navigation element
fn is_navigation_element(text: &str) -> bool {
    let nav_elements = ["live", "popular", "prematches", "results", "bonuses", "home"];
    nav_elements.contains(&text.to_lowercase().trim())
}

/// Normalize display name
fn normalize_name(name: &str) -> String {
    let name = name.trim();
    let name = name.replace(|c: char| c.is_whitespace() && c != ' ', " ");
    if name.len() > 2 {
        let mut chars = name.chars();
        let first = chars.next().unwrap().to_uppercase().to_string();
        first + &chars.as_str().to_lowercase()
    } else {
        name.to_string()
    }
}

/// Check if path is a valid sport category path
fn is_valid_sport_path(path: &str) -> bool {
    let valid_sports = [
        "football", "basketball", "tennis", "baseball", "volleyball",
        "esports", "hockey", "american-football", "boxing", "mma",
        "cricket", "rugby-union", "rugby-league", "aussie-rules",
    ];
    valid_sports.contains(&path)
}

/// Default sports list (verified against oddsportal.com)
pub fn default_sports() -> Vec<Category> {
    vec![
        // Main sports
        Category { slug: "football".to_string(), name: "Football".to_string(), url: "/football/".to_string(), category_type: None },
        Category { slug: "basketball".to_string(), name: "Basketball".to_string(), url: "/basketball/".to_string(), category_type: None },
        Category { slug: "tennis".to_string(), name: "Tennis".to_string(), url: "/tennis/".to_string(), category_type: None },
        Category { slug: "baseball".to_string(), name: "Baseball".to_string(), url: "/baseball/".to_string(), category_type: None },
        Category { slug: "volleyball".to_string(), name: "Volleyball".to_string(), url: "/volleyball/".to_string(), category_type: None },
        // Combat sports
        Category { slug: "boxing".to_string(), name: "Boxing".to_string(), url: "/boxing/".to_string(), category_type: None },
        Category { slug: "mma".to_string(), name: "MMA".to_string(), url: "/mma/".to_string(), category_type: None },
        // Team sports
        Category { slug: "hockey".to_string(), name: "Hockey".to_string(), url: "/hockey/".to_string(), category_type: None },
        Category { slug: "handball".to_string(), name: "Handball".to_string(), url: "/handball/".to_string(), category_type: None },
        Category { slug: "futsal".to_string(), name: "Futsal".to_string(), url: "/futsal/".to_string(), category_type: None },
        // Rugby/Aussie Rules
        Category { slug: "rugby-union".to_string(), name: "Rugby Union".to_string(), url: "/rugby-union/".to_string(), category_type: None },
        Category { slug: "rugby-league".to_string(), name: "Rugby League".to_string(), url: "/rugby-league/".to_string(), category_type: None },
        Category { slug: "aussie-rules".to_string(), name: "Aussie Rules".to_string(), url: "/aussie-rules/".to_string(), category_type: None },
        Category { slug: "bandy".to_string(), name: "Bandy".to_string(), url: "/bandy/".to_string(), category_type: None },
        // American Sports
        Category { slug: "american-football".to_string(), name: "American Football".to_string(), url: "/american-football/".to_string(), category_type: None },
        // Racket sports
        Category { slug: "table-tennis".to_string(), name: "Table Tennis".to_string(), url: "/table-tennis/".to_string(), category_type: None },
        Category { slug: "badminton".to_string(), name: "Badminton".to_string(), url: "/badminton/".to_string(), category_type: None },
        // Cue sports
        Category { slug: "snooker".to_string(), name: "Snooker".to_string(), url: "/snooker/".to_string(), category_type: None },
        Category { slug: "darts".to_string(), name: "Darts".to_string(), url: "/darts/".to_string(), category_type: None },
        // Water sports
        Category { slug: "water-polo".to_string(), name: "Water Polo".to_string(), url: "/water-polo/".to_string(), category_type: None },
        // Other sports
        Category { slug: "esports".to_string(), name: "eSports".to_string(), url: "/esports/".to_string(), category_type: None },
        Category { slug: "cricket".to_string(), name: "Cricket".to_string(), url: "/cricket/".to_string(), category_type: None },
        Category { slug: "floorball".to_string(), name: "Floorball".to_string(), url: "/floorball/".to_string(), category_type: None },
        Category { slug: "beach-volleyball".to_string(), name: "Beach Volleyball".to_string(), url: "/beach-volleyball/".to_string(), category_type: None },
    ]
}


/// Default football categories
pub fn default_football_categories() -> Vec<Category> {
    vec![
        Category { slug: "england".to_string(), name: "England".to_string(), url: "/football/england/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "spain".to_string(), name: "Spain".to_string(), url: "/football/spain/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "italy".to_string(), name: "Italy".to_string(), url: "/football/italy/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "germany".to_string(), name: "Germany".to_string(), url: "/football/germany/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "france".to_string(), name: "France".to_string(), url: "/football/france/".to_string(), category_type: Some("country".to_string()) },
    ]
}

/// Refresh category data (scrape + cache)
pub async fn refresh_category(client: &HttpClient, sport: &str) -> Result<CategoryData, ScraperError> {
    let data = scrape_category(client, sport).await?;
    
    // Save to storage
    if let Some(storage) = get_storage() {
        if let Err(e) = storage.save(sport, &data) {
            tracing::error!("Failed to save {} data to storage: {}", sport, e);
        }
    }
    
    Ok(data)
}

/// Get cached category data
pub fn get_cached_category(sport: &str) -> Option<CategoryData> {
    get_storage().and_then(|s| s.load(sport).ok().flatten())
}

/// Get category data or default
pub fn get_category_or_default(sport: &str) -> CategoryData {
    // Try cache first
    if let Some(data) = get_cached_category(sport) {
        return data;
    }
    
    // Return default based on sport
    if sport == "menu" {
        CategoryData {
            sport: "menu".to_string(),
            categories: default_sports(),
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "default".to_string(),
        }
    } else {
        CategoryData {
            sport: sport.to_string(),
            categories: default_football_categories(),
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "default".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_sport_path() {
        assert!(is_valid_sport_path("football"));
        assert!(is_valid_sport_path("basketball"));
        assert!(is_valid_sport_path("ice-hockey"));
        assert!(!is_valid_sport_path("admin"));
        assert!(!is_valid_sport_path("api"));
    }

    #[test]
    fn test_default_sports() {
        let sports = default_sports();
        assert!(!sports.is_empty());
        assert!(sports.iter().any(|s| s.slug == "football"));
    }

    #[test]
    fn test_default_football_categories() {
        let categories = default_football_categories();
        assert!(!categories.is_empty());
        assert!(categories.iter().any(|c| c.slug == "england"));
    }

    #[test]
    fn test_normalize_name() {
        assert_eq!(normalize_name("ENGLAND"), "England");
        assert_eq!(normalize_name("premier league"), "Premier league");
        assert_eq!(normalize_name("  TEST  "), "Test");
    }

    #[test]
    fn test_is_navigation_element() {
        assert!(is_navigation_element("LIVE"));
        assert!(is_navigation_element("Popular"));
        assert!(!is_navigation_element("England"));
    }
}
