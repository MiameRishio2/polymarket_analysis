use reqwest::Client;
use scraper::Selector;

const EXCLUDED_PATHS: &[&str] = &["results", "standings", "live", "archive"];

#[tokio::main]
async fn main() {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
    
    let url = "https://www.oddsportal.com/football/";
    let response = client.get(url).send().await.unwrap();
    let html = response.text().await.unwrap();
    
    let document = scraper::Html::parse_document(&html);
    let link_selector = Selector::parse("a[href]").unwrap();
    
    let base_pattern = "/football/";
    let mut categories = Vec::new();
    
    for element in document.select(&link_selector) {
        if let Some(href) = element.value().attr("href") {
            if let Some(slug) = extract_category_slug(href, base_pattern) {
                if EXCLUDED_PATHS.contains(&slug.to_lowercase().as_str()) {
                    continue;
                }
                if categories.contains(&slug) {
                    continue;
                }
                categories.push(slug);
            }
        }
    }
    
    categories.sort();
    println!("Found {} categories:", categories.len());
    for c in categories.iter().take(20) {
        println!("  - {}", c);
    }
    println!("  ... and {} more", categories.len() - 20);
}

fn extract_category_slug(href: &str, base_pattern: &str) -> Option<String> {
    let href = href.trim_end_matches('/');
    if !href.starts_with(base_pattern) {
        return None;
    }
    let remaining = &href[base_pattern.len()..];
    if remaining.is_empty() || remaining.contains('/') {
        return None;
    }
    Some(remaining.to_string())
}
