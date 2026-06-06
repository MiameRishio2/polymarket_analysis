//! 菜单爬取模块
//!
//! 从 oddsportal.com 爬取体育分类菜单数据，支持 SQLite 缓存

use crate::http::{HttpClient, HttpClientError};
use crate::storage::Storage;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// 单个体育分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SportCategory {
    pub slug: String,
    pub name: String,
    pub url: String,
}

/// 菜单数据响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuData {
    pub sports: Vec<SportCategory>,
    pub last_updated: String,
    pub source: String,
}

/// 内存缓存（用于快速访问）
static MEMORY_CACHE: OnceCell<RwLock<Option<MenuData>>> = OnceCell::new();

/// 获取内存缓存实例
fn get_memory_cache() -> &'static RwLock<Option<MenuData>> {
    MEMORY_CACHE.get_or_init(|| RwLock::new(None))
}

/// SQLite 存储实例
static STORAGE_INSTANCE: OnceCell<Storage> = OnceCell::new();

/// 默认体育分类列表（用于回退）
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

/// 初始化存储（调用一次）
pub fn init_storage(data_dir: Option<PathBuf>) -> Result<(), crate::storage::StorageError> {
    let db_path = data_dir.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("polymarket_analysis")
            .join("menu_cache.db")
    });
    
    let storage = Storage::new(&db_path)?;
    STORAGE_INSTANCE.set(storage).map_err(|_| {
        crate::storage::StorageError::NotInitialized
    })?;
    
    tracing::info!("Menu storage initialized at {:?}", db_path);
    Ok(())
}

/// 获取存储实例
fn get_storage() -> Option<&'static Storage> {
    STORAGE_INSTANCE.get()
}

/// 从 oddsportal.com 爬取菜单数据
pub async fn scrape_menu(client: &HttpClient) -> Result<MenuData, MenuClientError> {
    let url = format!("{}/", client.oddsportal_url);
    tracing::info!("Scraping menu from: {}", url);
    
    let html = client.get_with_retry(&url, 2).await?;
    
    // 解析 HTML 获取体育分类
    let sports = parse_menu_html(&html);
    
    let menu_data = MenuData {
        sports,
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: client.oddsportal_url.clone(),
    };
    
    // 更新缓存
    update_cache(&menu_data);
    
    Ok(menu_data)
}

/// 解析 HTML 提取体育分类
fn parse_menu_html(html: &str) -> Vec<SportCategory> {
    use regex::Regex;
    
    let mut sports = Vec::new();
    
    // Pattern 1: 匹配导航菜单中的体育分类链接
    let re_link = Regex::new(r#"href="(/[a-z-]+)"[^>]*>([^<]+)</a>"#).ok();
    let re_link2 = Regex::new(r#"<a[^>]*href="(/[a-z-]+)"[^>]*>\s*([^<\s]+)"#).ok();
    
    if let (Some(re1), Some(re2)) = (re_link, re_link2) {
        let mut seen = std::collections::HashSet::new();
        
        for cap in re1.captures_iter(html) {
            let path = &cap[1];
            let name = cap[2].trim();
            
            if is_valid_sport_path(path) && !seen.contains(path) {
                seen.insert(path.to_string());
                let slug = path.trim_start_matches('/').to_string();
                let name_upper = name.to_uppercase();
                sports.push(SportCategory {
                    slug: slug.clone(),
                    name: name_upper,
                    url: path.to_string(),
                });
            }
        }
        
        for cap in re2.captures_iter(html) {
            let path = &cap[1];
            let name = cap[2].trim();
            
            if is_valid_sport_path(path) && !seen.contains(path) {
                seen.insert(path.to_string());
                let slug = path.trim_start_matches('/').to_string();
                let name_upper = name.to_uppercase();
                sports.push(SportCategory {
                    slug: slug.clone(),
                    name: name_upper,
                    url: path.to_string(),
                });
            }
        }
    }
    
    // 如果没有从网站获取到数据，使用默认列表
    if sports.is_empty() {
        tracing::warn!("Could not parse menu from oddsportal, using default sports list");
        return default_sports();
    }
    
    // 去重并排序
    sports.sort_by(|a, b| a.slug.cmp(&b.slug));
    sports.dedup_by(|a, b| a.slug == b.slug);
    
    tracing::info!("Parsed {} sports from oddsportal", sports.len());
    sports
}

/// 检查路径是否为有效的体育分类路径
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

/// 更新菜单缓存（内存 + SQLite）
fn update_cache(menu_data: &MenuData) {
    // 更新内存缓存
    if let Ok(mut cache) = get_memory_cache().write() {
        *cache = Some(menu_data.clone());
    }
    
    // 更新 SQLite 缓存
    if let Some(storage) = get_storage() {
        if let Err(e) = storage.save_menu(menu_data) {
            tracing::error!("Failed to save menu to SQLite: {}", e);
        }
    }
    
    tracing::info!("Menu cache updated with {} sports", menu_data.sports.len());
}

/// 获取缓存的菜单数据（优先内存缓存）
pub fn get_cached_menu() -> Option<MenuData> {
    // 优先从内存缓存获取
    if let Ok(cache) = get_memory_cache().read() {
        if let Some(data) = cache.clone() {
            return Some(data);
        }
    }
    
    // 内存缓存没有，尝试从 SQLite 获取
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // 恢复内存缓存
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            return Some(data);
        }
    }
    
    None
}

/// 刷新菜单数据
pub async fn refresh_menu(client: &HttpClient) -> Result<MenuData, MenuClientError> {
    scrape_menu(client).await
}

/// 获取菜单数据（优先 SQLite 缓存，如无缓存则返回默认数据）
pub fn get_menu_or_default() -> MenuData {
    // 优先从缓存获取（包括 SQLite）
    if let Some(data) = get_cached_menu() {
        return data;
    }
    
    // 尝试从 SQLite 初始化（首次加载）
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // 恢复内存缓存
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            return data;
        }
    }
    
    // 使用默认数据
    MenuData {
        sports: default_sports(),
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: "default".to_string(),
    }
}

/// 初始化菜单数据（从 SQLite 加载）
pub fn init_menu_from_storage() -> Option<MenuData> {
    if let Some(storage) = get_storage() {
        if let Ok(Some(data)) = storage.load_menu() {
            // 恢复内存缓存
            if let Ok(mut cache) = get_memory_cache().write() {
                *cache = Some(data.clone());
            }
            tracing::info!("Menu loaded from SQLite storage, {} sports", data.sports.len());
            return Some(data);
        }
    }
    None
}

/// 菜单爬取错误类型
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
