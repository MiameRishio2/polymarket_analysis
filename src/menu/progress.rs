//! Football refresh progress tracking
//!
//! Tracks the progress of football data refresh operation.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;

/// Progress state for football refresh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshProgress {
    /// Whether a refresh is currently in progress
    pub in_progress: bool,
    /// Current stage: "idle", "fetching", "parsing", "complete", "error"
    pub stage: String,
    /// Current operation description
    pub current_operation: String,
    /// Progress percentage (0-100)
    pub percent: u32,
    /// Total items to process
    pub total_items: u32,
    /// Items processed so far
    pub processed_items: u32,
    /// Error message if failed
    pub error: Option<String>,
}

/// Internal progress state
pub struct ProgressStateInner {
    in_progress: AtomicBool,
    stage: RwLock<String>,
    current_operation: RwLock<String>,
    percent: AtomicU32,
    total_items: AtomicU32,
    processed_items: AtomicU32,
    error: RwLock<Option<String>>,
}

impl ProgressStateInner {
    fn new() -> Self {
        Self {
            in_progress: AtomicBool::new(false),
            stage: RwLock::new("idle".to_string()),
            current_operation: RwLock::new(String::new()),
            percent: AtomicU32::new(0),
            total_items: AtomicU32::new(0),
            processed_items: AtomicU32::new(0),
            error: RwLock::new(None),
        }
    }

    async fn start(&self, operation: &str) {
        self.in_progress.store(true, Ordering::SeqCst);
        *self.stage.write().await = "fetching".to_string();
        *self.current_operation.write().await = operation.to_string();
        self.percent.store(0, Ordering::SeqCst);
        self.total_items.store(0, Ordering::SeqCst);
        self.processed_items.store(0, Ordering::SeqCst);
        *self.error.write().await = None;
    }

    async fn update(&self, stage: &str, operation: &str, percent: u32) {
        *self.stage.write().await = stage.to_string();
        *self.current_operation.write().await = operation.to_string();
        self.percent.store(percent, Ordering::SeqCst);
    }

    async fn set_total(&self, total: u32) {
        self.total_items.store(total, Ordering::SeqCst);
        self.processed_items.store(0, Ordering::SeqCst);
    }

    async fn increment_processed(&self) -> u32 {
        let current = self.processed_items.fetch_add(1, Ordering::SeqCst) + 1;
        let total = self.total_items.load(Ordering::SeqCst);
        if total > 0 {
            self.percent.store((current * 100) / total, Ordering::SeqCst);
        }
        current
    }

    async fn complete(&self) {
        *self.stage.write().await = "complete".to_string();
        *self.current_operation.write().await = "刷新完成".to_string();
        self.percent.store(100, Ordering::SeqCst);
        self.in_progress.store(false, Ordering::SeqCst);
    }

    async fn fail(&self, error: &str) {
        *self.stage.write().await = "error".to_string();
        *self.current_operation.write().await = "刷新失败".to_string();
        *self.error.write().await = Some(error.to_string());
        self.in_progress.store(false, Ordering::SeqCst);
    }

    async fn reset(&self) {
        *self.stage.write().await = "idle".to_string();
        *self.current_operation.write().await = String::new();
        self.percent.store(0, Ordering::SeqCst);
        self.total_items.store(0, Ordering::SeqCst);
        self.processed_items.store(0, Ordering::SeqCst);
        *self.error.write().await = None;
        self.in_progress.store(false, Ordering::SeqCst);
    }

    async fn get_progress(&self) -> RefreshProgress {
        RefreshProgress {
            in_progress: self.in_progress.load(Ordering::SeqCst),
            stage: self.stage.read().await.clone(),
            current_operation: self.current_operation.read().await.clone(),
            percent: self.percent.load(Ordering::SeqCst),
            total_items: self.total_items.load(Ordering::SeqCst),
            processed_items: self.processed_items.load(Ordering::SeqCst),
            error: self.error.read().await.clone(),
        }
    }

    fn is_in_progress(&self) -> bool {
        self.in_progress.load(Ordering::SeqCst)
    }
}

/// Global progress state instance
pub static FOOTBALL_PROGRESS: Lazy<Arc<ProgressStateInner>> = 
    Lazy::new(|| Arc::new(ProgressStateInner::new()));

/// Start a new refresh operation
pub async fn start_refresh(operation: &str) {
    FOOTBALL_PROGRESS.start(operation).await;
}

/// Update progress
pub async fn update_progress(stage: &str, operation: &str, percent: u32) {
    FOOTBALL_PROGRESS.update(stage, operation, percent).await;
}

/// Set total items to process
pub async fn set_total_items(total: u32) {
    FOOTBALL_PROGRESS.set_total(total).await;
}

/// Mark one item as processed
pub async fn item_processed() {
    FOOTBALL_PROGRESS.increment_processed().await;
}

/// Complete the refresh
pub async fn complete_refresh() {
    FOOTBALL_PROGRESS.complete().await;
}

/// Mark refresh as failed
pub async fn fail_refresh(error: &str) {
    FOOTBALL_PROGRESS.fail(error).await;
}

/// Reset progress state
pub async fn reset_progress() {
    FOOTBALL_PROGRESS.reset().await;
}

/// Get current progress
pub async fn get_progress() -> RefreshProgress {
    FOOTBALL_PROGRESS.get_progress().await
}

/// Check if refresh is in progress
pub fn is_refresh_in_progress() -> bool {
    FOOTBALL_PROGRESS.is_in_progress()
}
