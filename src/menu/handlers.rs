//! Unified API handlers for menu and category data

use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use super::models::CategoryData;
use super::scraper::get_category_or_default;
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

pub(crate) async fn menu_handler(State(state): State<AppState>) -> Json<ApiResponse<CategoryData>> {
    // Try to load from storage first
    match state.storage.load("menu") {
        Ok(Some(data)) => {
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
        Ok(None) => {
            // No cached data, get default and save it
            let data = get_category_or_default("menu");
            let _ = state.storage.save("menu", &data);
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
        Err(e) => {
            // Storage error, return default
            tracing::warn!("Storage error loading menu: {}", e);
            let data = get_category_or_default("menu");
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
    }
}

pub(crate) async fn category_handler(
    Path(sport): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    // Try to load from storage first
    match state.storage.load(&sport) {
        Ok(Some(data)) => {
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
        Ok(None) => {
            // No cached data, get default and save it
            let data = get_category_or_default(&sport);
            let _ = state.storage.save(&sport, &data);
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
        Err(e) => {
            // Storage error, return default
            tracing::warn!("Storage error loading {}: {}", sport, e);
            let data = get_category_or_default(&sport);
            return Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            });
        }
    }
}

pub(crate) async fn menu_refresh_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let data = get_category_or_default("menu");
    match state.storage.save("menu", &data) {
        Ok(()) => {
            Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("Failed to save menu refresh: {}", e);
            Json(ApiResponse {
                ok: false,
                data: None,
                error: Some(format!("Failed to save: {}", e)),
            })
        }
    }
}

pub(crate) async fn category_refresh_handler(
    Path(sport): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<CategoryData>> {
    let data = get_category_or_default(&sport);
    match state.storage.save(&sport, &data) {
        Ok(()) => {
            Json(ApiResponse {
                ok: true,
                data: Some(data),
                error: None,
            })
        }
        Err(e) => {
            tracing::error!("Failed to save {} refresh: {}", sport, e);
            Json(ApiResponse {
                ok: false,
                data: None,
                error: Some(format!("Failed to save: {}", e)),
            })
        }
    }
}

pub(crate) async fn menu_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/menu.html"))
}

pub(crate) async fn category_page_handler() -> Html<&'static str> {
    Html(include_str!("../../public/menu.html"))
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
        .route("/api/menu/refresh", post(menu_refresh_handler))
        .route("/api/menu/:sport/refresh", post(category_refresh_handler))
        .route("/api/menu", get(menu_handler))
        .route("/api/menu/:sport", get(category_handler))
        .route("/menu", get(menu_page_handler))
        .route("/menu/:sport", get(category_page_handler))
        .with_state(state)
        .layer(cors)
}
