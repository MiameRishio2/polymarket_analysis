//! Menu module - unified data handling for sports and categories

pub mod models;
pub mod storage;
pub mod scraper;
pub mod handlers;
pub mod progress;

// Re-exports for convenience
pub use models::{Category, CategoryData, MenuData, SportCategory};
pub use storage::{Storage, StorageError};
pub use scraper::{refresh_category, get_category_or_default, get_cached_category, init_storage, ScraperError};
pub use handlers::AppState;
