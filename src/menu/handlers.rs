//! Menu API handlers
//!
//! Defines HTTP API endpoints for menu and football operations.

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
use super::football::models::FootballData;
use super::football::scraper::scrape_football;
use super::football::progress;

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

/// POST /api/menu/refresh response structure
#[derive(Serialize)]
pub struct RefreshResponse {
    pub ok: bool,
    pub data: Option<MenuData>,
    pub error: Option<String>,
    pub message: String,
}

/// GET /api/football response structure
#[derive(Serialize)]
pub struct FootballResponse {
    pub ok: bool,
    pub data: Option<FootballData>,
    pub error: Option<String>,
}

/// POST /api/football/refresh response structure
#[derive(Serialize)]
pub struct FootballRefreshResponse {
    pub ok: bool,
    pub data: Option<FootballData>,
    pub error: Option<String>,
    pub message: String,
}

/// GET /api/football/progress response structure
#[derive(Serialize)]
pub struct FootballProgressResponse {
    pub in_progress: bool,
    pub stage: String,
    pub current_operation: String,
    pub percent: u32,
    pub total_items: u32,
    pub processed_items: u32,
    pub error: Option<String>,
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

/// GET /api/football Handler
///
/// Returns football sub-categories (leagues, tournaments, etc.)
pub async fn football_api_handler(State(state): State<Arc<AppState>>) -> Json<FootballResponse> {
    match scrape_football(&state.http_client).await {
        Ok(data) => Json(FootballResponse {
            ok: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => {
            tracing::warn!("Failed to fetch football data: {}", e);
            Json(FootballResponse {
                ok: false,
                data: None,
                error: Some(format!("获取足球数据失败: {}", e)),
            })
        }
    }
}

/// POST /api/football/refresh Handler
pub async fn football_refresh_handler(State(state): State<Arc<AppState>>) -> Json<FootballRefreshResponse> {
    info!("Football data refresh requested");
    
    // Start progress tracking
    progress::start_refresh("正在从 oddsportal.com 获取足球数据...").await;
    
    match scrape_football(&state.http_client).await {
        Ok(data) => {
            let count = data.categories.len();
            progress::update_progress("complete", &format!("获取到 {} 个分类", count), 100).await;
            progress::complete_refresh().await;
            
            Json(FootballRefreshResponse {
                ok: true,
                data: Some(data),
                error: None,
                message: format!("足球分类已刷新，获取到 {} 个子分类", count),
            })
        },
        Err(e) => {
            let err_msg = format!("获取足球数据失败: {}", e);
            tracing::error!("Football data refresh failed: {}", e);
            progress::fail_refresh(&err_msg).await;
            
            Json(FootballRefreshResponse {
                ok: false,
                data: None,
                error: Some(err_msg),
                message: "刷新失败".to_string(),
            })
        }
    }
}

/// GET /api/football/progress Handler
///
/// Returns the current progress of the football data refresh operation
pub async fn football_progress_handler() -> Json<FootballProgressResponse> {
    let p = progress::get_progress().await;
    Json(FootballProgressResponse {
        in_progress: p.in_progress,
        stage: p.stage,
        current_operation: p.current_operation,
        percent: p.percent,
        total_items: p.total_items,
        processed_items: p.processed_items,
        error: p.error,
    })
}

/// GET /menu Handler
pub async fn menu_handler() -> Html<&'static str> {
    Html(include_str!("../../public/menu.html"))
}

/// GET /menu/football Handler
pub async fn football_handler() -> Html<&'static str> {
    Html(include_str!("../../public/football.html"))
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
        // Menu API
        .route("/api/menu", axum::routing::get(menu_api_handler))
        .route("/api/menu/refresh", axum::routing::post(menu_refresh_handler))
        // Football API
        .route("/api/football", axum::routing::get(football_api_handler))
        .route("/api/football/refresh", axum::routing::post(football_refresh_handler))
        .route("/api/football/progress", axum::routing::get(football_progress_handler))
        // Static pages
        .route("/menu", axum::routing::get(menu_handler))
        .route("/menu/football", axum::routing::get(football_handler))
        .route("/menu/*path", axum::routing::get(menu_handler))
        .nest_service("/", ServeDir::new("public"))
        .with_state(state)
        .layer(cors);

    let bind_addr = format!("http://{}", addr);
    if addr.ip().is_loopback() {
        info!("🚀 Server listening on {}", bind_addr);
        info!("🌐 本地访问:    {}/", bind_addr);
        info!("🌐 菜单访问:   {}/menu", bind_addr);
        info!("🌐 足球访问:   {}/menu/football", bind_addr);
        info!("⚠️  注意: 当前仅允许本地访问，如需远程访问请修改 config.yaml 中 web.host 为 0.0.0.0");
    } else {
        info!("🚀 Server listening on {}", bind_addr);
        info!("🌐 前端:        {}/", bind_addr);
        info!("🌐 菜单:        {}/menu", bind_addr);
        info!("🌐 足球:        {}/menu/football", bind_addr);
        info!("📡 API Hello:   {}/api/hello", bind_addr);
        info!("📡 API Config:  {}/api/config", bind_addr);
        info!("📡 API Menu:    {}/api/menu", bind_addr);
        info!("📡 API Football: {}/api/football", bind_addr);
        info!("📡 API Football Progress: {}/api/football/progress", bind_addr);
        info!("✅ 远程访问已启用，其他机器可通过 {} 访问", bind_addr);
    }

    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
    axum::serve(listener, app).await.unwrap();
}
