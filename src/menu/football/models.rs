//! Football data models
//!
//! Data structures for football category sub-items (leagues, tournaments, etc.)

use serde::{Deserialize, Serialize};

/// Single football sub-category (league, tournament, country, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootballSubCategory {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub category_type: String, // "league", "country", "tournament", "other"
}

/// Football data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootballData {
    pub categories: Vec<FootballSubCategory>,
    pub last_updated: String,
    pub source: String,
}
