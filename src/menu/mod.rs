//! Menu module - unified data handling for sports and categories

pub mod events;
pub mod handlers;
pub mod models;
pub mod scheduler;
pub mod scraper;
pub mod storage;

// Re-exports for convenience
pub use models::{Category, CategoryData};
pub use scheduler::{NewScheduledMatch, ScheduledMatch, UpdateMonitoringRequest};
pub use scraper::{fetch_sports, fetch_url, get_category_data, get_category_or_default};
pub use storage::Storage;
