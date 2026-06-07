//! Polymarket Analysis Library

pub mod config;
pub mod http;
pub mod menu;
pub mod sqlite;

// Modules for future expansion:
// pub mod analysis;

pub use config::{get_config, init_config, AppConfig};
pub use http::client::{HttpClient, HttpClientError};
pub use menu::models::{Category, CategoryData};
pub use menu::scraper::{get_category_or_default, get_category_data};
pub use menu::storage::Storage;
pub use menu::handlers::{create_router, AppState};
pub use sqlite::create_sqlite_router;
