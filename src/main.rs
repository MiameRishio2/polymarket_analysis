//! Polymarket Analysis - Web Server

use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tracing::info;

mod config;
mod http;
mod menu;

use menu::handlers::create_router;
use menu::storage::Storage;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let config = config::load_config("config.yaml").expect("Failed to load config");
    
    let addr: SocketAddr = format!("{}:{}", config.web.host, config.web.port)
        .parse()
        .expect("Invalid address");
    
    info!("Starting server on {}", addr);
    
    // Initialize storage
    let db_path: PathBuf = "data/menu.db".into();
    let storage = Storage::new(&db_path).expect("Failed to initialize storage");
    
    let app = create_router(storage);
    
    let listener = TcpListener::bind(&addr).await.unwrap();
    
    axum::serve(listener, app).await.unwrap();
}
