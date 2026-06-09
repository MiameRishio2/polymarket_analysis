//! Unified API handlers for menu and category data

use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use super::models::{Category, CategoryData};
use super::scraper::{
    fetch_categories_for_league_path, fetch_categories_for_path, fetch_categories_for_sport,
    fetch_url,
};
use super::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
}

#[derive(serde::Serialize)]
pub(crate) struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

fn ok_response<T>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

/// Fetch sports list from OddsPortal homepage
async fn fetch_menu_data() -> CategoryData {
    let html = match fetch_url("https://www.oddsportal.com/").await {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("Failed to fetch homepage: {}", e);
            return CategoryData {
                sport: "menu".to_string(),
                categories: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            };
        }
    };

    // Extract sport-data from HTML
    let sport_data = match extract_sport_data(&html) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse sport-data: {}", e);
            return CategoryData {
                sport: "menu".to_string(),
                categories: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            };
        }
    };

    let mut categories = Vec::new();
    for (_key, sport) in sport_data {
        let slug = sport
            .get("name")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let name = capitalize(&slug);
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

    CategoryData {
        sport: "menu".to_string(),
        categories,
        last_updated: chrono::Utc::now().to_rfc3339(),
        source: "scraped".to_string(),
    }
}

fn extract_sport_data(html: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    // Find sport-data="{...}"
    let start = html.find("sport-data=\"{").ok_or("No sport-data found")?;
    let data_start = start + 12;
    let end = html[data_start..].find('"').ok_or("No end quote")?;
    let json_str = &html[data_start..data_start + end];

    // Decode HTML entities
    let decoded = json_str.replace("&quot;", "\"");

    serde_json::from_str(&decoded).map_err(|e| e.to_string())
}

fn capitalize(s: &str) -> String {
    s.split(' ')
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

pub(crate) async fn menu_handler(State(state): State<AppState>) -> Json<ApiResponse<CategoryData>> {
    // Try to load from storage first
    match state.storage.load("menu_menu") {
        Ok(Some(data)) => {
            return ok_response(data);
        }
        Ok(None) | Err(_) => {
            let data = fetch_menu_data().await;
            let _ = state.storage.save("menu_menu", &data);
            return ok_response(data);
        }
    }
}

pub(crate) async fn category_handler(
    Path(sport): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    // Try to load from storage first
    match state.storage.load(&format!("menu_{}", sport)) {
        Ok(Some(mut data)) if is_usable_cached_category_data(&data) => {
            data.sport = sport.clone();
            return ok_response(data);
        }
        Ok(None) | Err(_) => {
            let data = match fetch_categories_for_sport(&sport).await {
                Ok(cats) => CategoryData {
                    sport: sport.clone(),
                    categories: cats,
                    last_updated: chrono::Utc::now().to_rfc3339(),
                    source: "scraped".to_string(),
                },
                Err(e) => {
                    tracing::error!("Failed to fetch {}: {}", sport, e);
                    CategoryData {
                        sport: sport.clone(),
                        categories: Vec::new(),
                        last_updated: chrono::Utc::now().to_rfc3339(),
                        source: "error".to_string(),
                    }
                }
            };
            if should_persist_category_data(&data) {
                let _ = state.storage.save(&format!("menu_{}", sport), &data);
            }
            return ok_response(data);
        }
        Ok(Some(_)) => {
            let data = match fetch_categories_for_sport(&sport).await {
                Ok(cats) => CategoryData {
                    sport: sport.clone(),
                    categories: cats,
                    last_updated: chrono::Utc::now().to_rfc3339(),
                    source: "scraped".to_string(),
                },
                Err(e) => {
                    tracing::error!("Failed to refresh stale {} cache: {}", sport, e);
                    CategoryData {
                        sport: sport.clone(),
                        categories: Vec::new(),
                        last_updated: chrono::Utc::now().to_rfc3339(),
                        source: "error".to_string(),
                    }
                }
            };
            if should_persist_category_data(&data) {
                let _ = state.storage.save(&format!("menu_{}", sport), &data);
            }
            return ok_response(data);
        }
    }
}

fn is_usable_cached_category_data(data: &CategoryData) -> bool {
    !data.categories.is_empty()
}

fn should_persist_category_data(data: &CategoryData) -> bool {
    data.source != "error" && !data.categories.is_empty()
}

fn nested_cache_key(segments: &[&str]) -> String {
    format!("menu_{}", segments.join("_"))
}

fn nested_sport_key(segments: &[&str]) -> String {
    segments.join("/")
}

fn third_level_cache_key(sport: &str, category: &str) -> String {
    nested_cache_key(&[sport, category])
}

fn third_level_sport_key(sport: &str, category: &str) -> String {
    nested_sport_key(&[sport, category])
}

fn fourth_level_cache_key(sport: &str, category: &str, league: &str) -> String {
    nested_cache_key(&[sport, category, league])
}

fn fourth_level_sport_key(sport: &str, category: &str, league: &str) -> String {
    nested_sport_key(&[sport, category, league])
}

pub(crate) async fn category_child_handler(
    Path((sport, category)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let cache_key = third_level_cache_key(&sport, &category);

    match state.storage.load(&cache_key) {
        Ok(Some(mut data)) => {
            data.sport = third_level_sport_key(&sport, &category);
            ok_response(data)
        }
        Ok(None) | Err(_) => {
            let data = match fetch_categories_for_path(&sport, &category).await {
                Ok(cats) => CategoryData {
                    sport: third_level_sport_key(&sport, &category),
                    categories: cats,
                    last_updated: chrono::Utc::now().to_rfc3339(),
                    source: "scraped".to_string(),
                },
                Err(e) => {
                    tracing::error!("Failed to fetch {}/{}: {}", sport, category, e);
                    CategoryData {
                        sport: third_level_sport_key(&sport, &category),
                        categories: Vec::new(),
                        last_updated: chrono::Utc::now().to_rfc3339(),
                        source: "error".to_string(),
                    }
                }
            };
            if should_persist_category_data(&data) {
                let _ = state.storage.save(&cache_key, &data);
            }
            ok_response(data)
        }
    }
}

pub(crate) async fn category_grandchild_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let cache_key = fourth_level_cache_key(&sport, &category, &league);

    match state.storage.load(&cache_key) {
        Ok(Some(mut data)) => {
            data.sport = fourth_level_sport_key(&sport, &category, &league);
            ok_response(data)
        }
        Ok(None) | Err(_) => {
            let data = match fetch_categories_for_league_path(&sport, &category, &league).await {
                Ok(cats) => CategoryData {
                    sport: fourth_level_sport_key(&sport, &category, &league),
                    categories: cats,
                    last_updated: chrono::Utc::now().to_rfc3339(),
                    source: "scraped".to_string(),
                },
                Err(e) => {
                    tracing::error!("Failed to fetch {}/{}/{}: {}", sport, category, league, e);
                    CategoryData {
                        sport: fourth_level_sport_key(&sport, &category, &league),
                        categories: Vec::new(),
                        last_updated: chrono::Utc::now().to_rfc3339(),
                        source: "error".to_string(),
                    }
                }
            };
            if should_persist_category_data(&data) {
                let _ = state.storage.save(&cache_key, &data);
            }
            ok_response(data)
        }
    }
}

pub(crate) async fn menu_refresh_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let data = fetch_menu_data().await;
    let _ = state.storage.save("menu_menu", &data);
    ok_response(data)
}

