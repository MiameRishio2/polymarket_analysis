//! 应用程序入口点
//!
//! 启动 Axum HTTP 服务器，托管静态页面并提供 JSON API。

use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "polymarket_analysis=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 绑定端口
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = std::net::TcpListener::bind(addr).expect("无法绑定端口 8080");
    listener.set_nonblocking(true).expect("无法设置为非阻塞模式");

    // 启动服务器
    polymarket_analysis::handlers::run_server(listener).await;
}
