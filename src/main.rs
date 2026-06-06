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

    // 加载配置文件
    let config_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("config.yaml");
    
    polymarket_analysis::init_config(&config_path).expect("无法加载配置文件 config.yaml");
    tracing::info!("✅ 配置已加载: {:?}", config_path);
    
    let config = polymarket_analysis::get_config();
    
    // 代理配置
    let proxy_status = if config.proxy_enabled {
        format!("✅ 启用 (proxy={})", config.proxy)
    } else {
        "❌ 禁用".to_string()
    };
    tracing::info!("📌 代理配置: {}", proxy_status);
    
    // 远程访问配置
    let remote_status = if config.is_remote_access_enabled() {
        "✅ 支持远程访问"
    } else {
        "⚠️ 仅本地访问"
    };
    tracing::info!("📌 远程访问: {}", remote_status);
    tracing::info!("📌 绑定地址: {}:{}", config.web_host(), config.web_port());
    tracing::info!("📌 Oddsportal URL: {}", config.oddsportal_url());
    tracing::info!("📌 Polymarket URL: {}", config.polymarket_url());

    // 从配置读取绑定地址和端口
    let addr: SocketAddr = format!("{}:{}", config.web_host(), config.web_port())
        .parse()
        .expect(&format!("无法解析地址 {}:{}", config.web_host(), config.web_port()));
    
    let listener = std::net::TcpListener::bind(addr).expect(&format!("无法绑定地址 {}", addr));
    listener.set_nonblocking(true).expect("无法设置为非阻塞模式");

    // 启动服务器
    polymarket_analysis::handlers::run_server(listener, addr).await;
}