pub(crate) async fn category_refresh_handler(
    Path(sport): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let data = match fetch_categories_for_sport(&sport).await {
        Ok(cats) => CategoryData {
            sport: sport.clone(),
            categories: cats,
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!("Failed to fetch {}: {}", sport, e);
            CategoryData {
                sport: sport.clone(),
                categories: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            }
        }
    };
    if should_persist_category_data(&data) {
        let _ = state.storage.save(&format!("menu_{}", sport), &data);
    }
    ok_response(data)
}

pub(crate) async fn category_child_refresh_handler(
    Path((sport, category)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let cache_key = third_level_cache_key(&sport, &category);
    let data = match fetch_categories_for_path(&sport, &category).await {
        Ok(cats) => CategoryData {
            sport: third_level_sport_key(&sport, &category),
            categories: cats,
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!("Failed to refresh {}/{}: {}", sport, category, e);
            CategoryData {
                sport: third_level_sport_key(&sport, &category),
                categories: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            }
        }
    };
    if should_persist_category_data(&data) {
        let _ = state.storage.save(&cache_key, &data);
    }
    ok_response(data)
}

// Page handlers
pub(crate) async fn index_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/index.html"))
}

pub(crate) async fn category_grandchild_refresh_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let cache_key = fourth_level_cache_key(&sport, &category, &league);
    let data = match fetch_categories_for_league_path(&sport, &category, &league).await {
        Ok(cats) => CategoryData {
            sport: fourth_level_sport_key(&sport, &category, &league),
            categories: cats,
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!("Failed to refresh {}/{}/{}: {}", sport, category, league, e);
            CategoryData {
                sport: fourth_level_sport_key(&sport, &category, &league),
                categories: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            }
        }
    };
    if should_persist_category_data(&data) {
        let _ = state.storage.save(&cache_key, &data);
    }
    ok_response(data)
}

pub(crate) async fn menu_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/menu.html"))
}

pub(crate) async fn analysis_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/analysis.html"))
}

pub(crate) async fn sqlite_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/sqlite.html"))
}

pub fn create_router(storage: Storage) -> Router {
    let state = AppState {
        storage: Arc::new(storage),
    };

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/", get(index_page_handler))
        .route("/api/menu/refresh", post(menu_refresh_handler))
        .route(
            "/api/menu/:sport/:category/:league/refresh",
            post(category_grandchild_refresh_handler),
        )
        .route(
            "/api/menu/:sport/:category/refresh",
            post(category_child_refresh_handler),
        )
        .route("/api/menu/:sport/refresh", post(category_refresh_handler))
        .route("/api/menu", get(menu_handler))
        .route(
            "/api/menu/:sport/:category/:league",
            get(category_grandchild_handler),
        )
        .route("/api/menu/:sport/:category", get(category_child_handler))
        .route("/api/menu/:sport", get(category_handler))
        .route("/menu", get(menu_page_handler))
        .route("/menu/:sport/:category/:league/", get(menu_page_handler))
        .route("/menu/:sport/:category/:league", get(menu_page_handler))
        .route("/menu/:sport/:category/", get(menu_page_handler))
        .route("/menu/:sport/:category", get(menu_page_handler))
        .route("/menu/:sport/", get(menu_page_handler))
        .route("/menu/:sport", get(menu_page_handler))
        .route("/analysis", get(analysis_page_handler))
        .route("/sqlite", get(sqlite_page_handler))
        .with_state(state)
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_cached_category_data_is_not_usable() {
        let data = CategoryData {
            sport: "menu_football".to_string(),
            categories: Vec::new(),
            last_updated: "2026-06-08T00:00:00Z".to_string(),
            source: "scraped".to_string(),
        };

        assert!(!is_usable_cached_category_data(&data));
    }

    #[test]
    fn empty_error_category_data_is_not_persistable() {
        let data = CategoryData {
            sport: "football".to_string(),
            categories: Vec::new(),
            last_updated: "2026-06-08T00:00:00Z".to_string(),
            source: "error".to_string(),
        };

        assert!(!should_persist_category_data(&data));
    }
}
