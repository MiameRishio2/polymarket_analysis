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
use super::scraper::{fetch_categories_for_sport, fetch_url};
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
        let slug = sport.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let name = capitalize(&slug);
        let url = sport.get("url").and_then(|s| s.as_str()).unwrap_or("/").to_string();
        
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
            return Json(ApiResponse { ok: true, data: Some(data), error: None });
        }
        Ok(None) | Err(_) => {
            let data = fetch_menu_data().await;
            let _ = state.storage.save("menu_menu", &data);
            return Json(ApiResponse { ok: true, data: Some(data), error: None });
        }
    }
}

pub(crate) async fn category_handler(
    Path(sport): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    // Try to load from storage first
    match state.storage.load(&format!("menu_{}", sport)) {
        Ok(Some(data)) => {
            return Json(ApiResponse { ok: true, data: Some(data), error: None });
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
            let _ = state.storage.save(&format!("menu_{}", sport), &data);
            return Json(ApiResponse { ok: true, data: Some(data), error: None });
        }
    }
}

pub(crate) async fn menu_refresh_handler(State(state): State<AppState>) -> Json<ApiResponse<CategoryData>> {
    let data = fetch_menu_data().await;
    let _ = state.storage.save("menu_menu", &data);
    Json(ApiResponse { ok: true, data: Some(data), error: None })
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
    let _ = state.storage.save(&format!("menu_{}", sport), &data);
    Json(ApiResponse { ok: true, data: Some(data), error: None })
}

// Page handlers
pub(crate) async fn index_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/index.html"))
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
    let state = AppState { storage: Arc::new(storage) };

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/", get(index_page_handler))
        .route("/api/menu/refresh", post(menu_refresh_handler))
        .route("/api/menu/:sport/refresh", post(category_refresh_handler))
        .route("/api/menu", get(menu_handler))
        .route("/api/menu/:sport", get(category_handler))
        .route("/menu", get(menu_page_handler))
        .route("/menu/:sport", get(menu_page_handler))
        .route("/analysis", get(analysis_page_handler))
        .route("/sqlite", get(sqlite_page_handler))
        .with_state(state)
        .layer(cors)
}
