//! Unified scraper for menu and category data from OddsPortal

use crate::menu::models::{Category, CategoryData};
use reqwest::header::ACCEPT_ENCODING;
use reqwest::{Client, Proxy};
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

/// Create HTTP client with proxy support from config
fn create_scraper_client() -> Result<Client, ScraperError> {
    let builder = create_raw_body_client_builder();

    // Try to load config and use proxy
    if let Ok(config) = crate::config::load_config("config.yaml") {
        if let Some(proxy_url) = config.proxy_url() {
            match create_scraper_proxy(&proxy_url) {
                Ok(proxy) => {
                    tracing::info!("Scraper using all-scheme proxy: {}", proxy_url);
                    return builder
                        .proxy(proxy)
                        .build()
                        .map_err(|e| ScraperError::Network(e.to_string()));
                }
                Err(e) => {
                    tracing::warn!("Failed to configure proxy: {}, continuing without", e);
                }
            }
        }
    } else {
        tracing::warn!("Failed to load config for proxy, scraper will not use proxy");
    }

    builder
        .build()
        .map_err(|e| ScraperError::Network(e.to_string()))
}

fn create_scraper_proxy(proxy_url: &str) -> Result<Proxy, ScraperError> {
    Proxy::all(proxy_url).map_err(|e| ScraperError::Network(e.to_string()))
}

fn create_raw_body_client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
}

/// HTTP client for scraping (lazy loaded with proxy)
static CLIENT: once_cell::sync::Lazy<Result<Client, ScraperError>> =
    once_cell::sync::Lazy::new(|| create_scraper_client());

/// Fetch main page to get sport list
pub async fn fetch_sports() -> Result<Vec<Category>, ScraperError> {
    let url = "https://www.oddsportal.com/";
    let html = fetch_url(url).await?;

    let sport_data = extract_sport_data(&html)?;

    let mut categories = Vec::new();
    for (_key, sport) in sport_data {
        let slug = sport
            .get("name")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let name = capitalize_first(&slug);
        let url = sport
            .get("url")
            .and_then(|s| s.as_str())
            .unwrap_or("/")
            .to_string();

        categories.push(Category {
            slug,
            name,
            url,
            category_type: None,
        });
    }

    Ok(categories)
}

/// Fetch URL and return HTML content
pub async fn fetch_url(url: &str) -> Result<String, ScraperError> {
    let client = CLIENT
        .as_ref()
        .map_err(|e| ScraperError::Network(e.to_string()))?;

    fetch_url_with_client(client, url).await
}

async fn fetch_url_with_client(client: &Client, url: &str) -> Result<String, ScraperError> {
    let response = client
        .get(url)
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|e| ScraperError::Network(e.to_string()))?;

    // Check for redirect to different domain
    let final_url = response.url().to_string();
    let original_host = url
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("");
    if final_url != url && !final_url.contains(&original_host) {
        tracing::warn!("Request to {} redirected to {}", url, final_url);
    }

    // Get raw bytes and decode with lossy conversion for any encoding
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ScraperError::Network(format!("Failed to read body: {}", e)))?;

    Ok(String::from_utf8_lossy(&bytes).into_owned())
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

    Err(ScraperError::Parse(
        "Could not find sport-data in HTML".to_string(),
    ))
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

    let html = fetch_url(&url).await?;

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

    // Log each category found for debugging
    tracing::debug!("Categories found for {}:", sport);
    for cat in &categories {
        tracing::debug!("  - {} ({})", cat.name, cat.slug);
    }

    categories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(categories)
}

/// Fetch a nested category page and extract direct child links like
/// /football/argentina/primera-nacional/
pub async fn fetch_categories_for_path(
    sport: &str,
    category: &str,
) -> Result<Vec<Category>, ScraperError> {
    let url = format!("https://www.oddsportal.com/{}/{}/", sport, category);

    tracing::info!("Fetching child categories from: {}", url);

    let html = fetch_url(&url).await?;
    extract_categories_for_path(&html, sport, category)
}

/// Extract direct child categories from HTML for a nested sport category path.
pub fn extract_categories_for_path(
    html: &str,
    sport: &str,
    category: &str,
) -> Result<Vec<Category>, ScraperError> {
    let document = scraper::Html::parse_document(html);
    let link_selector = Selector::parse("a[href]")
        .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;

    let mut categories = Vec::new();

    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            if let Some(slug) = extract_child_category_slug(href, sport, category) {
                if EXCLUDED_PATHS.contains(&slug.to_lowercase().as_str()) {
                    continue;
                }

                if categories.iter().any(|c: &Category| c.slug == slug) {
                    continue;
                }

                categories.push(Category {
                    name: format_name_from_slug(&slug),
                    url: format!("/{}/{}/{}/", sport, category, slug),
                    slug,
                    category_type: Some("league".to_string()),
                });
            }
        }
    }

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

fn extract_child_category_slug(href: &str, sport: &str, category: &str) -> Option<String> {
    let href = href.trim_end_matches('/');
    let base_pattern = format!("/{}/{}/", sport, category);

    if !href.starts_with(&base_pattern) {
        return None;
    }

    let remaining = &href[base_pattern.len()..];

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
            // Return empty data on error
            CategoryData {
                sport: sport.to_string(),
                categories: Vec::new(),
                last_updated: get_timestamp(),
                source: "error".to_string(),
            }
        }
    }
}

/// Get empty category data (fallback when scraping fails)
pub fn get_category_or_default(_sport: &str) -> CategoryData {
    CategoryData {
        sport: _sport.to_string(),
        categories: Vec::new(),
        last_updated: get_timestamp(),
        source: "error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::{timeout, Duration};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn scraper_proxy_is_used_for_https_requests() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("local proxy listener should bind");
        let proxy_url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("proxy should receive a connection");
            let mut buffer = [0_u8; 256];
            let bytes_read = stream
                .read(&mut buffer)
                .await
                .expect("proxy should read request bytes");
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).into_owned();
            let _ = tx.send(request);
            let _ = stream
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await;
        });

        let proxy = create_scraper_proxy(&proxy_url).expect("proxy should configure");
        let client = create_raw_body_client_builder()
            .proxy(proxy)
            .timeout(Duration::from_secs(2))
            .build()
            .expect("scraper client should build");

        let request = tokio::spawn(async move {
            let _ = client
                .get("https://www.oddsportal.com/football/")
                .send()
                .await;
        });

        let proxy_request = timeout(Duration::from_secs(2), rx)
            .await
            .expect("HTTPS request should reach configured proxy")
            .expect("proxy request should be captured");
        request.abort();

        assert!(
            proxy_request.starts_with("CONNECT www.oddsportal.com:443"),
            "expected HTTPS CONNECT through proxy, got: {proxy_request:?}"
        );
    }

    #[tokio::test]
    async fn fetch_url_with_client_does_not_decode_bad_gzip_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/football/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Encoding", "gzip")
                    .set_body_string("<html>plain body mislabeled as gzip</html>"),
            )
            .mount(&server)
            .await;

        let client = create_raw_body_client_builder()
            .build()
            .expect("scraper client should build");
        fetch_url_with_client(&client, &format!("{}/football/", server.uri()))
            .await
            .expect("mislabeled gzip response should not fail while reading body");
    }
}
