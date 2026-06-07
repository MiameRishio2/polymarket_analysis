//! Football scraper module
//!
//! Scrapes football category sub-items from oddsportal.com/football/

use crate::http::{HttpClient, HttpClientError};

use super::models::{FootballData, FootballSubCategory};
use super::progress;

/// Scrape football data from oddsportal.com/football/
pub async fn scrape_football(client: &HttpClient) -> Result<FootballData, FootballScraperError> {
    let base = client.oddsportal_url.trim_end_matches('/');
    let url = format!("{}/football/", base);
    
    progress::start_refresh("正在获取足球分类列表...").await;
    tracing::info!("Scraping football data from: {}", url);
    
    progress::update_progress("fetching", "正在从 oddsportal.com 获取足球数据...", 10).await;
    
    let html = match client.get_with_retry(&url, 2).await {
        Ok(html) => html,
        Err(e) => {
            let err_msg = format!("HTTP 请求失败: {}", e);
            tracing::error!("{}", err_msg);
            progress::fail_refresh(&err_msg).await;
            return Err(FootballScraperError::HttpError(e));
        }
    };
    
    progress::update_progress("parsing", "正在解析足球分类...", 50).await;
    
    let categories = parse_football_html(&html);
    let total_categories = categories.len();
    
    progress::set_total_items(total_categories as u32).await;
    
    progress::update_progress("complete", &format!("已获取 {} 个足球分类", total_categories), 100).await;
    
    let data = FootballData {
        categories,
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: url,
    };
    
    tracing::info!("Scraped football data with {} categories", data.categories.len());
    progress::complete_refresh().await;
    
    Ok(data)
}

fn parse_football_html(html: &str) -> Vec<FootballSubCategory> {
    let mut categories = Vec::new();
    
    if let Some(upcoming_start) = html.find("Upcoming Events") {
        let start = upcoming_start.saturating_sub(500);
        let end = (upcoming_start + 5000).min(html.len());
        let section = &html[start..end];
        
        if let Some(popular_idx) = section.find("bg-popular-icon") {
            let popular_section = &section[popular_idx.saturating_sub(200)..popular_idx + 50];
            if let Some(name) = extract_text_after_popular(popular_section) {
                if !name.is_empty() {
                    categories.push(FootballSubCategory {
                        slug: "popular".to_string(),
                        name,
                        url: String::new(),
                        category_type: "popular".to_string(),
                    });
                }
            }
        }
        
        // Filter to only country-level links (single path segment after /football/)
        let first_level_links = extract_first_level_links(section);
        
        for link in first_level_links {
            if categories.iter().any(|c| c.url == link.url || c.name == link.name) {
                continue;
            }
            
            // Only include country-level links (not multi-segment paths like /football/england/premier-league/)
            let path = link.url.trim_start_matches("/football/").trim_end_matches("/");
            if !path.contains('/') && link.name != "Popular" {
                categories.push(link);
            }
        }
    }
    
    categories.sort_by(|a, b| a.name.cmp(&b.name));
    categories.dedup_by(|a, b| a.name == b.name || a.slug == b.slug);
    
    categories
}

fn extract_text_after_popular(section: &str) -> Option<String> {
    let re = regex::Regex::new(r"<span[^>]*>\s*Popular\s*</span>").ok()?;
    if let Some(cap) = re.captures(section) {
        let text = cap.get(0)?.as_str();
        let text_re = regex::Regex::new(r">\s*([^<]+)\s*<").ok()?;
        if let Some(name_cap) = text_re.captures(text) {
            let name = name_cap.get(1)?.as_str().trim();
            if !name.is_empty() && name != "Popular" {
                return Some(name.to_string());
            }
        }
    }
    if section.contains(">Popular<") || section.contains(">Popular </") {
        return Some("Popular".to_string());
    }
    None
}

