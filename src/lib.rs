//! Polymarket Analysis Library

pub mod config;
pub mod http;
pub mod menu;

pub use config::{get_config, init_config, AppConfig};
pub use http::{HttpClient, HttpClientError};
pub use menu::{
    MenuData, SportCategory, Storage,
    get_cached_menu, get_menu_or_default, refresh_menu, init_menu_from_storage, init_storage,
};
pub use menu::storage::StorageError;
pub use menu::football::{FootballData, FootballSubCategory};
