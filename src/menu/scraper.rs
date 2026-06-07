//! Unified scraper for menu and category data from OddsPortal

use crate::menu::models::{Category, CategoryData};
use reqwest::Client;
use scraper::Selector;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

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
    
    // Parse sport-data from HTML
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
    // Find sport-data="..." pattern
    let start_pattern = r#"sport-data="{""#;
    let end_pattern = r#"" :href-lang"#;
    
    if let Some(start_idx) = html.find(start_pattern) {
        let data_start = start_idx + start_pattern.len() - 2; // Include the opening quote
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
    
    // Fallback: try to find sport-data pattern more broadly
    if let Some(start_idx) = html.find("sport-data=\"{") {
        let data_start = start_idx + 12;
        if let Some(end_idx) = html[data_start..].find('"') {
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

/// Decode HTML entities like &quot; to "
fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
     .replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&apos;", "'")
}

/// Fetch category page to get country/league tree
pub async fn fetch_categories_for_sport(sport: &str) -> Result<Vec<Category>, ScraperError> {
    let url = format!("https://www.oddsportal.com/{}/", sport);
    let html = CLIENT.get(&url).send().await
        .map_err(|e| ScraperError::Network(e.to_string()))?
        .text().await
        .map_err(|e| ScraperError::Network(e.to_string()))?;
    
    let document = scraper::Html::parse_document(&html);
    let mut categories = Vec::new();
    
    // Find all links that match the pattern /{sport}/{country}/ or /{sport}/{region}/
    let link_selector = Selector::parse("a[href]").unwrap();
    
    let base_pattern = format!("/{}/", sport);
    
    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            // Check if this is a category link (not event link)
            // Pattern: /{sport}/{slug}/
            // Exclude: /{sport}/{country}/{league}/{event}/
            let path = href.trim_end_matches('/');
            
            if path.starts_with(&base_pattern) && path != base_pattern.trim_end_matches('/') {
                let remaining = &path[base_pattern.len()..];
                
                // Only include top-level categories (one level deep)
                // i.e., /{sport}/{country}/
                if !remaining.contains('/') && !remaining.is_empty() {
                    // Determine if this is a country, region, or tournament
                    let category_type = classify_category(remaining, sport);
                    
                    let slug = remaining.to_string();
                    let name = format_name_from_slug(&slug);
                    let url = format!("{}/", path); // Ensure trailing slash
                    
                    // Avoid duplicates
                    if !categories.iter().any(|c: &Category| c.slug == slug) {
                        categories.push(Category {
                            slug,
                            name,
                            url,
                            category_type: Some(category_type),
                        });
                    }
                }
            }
        }
    }
    
    // Sort by name
    categories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    
    Ok(categories)
}

/// Classify the category type based on slug patterns
fn classify_category(slug: &str, _sport: &str) -> String {
    let slug_lower = slug.to_lowercase();
    
    // Common country names
    let countries = [
        "argentina", "australia", "austria", "belgium", "brazil", "chile", "china",
        "colombia", "croatia", "czech-republic", "denmark", "ecuador", "england",
        "finland", "france", "germany", "greece", "hungary", "india", "indonesia",
        "ireland", "italy", "japan", "mexico", "netherlands", "norway", "paraguay",
        "peru", "poland", "portugal", "romania", "russia", "saudi-arabia", "scotland",
        "serbia", "slovakia", "south-africa", "south-korea", "spain", "sweden",
        "switzerland", "turkey", "ukraine", "uruguay", "usa", "venezuela", "wales",
        "bosnia-and-herzegovina", "iran", "israel", "thailand", "vietnam",
    ];
    
    if countries.contains(&slug_lower.as_str()) {
        return "country".to_string();
    }
    
    // Special regions
    let regions = ["asia", "europe", "africa", "world", "central-america", "oceania", "conmebol", "uefa", "concacaf"];
    if regions.contains(&slug_lower.as_str()) {
        return "region".to_string();
    }
    
    // Common league patterns (international tournaments, world cups, etc.)
    let league_patterns = ["champions-league", "europa-league", "conference-league", 
                          "world-cup", "euro", "copa-america", "afcon",
                          "asian-cup", "premier-league", "serie-a", "bundesliga",
                          "ligue-1", "eredivisie", "primeira-liga", "la-liga",
                          "mls", "j-league", "a-league", "super-league"];
    
    for pattern in league_patterns {
        if slug_lower.contains(pattern) {
            return "league".to_string();
        }
    }
    
    // Default to "other" for unclassified categories
    "other".to_string()
}

/// Format slug to readable name
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

/// Capitalize first letter
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

/// Get category data (fetches from OddsPortal or returns cached)
pub async fn get_category_data(sport: &str) -> CategoryData {
    if sport == "menu" {
        match fetch_sports().await {
            Ok(categories) => {
                CategoryData {
                    sport: "menu".to_string(),
                    categories,
                    last_updated: get_timestamp(),
                    source: "oddsportal".to_string(),
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch sports: {}", e);
                // Return default data on error
                get_category_or_default("menu")
            }
        }
    } else {
        match fetch_categories_for_sport(sport).await {
            Ok(categories) => {
                CategoryData {
                    sport: sport.to_string(),
                    categories,
                    last_updated: get_timestamp(),
                    source: "oddsportal".to_string(),
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch categories for {}: {}", sport, e);
                // Return default data on error
                get_category_or_default(sport)
            }
        }
    }
}

/// Get default category data (fallback when scraping fails)
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
