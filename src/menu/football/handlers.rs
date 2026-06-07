//! Football API handlers
//!
//! Defines HTTP API endpoints for football category operations.

use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::http::HttpClient;
use super::models::FootballData;
use super::scraper::scrape_football;
use super::progress::{self, RefreshProgress};

/// Shared application state (reused from menu handlers)
#[derive(Clone)]
pub struct FootballAppState {
    pub http_client: HttpClient,
}

impl FootballAppState {
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
}

/// GET /api/football response structure
#[derive(Serialize)]
pub struct FootballResponse {
    pub ok: bool,
    pub data: Option<FootballData>,
    pub error: Option<String>,
}

/// POST /api/football/refresh response structure
#[derive(Serialize)]
pub struct FootballRefreshResponse {
    pub ok: bool,
    pub data: Option<FootballData>,
    pub error: Option<String>,
    pub message: String,
}

/// GET /api/football/progress response structure
#[derive(Serialize)]
pub struct ProgressResponse {
    pub in_progress: bool,
    pub stage: String,
    pub current_operation: String,
    pub percent: u32,
    pub total_items: u32,
    pub processed_items: u32,
    pub error: Option<String>,
}

impl From<RefreshProgress> for ProgressResponse {
    fn from(p: RefreshProgress) -> Self {
        ProgressResponse {
            in_progress: p.in_progress,
            stage: p.stage,
            current_operation: p.current_operation,
            percent: p.percent,
            total_items: p.total_items,
            processed_items: p.processed_items,
            error: p.error,
        }
    }
}

/// GET /api/football Handler
///
/// Returns football sub-categories (leagues, tournaments, etc.)
pub async fn football_api_handler(State(state): State<Arc<FootballAppState>>) -> Json<FootballResponse> {
    match scrape_football(&state.http_client).await {
        Ok(data) => Json(FootballResponse {
            ok: true,
            data: Some(data),
            error: None,
        }),
        Err(e) => {
            tracing::warn!("Failed to fetch football data: {}", e);
            Json(FootballResponse {
                ok: false,
                data: None,
                error: Some(format!("获取足球数据失败: {}", e)),
            })
        }
    }
}

/// POST /api/football/refresh Handler
/// 
/// Triggers a fresh scrape of football categories with detailed progress reporting.
/// The progress can be queried via GET /api/football/progress
pub async fn football_refresh_handler(State(state): State<Arc<FootballAppState>>) -> Json<FootballRefreshResponse> {
    tracing::info!("Football data refresh requested");
    
    // Note: Progress tracking is now handled inside scrape_football()
    // The scraper reports progress at each stage:
    // 1. start_refresh() - when beginning
    // 2. update_progress() - during HTTP fetch and parsing
    // 3. set_total_items() - after discovering categories
    // 4. complete_refresh() or fail_refresh() - when done
    
    let result = scrape_football(&state.http_client).await;
    
    match result {
        Ok(data) => {
            let count = data.categories.len();
            
            Json(FootballRefreshResponse {
                ok: true,
                data: Some(data),
                error: None,
                message: format!("足球分类已刷新，获取到 {} 个子分类", count),
            })
        },
        Err(e) => {
            let err_msg = format!("获取足球数据失败: {}", e);
            tracing::error!("Football data refresh failed: {}", e);
            // Note: fail_refresh() is called inside scrape_football via ? operator
            // through the error handling in the scraper
            
            Json(FootballRefreshResponse {
                ok: false,
                data: None,
                error: Some(err_msg.clone()),
                message: "刷新失败".to_string(),
            })
        }
    }
}

/// GET /api/football/progress Handler
///
/// Returns the current progress of the football data refresh operation.
/// 
/// Response includes:
/// - `in_progress`: whether refresh is in progress
/// - `stage`: current stage (fetching, parsing, complete, error)
/// - `current_operation`: detailed description of current operation
/// - `percent`: overall progress percentage (0-100)
/// - `total_items`: total number of categories to process
/// - `processed_items`: number of categories already processed
/// - `error`: error message if failed
pub async fn football_progress_handler() -> Json<ProgressResponse> {
    let p = progress::get_progress().await;
    Json(ProgressResponse::from(p))
}
