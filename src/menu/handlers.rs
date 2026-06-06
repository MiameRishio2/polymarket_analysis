//! Menu API handlers
//!
//! Defines HTTP API endpoints for menu operations.

use axum::{extract::State, Json, response::Html};
use chrono::Utc;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

use crate::http::HttpClient;
use super::models::MenuData;
use super::scraper::{get_menu_or_default, get_cached_menu, refresh_menu, init_menu_from_storage, init_storage};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub proxy_enabled: bool,
    pub remote_access_enabled: bool,
    pub http_client: HttpClient,
}

impl AppState {
    pub fn new() -> Self {
        let config = crate::get_config();
        let http_client = HttpClient::new(config).expect("Failed to create HTTP client");
        
        Self {
            oddsportal_url: config.oddsportal_url().to_string(),
            polymarket_url: config.polymarket_url().to_string(),
            proxy_enabled: config.proxy_enabled,
            remote_access_enabled: config.is_remote_access_enabled(),
            http_client,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// GET /api/hello response structure
#[derive(Serialize)]
pub struct HelloResponse {
    pub message: String,
    pub timestamp: String,
}

/// GET /api/config response structure
#[derive(Serialize)]
pub struct ConfigResponse {
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub proxy_enabled: bool,
    pub remote_access_enabled: bool,
}

/// GET /api/menu response structure
#[derive(Serialize)]
pub struct MenuResponse {
    pub ok: bool,
    pub data: Option<MenuData>,
    pub error: Option<String>,
}

/// GET /api/menu/refresh response structure
#[derive(Serialize)]
pub struct RefreshResponse {
    pub ok: bool,
    pub data: Option<MenuData>,
    pub error: Option<String>,
    pub message: String,
}

/// GET /api/hello Handler
pub async fn hello_handler(State(_state): State<Arc<AppState>>) -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello, World!".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

/// GET /api/config Handler
pub async fn config_handler(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        oddsportal_url: state.oddsportal_url.clone(),
        polymarket_url: state.polymarket_url.clone(),
        proxy_enabled: state.proxy_enabled,
        remote_access_enabled: state.remote_access_enabled,
    })
}

/// GET /api/menu Handler
///
/// Returns sports menu data, priority from SQLite cache.
pub async fn menu_api_handler(State(state): State<Arc<AppState>>) -> Json<MenuResponse> {
    // Return cached data first (including from SQLite)
    if let Some(data) = get_cached_menu() {
        return Json(MenuResponse {
            ok: true,
            data: Some(data),
            error: None,
        });
    }
    
    // If no cached data, scrape on initialization
    match refresh_menu(&state.http_client).await {
        Ok(data) => Json(MenuResponse {
            ok: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => {
            tracing::warn!("Failed to fetch menu: {}, using default", e);
            Json(MenuResponse {
                ok: true,
                data: Some(get_menu_or_default()),
                error: Some(format!("使用默认数据: {}", e)),
            })
        }
    }
}

/// POST /api/menu/refresh Handler
pub async fn menu_refresh_handler(State(state): State<Arc<AppState>>) -> Json<RefreshResponse> {
    info!("Menu refresh requested");
    
    match refresh_menu(&state.http_client).await {
        Ok(data) => Json(RefreshResponse {
            ok: true,
            data: Some(data.clone()),
            error: None,
            message: format!("菜单已刷新，获取到 {} 个体育分类", data.sports.len()),
        }),
        Err(e) => {
            tracing::error!("Menu refresh failed: {}", e);
            Json(RefreshResponse {
                ok: false,
                data: get_cached_menu(),
                error: Some(format!("刷新失败: {}", e)),
                message: "刷新失败，使用缓存数据".to_string(),
            })
        }
    }
}

/// GET /menu Handler
pub async fn menu_handler() -> Html<&'static str> {
    Html(include_str!("../../public/menu.html"))
}

/// Initialize storage and menu data
fn init_menu_storage() {
    // Initialize SQLite storage using scraper's init_storage
    // This sets the global STORAGE_INSTANCE for use by update_cache()
    if let Err(e) = init_storage(None) {
        tracing::warn!("Failed to initialize storage: {}, continuing without persistent cache", e);
        return;
    }
    
    // Try loading existing menu data from SQLite to memory cache
    if init_menu_from_storage().is_some() {
        tracing::info!("Menu data loaded from SQLite on startup");
    } else {
        tracing::info!("No existing menu data in SQLite, will fetch on first request");
    }
}

/// Run HTTP server
pub async fn run_server(listener: std::net::TcpListener, addr: SocketAddr) {
    // Initialize storage and menu data
    init_menu_storage();
    
    let state = Arc::new(AppState::new());
    let _port = addr.port();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/api/hello", axum::routing::get(hello_handler))
        .route("/api/config", axum::routing::get(config_handler))
        .route("/api/menu", axum::routing::get(menu_api_handler))
        .route("/api/menu/refresh", axum::routing::post(menu_refresh_handler))
        .route("/menu", axum::routing::get(menu_handler))
        .route("/menu/*path", axum::routing::get(menu_handler))
        .nest_service("/", ServeDir::new("public"))
        .with_state(state)
        .layer(cors);

    let bind_addr = format!("http://{}", addr);
    if addr.ip().is_loopback() {
        info!("🚀 Server listening on {}", bind_addr);
        info!("🌐 本地访问:    {}/", bind_addr);
        info!("🌐 菜单访问:   {}/menu", bind_addr);
        info!("⚠️  注意: 当前仅允许本地访问，如需远程访问请修改 config.yaml 中 web.host 为 0.0.0.0");
    } else {
        info!("🚀 Server listening on {}", bind_addr);
        info!("🌐 前端:        {}/", bind_addr);
        info!("🌐 菜单:        {}/menu", bind_addr);
        info!("📡 API Hello:   {}/api/hello", bind_addr);
        info!("📡 API Config:  {}/api/config", bind_addr);
        info!("📡 API Menu:    {}/api/menu", bind_addr);
        info!("📡 API Refresh: POST {}/api/menu/refresh", bind_addr);
        info!("✅ 远程访问已启用，其他机器可通过 {} 访问", bind_addr);
    }

    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
    axum::serve(listener, app).await.unwrap();
}
