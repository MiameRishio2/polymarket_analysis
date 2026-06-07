//! Menu module - unified data handling for sports and categories

pub mod models;
pub mod storage;
pub mod scraper;
pub mod handlers;

// Re-exports for convenience
pub use models::{Category, CategoryData};
pub use storage::Storage;
pub use scraper::{get_category_or_default, get_cached_category};