fn extract_first_level_links(section: &str) -> Vec<FootballSubCategory> {
    let mut links = Vec::new();
    
    let re = match regex::Regex::new(r#"<a\s+href="(/football/[^"]+/)"[^>]*>([^<]+)</a>"#) {
        Ok(r) => r,
        Err(_) => return links,
    };
    
    for cap in re.captures_iter(section) {
        if let (Some(url), Some(name)) = (cap.get(1), cap.get(2)) {
            let url_str = url.as_str();
            let name_str = name.as_str().trim();
            
            if is_navigation_element(name_str) {
                continue;
            }
            
            if is_valid_football_category(url_str) {
                let slug = extract_slug_from_url(url_str);
                let category_type = determine_category_type(url_str);
                
                links.push(FootballSubCategory {
                    slug,
                    name: normalize_name(name_str),
                    url: url_str.to_string(),
                    category_type,
                });
            }
        }
    }
    
    links
}

fn is_valid_football_category(url: &str) -> bool {
    if !url.starts_with("/football/") || !url.ends_with('/') {
        return false;
    }
    
    let path = url.trim_start_matches("/football/").trim_end_matches("/");
    !matches!(path, "" | "search" | "results" | "archive" | "api" | "tools")
}

fn determine_category_type(url: &str) -> String {
    let path = url.trim_start_matches("/football/").trim_end_matches("/");
    
    if path.contains('/') {
        if path.contains("premier-league") || path.contains("la-liga") || path.contains("bundesliga")
            || path.contains("serie-a") || path.contains("ligue-1") || path.contains("champions-league")
            || path.contains("europa-league") || path.contains("world-cup") || path.contains("euro")
        {
            "tournament".to_string()
        } else {
            "other".to_string()
        }
    } else {
        "country".to_string()
    }
}

fn is_navigation_element(text: &str) -> bool {
    let nav_keywords = ["home", "login", "register", "signup", "account", "settings", 
        "help", "faq", "contact", "about", "terms", "privacy", "bet", "betting", 
        "sports", "live", "casino", "promotions"];
    
    let lower = text.to_lowercase();
    nav_keywords.iter().any(|&kw| lower.contains(kw))
}

fn extract_slug_from_url(url: &str) -> String {
    let path = url.trim_start_matches("/football/").trim_end_matches("/");
    path.split('/').last().unwrap_or(path).to_string()
}

#[inline(never)]
fn normalize_name(name: &str) -> String {
    // Use explicit array type to avoid type inference issues
    let special_names: [(&str, &str); 9] = [
        ("premier league", "Premier League"),
        ("la liga", "La Liga"),
        ("bundesliga", "Bundesliga"),
        ("serie a", "Serie A"),
        ("ligue 1", "Ligue 1"),
        ("champions league", "UEFA Champions League"),
        ("europa league", "UEFA Europa League"),
        ("world cup", "FIFA World Cup"),
        ("europe", "Europe"),
    ];
    
    let lower = name.to_lowercase().trim().to_string();
    for (pattern, replacement) in special_names.iter() {
        if lower == *pattern {
            return replacement.to_string();
        }
    }
    
    // Title case for other names
    let words: Vec<String> = lower
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect();
    
    words.join(" ")
}

fn parse_football_html_fallback(html: &str) -> Vec<FootballSubCategory> {
    let mut categories = Vec::new();
    
    let re = match regex::Regex::new(r#"<a\s+href="(/football/[^"]+/)"[^>]*>([^<]+)</a>"#) {
        Ok(r) => r,
        Err(_) => return categories,
    };
    
    let mut seen_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
    
    for cap in re.captures_iter(html) {
        if let (Some(url), Some(name)) = (cap.get(1), cap.get(2)) {
            let url_str = url.as_str();
            let name_str = name.as_str().trim();
            
            if is_navigation_element(name_str) || !is_valid_football_category(url_str) {
                continue;
            }
            
            let slug = extract_slug_from_url(url_str);
            if seen_slugs.contains(&slug) {
                continue;
            }
            seen_slugs.insert(slug.clone());
            
            let category_type = determine_category_type(url_str);
            let normalized_name = normalize_name(name_str);
            
            categories.push(FootballSubCategory {
                slug,
                name: normalized_name,
                url: url_str.to_string(),
                category_type,
            });
        }
    }
    
    categories
}

