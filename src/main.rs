//! Polymarket Analysis - Web Server
//!
//! Provides HTTP API for scraping and displaying sports/category data.

use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

mod config;
mod http;
mod menu;

use menu::handlers::create_router;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Load config
    let config = config::load_config("config.yaml").expect("Failed to load config");
    
    // Determine bind address
    let addr: SocketAddr = format!("{}:{}", config.web.host, config.web.port)
        .parse()
        .expect("Invalid address");
    
    info!("Starting server on {}", addr);
    
    // Create router
    let app = create_router();
    
    // Start server
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
