//! Unified API handlers for menu and category data

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Json,
    routing::get,
    Router,
};
use tower_http::cors::CorsLayer;

use crate::config;
use crate::http::HttpClient;
use super::models::CategoryData;
use super::scraper::get_category_or_default;

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub config: config::AppConfig,
    pub http_client: HttpClient,
}

impl AppState {
    pub fn new() -> Self {
        let config = config::load_config("config.yaml").expect("Failed to load config");
        let http_client = HttpClient::new(&config).expect("Failed to create HTTP client");
        Self { config, http_client }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// API Response types
#[derive(serde::Serialize)]
struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

/// GET /api/menu Handler
pub async fn menu_handler() -> Json<ApiResponse<CategoryData>> {
    let data = get_category_or_default("menu");
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

/// GET /api/menu/:sport Handler
pub async fn category_handler(Path(sport): Path<String>) -> Json<ApiResponse<CategoryData>> {
    let data = get_category_or_default(&sport);
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

/// POST /api/menu/refresh Handler
pub async fn menu_refresh_handler(State(state): State<Arc<AppState>>) -> Json<ApiResponse<CategoryData>> {
    match super::scraper::refresh_category(&state.http_client, "menu").await {
        Ok(data) => Json(ApiResponse {
            ok: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            ok: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

/// POST /api/menu/:sport/refresh Handler
pub async fn category_refresh_handler(
    Path(sport): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<CategoryData>> {
    match super::scraper::refresh_category(&state.http_client, &sport).await {
        Ok(data) => Json(ApiResponse {
            ok: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            ok: false,
            data: None,
            error: Some(e.to_string()),
        }),
    }
}

/// GET /menu Handler - serves the unified menu page
pub async fn menu_page_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../../public/menu.html"))
}

/// GET /menu/:sport Handler - serves the unified menu page
pub async fn category_page_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("../../public/menu.html"))
}

/// Create the router with all routes
pub fn create_router() -> Router {
    let state = Arc::new(AppState::new());
    
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        // API endpoints
        .route("/api/menu", get(menu_handler))
        .route("/api/menu/:sport", get(category_handler))
        .route("/api/menu/refresh", axum::routing::post(menu_refresh_handler))
        .route("/api/menu/:sport/refresh", axum::routing::post(category_refresh_handler))
        // Page routes
        .route("/menu", get(menu_page_handler))
        .route("/menu/:sport", get(category_page_handler))
        // Static files
        .nest_service("/", tower_http::services::ServeDir::new("public"))
        .with_state(state)
        .layer(cors)
}