#[allow(dead_code)]
fn default_football_categories() -> Vec<FootballSubCategory> {
    vec![
        FootballSubCategory { slug: "popular".to_string(), name: "Popular".to_string(), url: String::new(), category_type: "popular".to_string() },
        FootballSubCategory { slug: "algeria".to_string(), name: "Algeria".to_string(), url: "/football/algeria/".to_string(), category_type: "country".to_string() },
        FootballSubCategory { slug: "argentina".to_string(), name: "Argentina".to_string(), url: "/football/argentina/".to_string(), category_type: "country".to_string() },
        FootballSubCategory { slug: "zimbabwe".to_string(), name: "Zimbabwe".to_string(), url: "/football/zimbabwe/".to_string(), category_type: "country".to_string() },
        FootballSubCategory { slug: "england-premier-league".to_string(), name: "England - Premier League".to_string(), url: "/football/england/premier-league/".to_string(), category_type: "tournament".to_string() },
        FootballSubCategory { slug: "champions-league".to_string(), name: "UEFA Champions League".to_string(), url: "/football/europe/champions-league/".to_string(), category_type: "tournament".to_string() },
    ]
}

#[derive(Debug)]
pub enum FootballScraperError {
    HttpError(HttpClientError),
    ParseError(String),
}

impl std::fmt::Display for FootballScraperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FootballScraperError::HttpError(e) => write!(f, "HTTP error: {}", e),
            FootballScraperError::ParseError(s) => write!(f, "Parse error: {}", s),
        }
    }
}

impl std::error::Error for FootballScraperError {}

impl From<HttpClientError> for FootballScraperError {
    fn from(err: HttpClientError) -> Self {
        FootballScraperError::HttpError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_country_name() {
        assert_eq!("Algeria", normalize_name("algeria"));
        assert_eq!("Argentina", normalize_name("argentina"));
        assert_eq!("Zimbabwe", normalize_name("zimbabwe"));
    }

    #[test]
    fn test_extract_slug_from_url() {
        assert_eq!("premier-league", extract_slug_from_url("/football/england/premier-league/"));
        assert_eq!("algeria", extract_slug_from_url("/football/algeria/"));
        assert_eq!("argentina", extract_slug_from_url("/football/argentina/"));
    }

    #[test]
    fn test_normalize_name() {
        assert_eq!("Premier League", normalize_name("premier league"));
        assert_eq!("La Liga", normalize_name("LA LIGA"));
        // These are the key tests - lowercase should return special name
        assert_eq!("UEFA Champions League", normalize_name("champions league"));
        assert_eq!("UEFA Champions League", normalize_name("Champions League"));
    }

    #[test]
    fn test_is_navigation_element() {
        assert!(is_navigation_element("Home"));
        assert!(is_navigation_element("Login"));
        assert!(!is_navigation_element("Premier League"));
        assert!(!is_navigation_element("Algeria"));
    }

    #[test]
    fn test_is_valid_football_category() {
        assert!(is_valid_football_category("/football/england/premier-league/"));
        assert!(is_valid_football_category("/football/algeria/"));
        assert!(is_valid_football_category("/football/argentina/"));
        assert!(!is_valid_football_category("/football/"));
        assert!(!is_valid_football_category("/football/search"));
        assert!(!is_valid_football_category("/football/api/hello"));
    }

    #[test]
    fn test_determine_category_type() {
        assert_eq!("tournament", determine_category_type("/football/england/premier-league/"));
        assert_eq!("country", determine_category_type("/football/algeria/"));
        assert_eq!("tournament", determine_category_type("/football/europe/champions-league/"));
    }

    #[test]
    fn test_default_football_categories() {
        let categories = default_football_categories();
        assert!(!categories.is_empty());
        assert!(categories.iter().any(|c| c.slug == "popular"));
        assert!(categories.iter().any(|c| c.slug == "algeria"));
        assert!(categories.iter().any(|c| c.slug == "argentina"));
        assert!(categories.iter().any(|c| c.slug == "zimbabwe"));
    }
}
