//! Polymarket Analysis Library

pub mod config;
pub mod http;
pub mod menu;

pub use config::{get_config, init_config, AppConfig};
pub use http::client::{HttpClient, HttpClientError};
pub use menu::models::{Category, CategoryData};
pub use menu::scraper::{refresh_category, get_category_or_default, get_cached_category};
pub use menu::storage::Storage;
pub use menu::handlers::{create_router, AppState};
