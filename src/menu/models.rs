//! Unified models for menu and category data

use serde::{Deserialize, Serialize};

/// Single category item (used for both sports menu and sport-specific categories)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub category_type: Option<String>, // "league", "country", "tournament", "other"
}

impl Category {
    pub fn new(slug: impl Into<String>, name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            slug: slug.into(),
            name: name.into(),
            url: url.into(),
            category_type: None,
        }
    }

    pub fn with_type(mut self, category_type: impl Into<String>) -> Self {
        self.category_type = Some(category_type.into());
        self
    }
}

/// Category data response (unified for all category types)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryData {
    pub sport: String,
    pub categories: Vec<Category>,
    pub last_updated: String,
    pub source: String,
}

impl CategoryData {
    pub fn new(sport: impl Into<String>) -> Self {
        Self {
            sport: sport.into(),
            categories: Vec::new(),
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: String::new(),
        }
    }

    pub fn with_categories(mut self, categories: Vec<Category>) -> Self {
        self.categories = categories;
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

// Backward compatibility aliases
pub type SportCategory = Category;
pub type MenuData = CategoryData;
