//! Scheduler storage and API handlers for monitored match candidates.

use axum::{
    extract::{Path, State},
    Json,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::handlers::{ApiResponse, AppState};
use super::storage::{Storage, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledMatch {
    pub id: String,
    pub matchup: String,
    pub home_team: Option<String>,
    pub away_team: Option<String>,
    pub start_time: Option<String>,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub source_page: Option<String>,
    pub monitoring_started: bool,
    pub added_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewScheduledMatch {
    pub matchup: String,
    pub home_team: Option<String>,
    pub away_team: Option<String>,
    pub start_time: Option<String>,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub source_page: Option<String>,
    pub monitoring_started: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMonitoringRequest {
    pub monitoring_started: bool,
}

impl Storage {
    pub(super) fn init_scheduler_schema(&self, conn: &Connection) -> Result<(), StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS scheduled_matches (
                id TEXT PRIMARY KEY,
                matchup TEXT NOT NULL,
                home_team TEXT,
                away_team TEXT,
                start_time TEXT,
                oddsportal_url TEXT,
                polymarket_url TEXT,
                source_page TEXT,
                monitoring_started INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_scheduled_matches_order
             ON scheduled_matches(monitoring_started DESC, start_time ASC, matchup ASC)",
            [],
        )?;
        Ok(())
    }

    pub fn upsert_scheduled_match(
        &self,
        input: &NewScheduledMatch,
    ) -> Result<ScheduledMatch, StorageError> {
        let id = scheduled_match_id(input);
        let now = chrono::Utc::now().to_rfc3339();
        let monitoring = input.monitoring_started.map(bool_to_i64);
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO scheduled_matches
             (id, matchup, home_team, away_team, start_time, oddsportal_url, polymarket_url,
              source_page, monitoring_started, added_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, COALESCE(?9, 0), ?10, ?10)
             ON CONFLICT(id) DO UPDATE SET
                matchup = excluded.matchup,
                home_team = excluded.home_team,
                away_team = excluded.away_team,
                start_time = excluded.start_time,
                oddsportal_url = excluded.oddsportal_url,
                polymarket_url = excluded.polymarket_url,
                source_page = excluded.source_page,
                monitoring_started = COALESCE(?9, scheduled_matches.monitoring_started),
                updated_at = excluded.updated_at",
            params![
                id,
                input.matchup,
                input.home_team,
                input.away_team,
                input.start_time,
                input.oddsportal_url,
                input.polymarket_url,
                input.source_page,
                monitoring,
                now
            ],
        )?;

        query_scheduled_match(&conn, &id).map(|item| {
            item.expect("scheduled match must exist immediately after successful upsert")
        })
    }

    pub fn list_scheduled_matches(&self) -> Result<Vec<ScheduledMatch>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, matchup, home_team, away_team, start_time, oddsportal_url, polymarket_url,
                    source_page, monitoring_started, added_at, updated_at
             FROM scheduled_matches
             ORDER BY monitoring_started DESC, COALESCE(start_time, '') ASC, matchup ASC",
        )?;
        let rows = stmt.query_map([], scheduled_match_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn set_scheduled_match_monitoring(
        &self,
        id: &str,
        monitoring_started: bool,
    ) -> Result<Option<ScheduledMatch>, StorageError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE scheduled_matches
             SET monitoring_started = ?1, updated_at = ?2
             WHERE id = ?3",
            params![bool_to_i64(monitoring_started), now, id],
        )?;
        query_scheduled_match(&conn, id)
    }

    pub fn delete_scheduled_match(&self, id: &str) -> Result<bool, StorageError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM scheduled_matches WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}

pub(crate) async fn scheduler_list_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ScheduledMatch>>> {
    match state.storage.list_scheduled_matches() {
        Ok(matches) => ok_response(matches),
        Err(error) => error_response(error.to_string()),
    }
}

pub(crate) async fn scheduler_upsert_handler(
    State(state): State<AppState>,
    Json(payload): Json<NewScheduledMatch>,
) -> Json<ApiResponse<ScheduledMatch>> {
    match state.storage.upsert_scheduled_match(&payload) {
        Ok(item) => ok_response(item),
        Err(error) => error_response(error.to_string()),
    }
}

pub(crate) async fn scheduler_monitoring_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateMonitoringRequest>,
) -> Json<ApiResponse<ScheduledMatch>> {
    match state
        .storage
        .set_scheduled_match_monitoring(&id, payload.monitoring_started)
    {
        Ok(Some(item)) => ok_response(item),
        Ok(None) => error_response(format!("Scheduled match '{}' not found", id)),
        Err(error) => error_response(error.to_string()),
    }
}

pub(crate) async fn scheduler_delete_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<bool>> {
    match state.storage.delete_scheduled_match(&id) {
        Ok(deleted) => ok_response(deleted),
        Err(error) => error_response(error.to_string()),
    }
}

fn ok_response<T>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

fn error_response<T>(message: String) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        ok: false,
        data: None,
        error: Some(message),
    })
}

fn query_scheduled_match(
    conn: &Connection,
    id: &str,
) -> Result<Option<ScheduledMatch>, StorageError> {
    let mut stmt = conn.prepare(
        "SELECT id, matchup, home_team, away_team, start_time, oddsportal_url, polymarket_url,
                source_page, monitoring_started, added_at, updated_at
         FROM scheduled_matches
         WHERE id = ?1",
    )?;
    Ok(stmt
        .query_row(params![id], scheduled_match_from_row)
        .optional()?)
}

fn scheduled_match_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledMatch> {
    Ok(ScheduledMatch {
        id: row.get(0)?,
        matchup: row.get(1)?,
        home_team: row.get(2)?,
        away_team: row.get(3)?,
        start_time: row.get(4)?,
        oddsportal_url: row.get(5)?,
        polymarket_url: row.get(6)?,
        source_page: row.get(7)?,
        monitoring_started: row.get::<_, i64>(8)? != 0,
        added_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn scheduled_match_id(input: &NewScheduledMatch) -> String {
    let source = input
        .oddsportal_url
        .as_deref()
        .or(input.polymarket_url.as_deref())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            format!(
                "{}|{}|{}",
                input.matchup,
                input.start_time.as_deref().unwrap_or_default(),
                input.source_page.as_deref().unwrap_or_default()
            )
        });
    format!("s{:016x}", stable_hash(&source))
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}
