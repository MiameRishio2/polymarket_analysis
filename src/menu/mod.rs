//! Menu module
//! 
//! Provides menu data scraping, caching, and API handlers.

pub mod models;
pub mod storage;
pub mod scraper;
pub mod handlers;

pub use models::{MenuData, SportCategory};
pub use scraper::{get_cached_menu, get_menu_or_default, refresh_menu, init_menu_from_storage, init_storage};
pub use storage::Storage;
