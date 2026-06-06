//! API Handler 模块
//!
//! 定义所有 HTTP API 端点的处理逻辑。

use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// 共享应用状态（当前为空，后续可扩展）
#[derive(Clone)]
pub struct AppState {}

/// GET /api/hello 的响应结构
#[derive(Serialize)]
pub struct HelloResponse {
    pub message: String,
    pub timestamp: String,
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

/// 启动 HTTP 服务器
///
/// - 监听 `0.0.0.0:8080`
/// - 挂载静态文件服务（`public/` 目录）
/// - 注册路由：`/` → 静态文件，`/api/hello` → JSON API
pub async fn run_server(listener: std::net::TcpListener) {
    let state = Arc::new(AppState {});

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .route("/api/hello", axum::routing::get(hello_handler))
        .nest_service("/", tower_http::services::ServeDir::new("public"))
        .with_state(state)
        .layer(cors);

    info!("🚀 Server listening on http://127.0.0.1:8080");
    info!("🌐 Frontend: http://127.0.0.1:8080/");
    info!("📡 API:      http://127.0.0.1:8080/api/hello");

    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
    axum::serve(listener, app).await.unwrap();
}
