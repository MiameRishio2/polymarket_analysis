//! Football sub-module
//!
//! Provides football category sub-item scraping, caching, and API handlers.
//! Similar structure to the menu module but focused on football-specific data.

pub mod models;
pub mod scraper;
pub mod handlers;
pub mod progress;

pub use models::{FootballData, FootballSubCategory};
pub use scraper::scrape_football;
