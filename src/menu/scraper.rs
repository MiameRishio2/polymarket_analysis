//! Unified scraper for menu and category data from OddsPortal

use crate::menu::models::{Category, CategoryData};
use reqwest::Client;
use scraper::Selector;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// Non-category paths to exclude (pages, not country/region links)
const EXCLUDED_PATHS: &[&str] = &["results", "standings", "live", "archive"];

/// Scraper error type
#[derive(Debug, thiserror::Error)]
pub enum ScraperError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("HTML parse error: {0}")]
    Parse(String),
}

/// Get current timestamp
fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}

/// HTTP client for scraping
static CLIENT: once_cell::sync::Lazy<Client> = once_cell::sync::Lazy::new(|| {
    Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

/// Fetch main page to get sport list
pub async fn fetch_sports() -> Result<Vec<Category>, ScraperError> {
    let url = "https://www.oddsportal.com/";
    let html = CLIENT.get(url).send().await
        .map_err(|e| ScraperError::Network(e.to_string()))?
        .text().await
        .map_err(|e| ScraperError::Network(e.to_string()))?;
    
    let sport_data = extract_sport_data(&html)?;
    
    let mut categories = Vec::new();
    for (_key, sport) in sport_data {
        let slug = sport.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let name = capitalize_first(&slug);
        let url = sport.get("url").and_then(|s| s.as_str()).unwrap_or("/").to_string();
        
        categories.push(Category {
            slug,
            name,
            url,
            category_type: None,
        });
    }
    
    Ok(categories)
}

/// Extract sport-data JSON from HTML
fn extract_sport_data(html: &str) -> Result<serde_json::Map<String, Value>, ScraperError> {
    let start_pattern = r#"sport-data="{""#;
    let end_pattern = r#"" :href-lang"#;
    
    if let Some(start_idx) = html.find(start_pattern) {
        let data_start = start_idx + start_pattern.len() - 2;
        if let Some(end_idx) = html[data_start..].find(end_pattern) {
            let json_str = &html[data_start..data_start + end_idx];
            let decoded = decode_html_entities(json_str);
            
            match serde_json::from_str::<Value>(&decoded) {
                Ok(value) => {
                    if let Some(obj) = value.as_object() {
                        return Ok(obj.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse sport-data JSON: {}", e);
                }
            }
        }
    }
    
    Err(ScraperError::Parse("Could not find sport-data in HTML".to_string()))
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
     .replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&apos;", "'")
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Fetch category page - extract direct href links like /football/argentina/
pub async fn fetch_categories_for_sport(sport: &str) -> Result<Vec<Category>, ScraperError> {
    let url = format!("https://www.oddsportal.com/{}/", sport);
    
    tracing::info!("Fetching categories from: {}", url);
    
    let response = CLIENT.get(&url).send().await
        .map_err(|e| ScraperError::Network(e.to_string()))?;
    
    // Use lossy conversion to handle any encoding issues
    let bytes = response.bytes().await
        .map_err(|e| ScraperError::Network(e.to_string()))?;
    let html = String::from_utf8_lossy(&bytes);
    
    let document = scraper::Html::parse_document(&html);
    let link_selector = Selector::parse("a[href]").unwrap();
    
    let base_pattern = format!("/{}/", sport);
    let mut categories = Vec::new();
    
    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            // Match: /{sport}/{slug}/  e.g., /football/argentina/
            if let Some(slug) = extract_category_slug(href, &base_pattern) {
                // Skip excluded paths (results, standings, etc.)
                if EXCLUDED_PATHS.contains(&slug.to_lowercase().as_str()) {
                    continue;
                }
                
                // Avoid duplicates
                if categories.iter().any(|c: &Category| c.slug == slug) {
                    continue;
                }
                
                let name = format_name_from_slug(&slug);
                let url = format!("/{}/{}/", sport, slug);
                
                categories.push(Category {
                    slug,
                    name,
                    url,
                    category_type: Some("country".to_string()),
                });
            }
        }
    }
    
    tracing::info!("Found {} categories for {}", categories.len(), sport);
    
    categories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(categories)
}

/// Extract category slug from href if matches /{sport}/{slug}/
fn extract_category_slug(href: &str, base_pattern: &str) -> Option<String> {
    let href = href.trim_end_matches('/');
    
    if !href.starts_with(base_pattern) {
        return None;
    }
    
    let remaining = &href[base_pattern.len()..];
    
    // Must be single-level (no more slashes)
    if remaining.is_empty() || remaining.contains('/') {
        return None;
    }
    
    Some(remaining.to_string())
}

/// Format slug to readable name (e.g., "england" -> "England")
fn format_name_from_slug(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Get category data
pub async fn get_category_data(sport: &str) -> CategoryData {
    match fetch_categories_for_sport(sport).await {
        Ok(categories) => CategoryData {
            sport: sport.to_string(),
            categories,
            last_updated: get_timestamp(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!("Failed to fetch categories for {}: {}", sport, e);
            get_category_or_default(sport)
        }
    }
}

/// Get default category data
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
    ]
}

fn default_football_categories() -> Vec<Category> {
    vec![
        Category { slug: "argentina".to_string(), name: "Argentina".to_string(), url: "/football/argentina/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "asia".to_string(), name: "Asia".to_string(), url: "/football/asia/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "austria".to_string(), name: "Austria".to_string(), url: "/football/austria/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "england".to_string(), name: "England".to_string(), url: "/football/england/".to_string(), category_type: Some("country".to_string()) },
        Category { slug: "europe".to_string(), name: "Europe".to_string(), url: "/football/europe/".to_string(), category_type: Some("country".to_string()) },
    ]
}
