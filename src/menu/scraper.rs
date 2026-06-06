//! Menu scraper module
//!
//! Scrapes sports category menu data from oddsportal.com with SQLite caching

use crate::http::{HttpClient, HttpClientError};
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::RwLock;

use super::models::{MenuData, SportCategory};
use super::storage::Storage;

/// Memory cache (for fast access)
static MEMORY_CACHE: OnceCell<RwLock<Option<MenuData>>> = OnceCell::new();

/// Get memory cache instance
fn get_memory_cache() -> &'static RwLock<Option<MenuData>> {
    MEMORY_CACHE.get_or_init(|| RwLock::new(None))
}

/// SQLite storage instance
static STORAGE_INSTANCE: OnceCell<Storage> = OnceCell::new();

/// Default sports list (for fallback)
fn default_sports() -> Vec<SportCategory> {
    vec![
        SportCategory { slug: "football".to_string(), name: "FOOTBALL".to_string(), url: "/football".to_string() },
        SportCategory { slug: "basketball".to_string(), name: "BASKETBALL".to_string(), url: "/basketball".to_string() },
        SportCategory { slug: "tennis".to_string(), name: "TENNIS".to_string(), url: "/tennis".to_string() },
        SportCategory { slug: "baseball".to_string(), name: "BASEBALL".to_string(), url: "/baseball".to_string() },
        SportCategory { slug: "hockey".to_string(), name: "HOCKEY".to_string(), url: "/ice-hockey".to_string() },
        SportCategory { slug: "american-football".to_string(), name: "AMERICAN FOOTBALL".to_string(), url: "/american-football".to_string() },
        SportCategory { slug: "aussie-rules".to_string(), name: "AUSSIE RULES".to_string(), url: "/aussie-rules".to_string() },
        SportCategory { slug: "badminton".to_string(), name: "BADMINTON".to_string(), url: "/badminton".to_string() },
        SportCategory { slug: "boxing".to_string(), name: "BOXING".to_string(), url: "/boxing".to_string() },
        SportCategory { slug: "cricket".to_string(), name: "CRICKET".to_string(), url: "/cricket".to_string() },
        SportCategory { slug: "darts".to_string(), name: "DARTS".to_string(), url: "/darts".to_string() },
        SportCategory { slug: "esports".to_string(), name: "ESPORTS".to_string(), url: "/esports".to_string() },
        SportCategory { slug: "futsal".to_string(), name: "FUTSAL".to_string(), url: "/futsal".to_string() },
        SportCategory { slug: "golf".to_string(), name: "GOLF".to_string(), url: "/golf".to_string() },
        SportCategory { slug: "handball".to_string(), name: "HANDBALL".to_string(), url: "/handball".to_string() },
        SportCategory { slug: "mma".to_string(), name: "MMA".to_string(), url: "/mma".to_string() },
        SportCategory { slug: "rugby-league".to_string(), name: "RUGBY LEAGUE".to_string(), url: "/rugby-league".to_string() },
        SportCategory { slug: "rugby-union".to_string(), name: "RUGBY UNION".to_string(), url: "/rugby-union".to_string() },
        SportCategory { slug: "snooker".to_string(), name: "SNOOKER".to_string(), url: "/snooker".to_string() },
        SportCategory { slug: "table-tennis".to_string(), name: "TABLE TENNIS".to_string(), url: "/table-tennis".to_string() },
        SportCategory { slug: "volleyball".to_string(), name: "VOLLEYBALL".to_string(), url: "/volleyball".to_string() },
        SportCategory { slug: "water-polo".to_string(), name: "WATER POLO".to_string(), url: "/water-polo".to_string() },
    ]
}

/// Initialize storage (call once)
pub fn init_storage(data_dir: Option<PathBuf>) -> Result<(), super::storage::StorageError> {
    let db_path = data_dir.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polymarket_analysis")
            .join("menu_cache.db")
    });
    
    let storage = Storage::new(&db_path)?;
    STORAGE_INSTANCE.set(storage).map_err(|_| {
        super::storage::StorageError::NotInitialized
    })?;
    
    tracing::info!("Menu storage initialized at {:?}", db_path);
    Ok(())
}

/// Get storage instance
fn get_storage() -> Option<&'static Storage> {
    STORAGE_INSTANCE.get()
}

/// Scrape menu data from oddsportal.com
pub async fn scrape_menu(client: &HttpClient) -> Result<MenuData, MenuClientError> {
    let url = format!("{}/", client.oddsportal_url);
    tracing::info!("Scraping menu from: {}", url);
    
    let html = client.get_with_retry(&url, 2).await?;
    
    // Parse HTML to get sports categories
    let sports = parse_menu_html(&html);
    
    let menu_data = MenuData {
        sports,
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: client.oddsportal_url.clone(),
    };
    
    // Update cache
    update_cache(&menu_data);
    
    Ok(menu_data)
}

