//! API Handler 模块
//!
//! 定义所有 HTTP API 端点的处理逻辑。

use axum::{extract::State, Json, response::Html};
use chrono::Utc;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

/// 共享应用状态
#[derive(Clone)]
pub struct AppState {
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub proxy_enabled: bool,
    pub remote_access_enabled: bool,
}

impl AppState {
    pub fn new() -> Self {
        let config = crate::get_config();
        Self {
            oddsportal_url: config.oddsportal_url().to_string(),
            polymarket_url: config.polymarket_url().to_string(),
            proxy_enabled: config.proxy_enabled,
            remote_access_enabled: config.is_remote_access_enabled(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// GET /api/hello 的响应结构
#[derive(Serialize)]
pub struct HelloResponse {
    pub message: String,
    pub timestamp: String,
}

/// GET /api/config 的响应结构
#[derive(Serialize)]
pub struct ConfigResponse {
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub proxy_enabled: bool,
    pub remote_access_enabled: bool,
}

/// GET /api/hello Handler
///
/// 返回简单的 JSON 问候响应，包含当前 UTC 时间戳。
pub async fn hello_handler(State(_state): State<Arc<AppState>>) -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello, World!".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

/// GET /api/config Handler
///
/// 返回当前配置信息。
pub async fn config_handler(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        oddsportal_url: state.oddsportal_url.clone(),
        polymarket_url: state.polymarket_url.clone(),
        proxy_enabled: state.proxy_enabled,
        remote_access_enabled: state.remote_access_enabled,
    })
}

/// GET /menu Handler
///
/// 返回体育菜单页面。
pub async fn menu_handler() -> Html<&'static str> {
    Html(include_str!("../public/menu.html"))
}

/// 启动 HTTP 服务器
///
/// - 监听 `<host>:<port>`
/// - 挂载静态文件服务（`public/` 目录）
/// - 注册路由：
///   - `/` → 静态文件
///   - `/menu` → 体育菜单页面
///   - `/menu/<path>` → 体育菜单子页面
///   - `/api/hello` → JSON API
///   - `/api/config` → 配置信息 API
pub async fn run_server(listener: std::net::TcpListener, addr: SocketAddr) {
    let state = Arc::new(AppState::new());
    let _port = addr.port();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/api/hello", axum::routing::get(hello_handler))
        .route("/api/config", axum::routing::get(config_handler))
        .route("/menu", axum::routing::get(menu_handler))
        .route("/menu/*path", axum::routing::get(menu_handler))
        .nest_service("/", ServeDir::new("public"))
        .with_state(state)
        .layer(cors);

    // 根据绑定地址显示不同的访问说明
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
        info!("✅ 远程访问已启用，其他机器可通过 {} 访问", bind_addr);
    }

    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
    axum::serve(listener, app).await.unwrap();
}
