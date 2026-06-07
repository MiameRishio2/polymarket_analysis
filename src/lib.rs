//! Polymarket Analysis Library

pub mod config;
pub mod http;
pub mod menu;

pub use config::{get_config, init_config, AppConfig};
pub use http::{HttpClient, HttpClientError};
pub use menu::{
    Category, CategoryData, MenuData, SportCategory,
    Storage, StorageError,
    init_storage, refresh_category, get_category_or_default, get_cached_category,
    AppState,
};