/// Parse menu HTML to extract sport categories
fn parse_menu_html(html: &str) -> Vec<SportCategory> {
    let mut sports = Vec::new();
    
    // Simple parsing: find sport links in the navigation
    // Looking for patterns like: <a href="/football/">Football</a>
    let re = regex::Regex::new(r#"<a\s+href="(/\w[\w-]*/?)"[^>]*>([^<]+)</a>"#).unwrap();
    
    for cap in re.captures_iter(html) {
        if let (Some(path_match), Some(name_match)) = (cap.get(1), cap.get(2)) {
            let path = path_match.as_str();
            let name = name_match.as_str().trim();
            
            // Skip non-sport paths
            if !is_valid_sport_path(path) {
                continue;
            }
            
            // Convert path to slug
            let slug = path.trim_start_matches('/').trim_end_matches('/').to_string();
            
            // Skip duplicates
            if sports.iter().any(|s: &SportCategory| s.slug == slug) {
                continue;
            }
            
            // Validate name (should be capitalized)
            if name.len() < 2 || name.chars().any(|c| !c.is_ascii_alphabetic() && c != ' ') {
                continue;
            }
            
            sports.push(SportCategory {
                slug,
                name: name.to_uppercase(),
                url: path.to_string(),
            });
        }
    }
    
    // Fallback to default if parsing fails
    if sports.is_empty() {
        tracing::warn!("Could not parse menu from oddsportal, using default sports list");
        return default_sports();
    }
    
    // Deduplicate and sort
    sports.sort_by(|a, b| a.slug.cmp(&b.slug));
    sports.dedup_by(|a, b| a.slug == b.slug);
    
    tracing::info!("Parsed {} sports from oddsportal", sports.len());
    sports
}

/// Check if path is a valid sport category path
fn is_valid_sport_path(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    
    let exclude_paths = [
        "search", "login", "register", "user", "admin", "settings",
        "api", "static", "css", "js", "images", "img", "help",
        "bookmaker", "coupon", "betslip", "account",
    ];
    
    if exclude_paths.contains(&path) {
        return false;
    }
    
    if !path.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
        return false;
    }
    
    if path.len() < 3 || path.len() > 30 {
        return false;
    }
    
    true
}

/// Update menu cache (memory + SQLite)
fn update_cache(menu_data: &MenuData) {
    // Update memory cache
    if let Ok(mut cache) = get_memory_cache().write() {
        *cache = Some(menu_data.clone());
    }
    
    // Update SQLite cache
    if let Some(storage) = get_storage() {
        if let Err(e) = storage.save_menu(menu_data) {
            tracing::error!("Failed to save menu to SQLite: {}", e);
        }
    }
    
    tracing::info!("Menu cache updated with {} sports", menu_data.sports.len());
}

/// Get cached menu data (priority: memory cache)
pub fn get_cached_menu() -> Option<MenuData> {
    // Try memory cache first
    if let Ok(cache) = get_memory_cache().read() {
        if let Some(data) = cache.clone() {
            return Some(data);
        }
    }
    
    // Memory cache miss, try SQLite
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // Restore memory cache
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            return Some(data);
        }
    }
    
    None
}

/// Refresh menu data
pub async fn refresh_menu(client: &HttpClient) -> Result<MenuData, MenuClientError> {
    scrape_menu(client).await
}

/// Get menu data (priority: SQLite cache, fallback to default)
pub fn get_menu_or_default() -> MenuData {
    // Try cache first (including SQLite)
    if let Some(data) = get_cached_menu() {
        return data;
    }
    
    // Try SQLite (first load)
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // Restore memory cache
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            return data;
        }
    }
    
    // Use default data
    MenuData {
        sports: default_sports(),
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: "default".to_string(),
    }
}

/// Initialize menu data (from SQLite)
pub fn init_menu_from_storage() -> Option<MenuData> {
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // Restore memory cache
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            tracing::info!("Menu loaded from SQLite storage, {} sports", data.sports.len());
            return Some(data);
        }
    }
    None
}

/// Menu scraper error type
#[derive(Debug)]
pub enum MenuClientError {
    HttpError(HttpClientError),
    ParseError(String),
}

impl std::fmt::Display for MenuClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MenuClientError::HttpError(e) => write!(f, "HTTP error: {}", e),
            MenuClientError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for MenuClientError {}

impl From<HttpClientError> for MenuClientError {
    fn from(err: HttpClientError) -> Self {
        MenuClientError::HttpError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_sport_path() {
        assert!(is_valid_sport_path("/football"));
        assert!(is_valid_sport_path("/basketball"));
        assert!(is_valid_sport_path("/ice-hockey"));
        assert!(!is_valid_sport_path("/admin"));
        assert!(!is_valid_sport_path("/api/hello"));
        assert!(!is_valid_sport_path("/"));
    }

    #[test]
    fn test_default_sports() {
        let sports = default_sports();
        assert!(!sports.is_empty());
        assert!(sports.iter().any(|s| s.slug == "football"));
    }

    #[test]
    fn test_get_menu_or_default() {
        let menu = get_menu_or_default();
        assert!(!menu.sports.is_empty());
        assert_eq!(menu.source, "default");
    }
}
