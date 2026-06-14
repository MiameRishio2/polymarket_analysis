use axum::{
    extract::{Path, State},
    Json,
};
use base64::Engine;
use flate2::read::GzDecoder;
use openssl::{
    hash::MessageDigest,
    pkcs5,
    symm::{decrypt, Cipher},
};
use regex::Regex;
use reqwest::{Client, Proxy};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    events::event_has_ended_at,
    handlers::{ApiResponse, AppState},
    scheduler::ScheduledMatch,
    scraper::fetch_url,
    storage::{Storage, StorageError},
};

const ODDSPORTAL_KEYS: [(&str, &str); 2] = [
    (
        "J*8sQ!p$7aD_fR2yW@gHn*3bVp#sAdLd_k",
        "5b9a8f2c3e6d1a4b7c8e9d0f1a2b3c4d",
    ),
    (
        "%RtR8AB&nWsh=AQC+v!=pgAe@dSQG3kQ",
        "orieC_jQQWRmhkPvR6u2kzXeTube6aYupiOddsPortal",
    ),
];

static ODDS_ANALYSIS_COLLECTION_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOdds {
    pub status: String,
    pub home_probability: f64,
    pub away_probability: f64,
    pub sample_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OddsPortalOutcomeOdds {
    pub index: usize,
    pub label: String,
    pub odds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OddsPortalMarketRow {
    pub market_key: String,
    pub market_label: String,
    pub bookmaker_id: String,
    #[serde(default)]
    pub bookmaker_name: Option<String>,
    pub outcomes: Vec<OddsPortalOutcomeOdds>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OddsPortalMarketSnapshot {
    pub match_id: String,
    pub matchup: String,
    pub captured_at: String,
    pub rows: Vec<OddsPortalMarketRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestOddsSnapshot {
    pub match_id: String,
    pub matchup: String,
    pub home_team: Option<String>,
    pub away_team: Option<String>,
    pub start_time: Option<String>,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub oddsportal: Option<SourceOdds>,
    pub polymarket: Option<SourceOdds>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwoWayProbability {
    pub home_probability: f64,
    pub away_probability: f64,
}

impl Storage {
    pub(super) fn init_odds_analysis_schema(&self, conn: &Connection) -> Result<(), StorageError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS latest_odds_snapshots (
                match_id TEXT PRIMARY KEY,
                matchup TEXT NOT NULL,
                home_team TEXT,
                away_team TEXT,
                start_time TEXT,
                oddsportal_url TEXT,
                polymarket_url TEXT,
                oddsportal_json TEXT,
                polymarket_json TEXT,
                captured_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_latest_odds_captured
             ON latest_odds_snapshots(captured_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS oddsportal_market_snapshots (
                match_id TEXT NOT NULL,
                captured_at TEXT NOT NULL,
                matchup TEXT NOT NULL,
                rows_json TEXT NOT NULL,
                PRIMARY KEY (match_id, captured_at)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_oddsportal_market_history
             ON oddsportal_market_snapshots(match_id, captured_at DESC)",
            [],
        )?;
        Ok(())
    }

    pub fn save_latest_odds_snapshot(
        &self,
        snapshot: &LatestOddsSnapshot,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let oddsportal_json = serialize_source(&snapshot.oddsportal)?;
        let polymarket_json = serialize_source(&snapshot.polymarket)?;
        conn.execute(
            "INSERT OR REPLACE INTO latest_odds_snapshots
             (match_id, matchup, home_team, away_team, start_time, oddsportal_url, polymarket_url,
              oddsportal_json, polymarket_json, captured_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                snapshot.match_id,
                snapshot.matchup,
                snapshot.home_team,
                snapshot.away_team,
                snapshot.start_time,
                snapshot.oddsportal_url,
                snapshot.polymarket_url,
                oddsportal_json,
                polymarket_json,
                snapshot.captured_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_latest_odds_snapshots(&self) -> Result<Vec<LatestOddsSnapshot>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT match_id, matchup, home_team, away_team, start_time, oddsportal_url,
                    polymarket_url, oddsportal_json, polymarket_json, captured_at
             FROM latest_odds_snapshots
             ORDER BY captured_at DESC, matchup ASC",
        )?;
        let rows = stmt.query_map([], latest_snapshot_from_row)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn latest_odds_snapshot(
        &self,
        match_id: &str,
    ) -> Result<Option<LatestOddsSnapshot>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT match_id, matchup, home_team, away_team, start_time, oddsportal_url,
                    polymarket_url, oddsportal_json, polymarket_json, captured_at
             FROM latest_odds_snapshots
             WHERE match_id = ?1",
        )?;
        Ok(stmt
            .query_row(params![match_id], latest_snapshot_from_row)
            .optional()?)
    }

    pub fn save_oddsportal_market_snapshot(
        &self,
        snapshot: &OddsPortalMarketSnapshot,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();
        let rows_json = serde_json::to_string(&snapshot.rows)?;
        conn.execute(
            "INSERT OR REPLACE INTO oddsportal_market_snapshots
             (match_id, captured_at, matchup, rows_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snapshot.match_id,
                snapshot.captured_at,
                snapshot.matchup,
                rows_json
            ],
        )?;
        Ok(())
    }

    pub fn list_oddsportal_market_history(
        &self,
        match_id: &str,
        limit: usize,
    ) -> Result<Vec<OddsPortalMarketSnapshot>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT match_id, captured_at, matchup, rows_json
             FROM oddsportal_market_snapshots
             WHERE match_id = ?1
             ORDER BY captured_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_id, limit as i64], |row| {
            let rows_json: String = row.get(3)?;
            let market_rows = serde_json::from_str::<Vec<OddsPortalMarketRow>>(&rows_json)
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            Ok(OddsPortalMarketSnapshot {
                match_id: row.get(0)?,
                captured_at: row.get(1)?,
                matchup: row.get(2)?,
                rows: market_rows,
            })
        })?;
        let mut snapshots = rows.filter_map(Result::ok).collect::<Vec<_>>();
        snapshots.reverse();
        Ok(snapshots)
    }
}

pub(crate) async fn analysis_odds_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<LatestOddsSnapshot>>> {
    match state.storage.list_latest_odds_snapshots() {
        Ok(snapshots) => Json(ApiResponse {
            ok: true,
            data: Some(snapshots),
            error: None,
        }),
        Err(error) => Json(ApiResponse {
            ok: false,
            data: None,
            error: Some(error.to_string()),
        }),
    }
}

pub(crate) async fn analysis_odds_collect_handler(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<LatestOddsSnapshot>>> {
    if !ODDS_ANALYSIS_COLLECTION_RUNNING.swap(true, Ordering::AcqRel) {
        match state.storage.list_scheduled_matches() {
            Ok(matches) => {
                let storage = state.storage.clone();
                tokio::spawn(async move {
                    let mut tasks = tokio::task::JoinSet::new();
                    for item in matches.into_iter().filter(|item| item.monitoring_started) {
                        let storage = storage.clone();
                        tasks.spawn(async move {
                            collect_and_store_latest_odds(storage.as_ref(), &item).await;
                        });
                    }
                    while let Some(result) = tasks.join_next().await {
                        if let Err(error) = result {
                            tracing::warn!(error = %error, "odds analysis collection task failed");
                        }
                    }
                    ODDS_ANALYSIS_COLLECTION_RUNNING.store(false, Ordering::Release);
                });
            }
            Err(error) => {
                ODDS_ANALYSIS_COLLECTION_RUNNING.store(false, Ordering::Release);
                return Json(ApiResponse {
                    ok: false,
                    data: None,
                    error: Some(error.to_string()),
                });
            }
        }
    }
    analysis_odds_handler(State(state)).await
}

pub(crate) async fn analysis_oddsportal_history_handler(
    Path(match_id): Path<String>,
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<OddsPortalMarketSnapshot>>> {
    match state.storage.list_oddsportal_market_history(&match_id, 120) {
        Ok(history) => Json(ApiResponse {
            ok: true,
            data: Some(history),
            error: None,
        }),
        Err(error) => Json(ApiResponse {
            ok: false,
            data: None,
            error: Some(error.to_string()),
        }),
    }
}

pub async fn collect_and_store_latest_odds(storage: &Storage, scheduled: &ScheduledMatch) {
    if scheduled_match_has_started(scheduled)
        && matches!(storage.latest_odds_snapshot(&scheduled.id), Ok(Some(_)))
    {
        return;
    }

    let (oddsportal, rows) =
        collect_oddsportal_source_and_market_rows(scheduled.oddsportal_url.as_deref()).await;
    let polymarket =
        collect_polymarket_source(scheduled.polymarket_url.as_deref(), scheduled).await;
    let captured_at = chrono::Utc::now().to_rfc3339();
    let snapshot = LatestOddsSnapshot {
        match_id: scheduled.id.clone(),
        matchup: scheduled.matchup.clone(),
        home_team: scheduled.home_team.clone(),
        away_team: scheduled.away_team.clone(),
        start_time: scheduled.start_time.clone(),
        oddsportal_url: scheduled.oddsportal_url.clone(),
        polymarket_url: scheduled.polymarket_url.clone(),
        oddsportal: Some(oddsportal),
        polymarket: Some(polymarket),
        captured_at: captured_at.clone(),
    };

    if !rows.is_empty() {
        let market_snapshot = OddsPortalMarketSnapshot {
            match_id: scheduled.id.clone(),
            matchup: scheduled.matchup.clone(),
            captured_at,
            rows,
        };
        if let Err(error) = storage.save_oddsportal_market_snapshot(&market_snapshot) {
            tracing::warn!(
                match_id = %scheduled.id,
                error = %error,
                "failed to save oddsportal market snapshot"
            );
        }
    }
    if let Err(error) = storage.save_latest_odds_snapshot(&snapshot) {
        tracing::warn!(
            match_id = %scheduled.id,
            error = %error,
            "failed to save latest odds snapshot"
        );
    }
}

fn scheduled_match_has_started(scheduled: &ScheduledMatch) -> bool {
    scheduled
        .start_time
        .as_deref()
        .map(|start_time| event_has_ended_at(start_time, chrono::Utc::now()))
        .unwrap_or(false)
}

async fn collect_oddsportal_source_and_market_rows(
    url: Option<&str>,
) -> (SourceOdds, Vec<OddsPortalMarketRow>) {
    let Some(url) = url.filter(|value| !value.trim().is_empty()) else {
        return (source_error("missing_url"), Vec::new());
    };
    match fetch_oddsportal_feed_value(url).await {
        Ok(feed) => {
            let source = parse_oddsportal_oddsdata_snapshot(&feed)
                .unwrap_or_else(|| source_error("oddsportal_feed_parse_failed"));
            let rows = parse_oddsportal_market_odds(&feed);
            (source, rows)
        }
        Err(error) => (source_error(error), Vec::new()),
    }
}

pub async fn collect_latest_odds_snapshot(scheduled: &ScheduledMatch) -> LatestOddsSnapshot {
    let (oddsportal, _) =
        collect_oddsportal_source_and_market_rows(scheduled.oddsportal_url.as_deref()).await;
    let polymarket =
        collect_polymarket_source(scheduled.polymarket_url.as_deref(), scheduled).await;
    LatestOddsSnapshot {
        match_id: scheduled.id.clone(),
        matchup: scheduled.matchup.clone(),
        home_team: scheduled.home_team.clone(),
        away_team: scheduled.away_team.clone(),
        start_time: scheduled.start_time.clone(),
        oddsportal_url: scheduled.oddsportal_url.clone(),
        polymarket_url: scheduled.polymarket_url.clone(),
        oddsportal: Some(oddsportal),
        polymarket: Some(polymarket),
        captured_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn oddsportal_probabilities_from_decimal_odds(odds: &[f64]) -> Option<TwoWayProbability> {
    let home_odds = *odds.first()?;
    let away_odds = if odds.len() >= 3 {
        odds[2]
    } else {
        *odds.get(1)?
    };
    if home_odds <= 1.0 || away_odds <= 1.0 {
        return None;
    }

    let home_implied = 1.0 / home_odds;
    let away_implied = 1.0 / away_odds;
    let total = home_implied + away_implied;
    if total <= 0.0 {
        return None;
    }

    Some(TwoWayProbability {
        home_probability: home_implied / total,
        away_probability: away_implied / total,
    })
}

pub fn parse_oddsportal_oddsdata_snapshot(value: &Value) -> Option<SourceOdds> {
    let back = value
        .pointer("/d/oddsdata/back")
        .or_else(|| value.pointer("/oddsdata/back"))?;
    let mut probabilities = Vec::new();
    collect_oddsportal_probabilities(back, &mut probabilities);
    if probabilities.is_empty() {
        return None;
    }

    let sample_count = probabilities.len();
    let home_probability = probabilities
        .iter()
        .map(|prob| prob.home_probability)
        .sum::<f64>()
        / sample_count as f64;
    let away_probability = probabilities
        .iter()
        .map(|prob| prob.away_probability)
        .sum::<f64>()
        / sample_count as f64;

    Some(SourceOdds {
        status: "ok".to_string(),
        home_probability,
        away_probability,
        sample_count,
        error: None,
    })
}

pub fn parse_oddsportal_market_odds(value: &Value) -> Vec<OddsPortalMarketRow> {
    let mut rows = Vec::new();
    collect_oddsportal_market_rows(value, "root", &mut rows);
    rows.sort_by(|left, right| {
        left.market_label
            .cmp(&right.market_label)
            .then(left.bookmaker_id.cmp(&right.bookmaker_id))
    });
    rows
}

fn collect_oddsportal_market_rows(value: &Value, path: &str, rows: &mut Vec<OddsPortalMarketRow>) {
    match value {
        Value::Object(map) => {
            if let Some(odds_map) = map.get("odds").and_then(Value::as_object) {
                let before = rows.len();
                let market_label = oddsportal_market_label(map, path);
                for (bookmaker_id, odds_value) in odds_map {
                    let outcomes = oddsportal_outcome_odds(odds_value);
                    if outcomes.is_empty() {
                        continue;
                    }
                    rows.push(OddsPortalMarketRow {
                        market_key: path.to_string(),
                        market_label: market_label.clone(),
                        bookmaker_id: bookmaker_id.clone(),
                        bookmaker_name: Some(bookmaker_name_from_market(map, bookmaker_id)),
                        outcomes,
                    });
                }
                if rows.len() > before {
                    return;
                }
            }

            for (key, child) in map {
                let child_path = if path == "root" {
                    key.to_string()
                } else {
                    format!("{path}/{key}")
                };
                collect_oddsportal_market_rows(child, &child_path, rows);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_oddsportal_market_rows(child, &format!("{path}/{index}"), rows);
            }
        }
        _ => {}
    }
}

fn bookmaker_name_from_market(
    market: &serde_json::Map<String, Value>,
    bookmaker_id: &str,
) -> String {
    market
        .get("bs")
        .and_then(|bs| bs.get(bookmaker_id))
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .and_then(bookmaker_name_from_betslip)
        .unwrap_or_else(|| format!("Bookmaker {bookmaker_id}"))
}

fn bookmaker_name_from_betslip(value: &str) -> Option<String> {
    let slug = value
        .split("/bookmakers/")
        .nth(1)?
        .split('/')
        .next()?
        .trim();
    if slug.is_empty() {
        return None;
    }

    Some(
        slug.split('-')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => format!(
                        "{}{}",
                        first.to_uppercase().collect::<String>(),
                        chars.as_str()
                    ),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn oddsportal_outcome_odds(value: &Value) -> Vec<OddsPortalOutcomeOdds> {
    let odds = match value {
        Value::Array(items) => numeric_array(items).unwrap_or_default(),
        Value::Object(map) => numeric_indexed_object(map).unwrap_or_default(),
        _ => Vec::new(),
    };
    let labels = outcome_labels(odds.len());
    odds.into_iter()
        .enumerate()
        .map(|(index, odds)| OddsPortalOutcomeOdds {
            index,
            label: labels
                .get(index)
                .map(|value| (*value).to_string())
                .unwrap_or_else(|| index.to_string()),
            odds,
        })
        .collect()
}

fn outcome_labels(count: usize) -> Vec<&'static str> {
    match count {
        3 => vec!["1", "X", "2"],
        2 => vec!["1", "2"],
        _ => Vec::new(),
    }
}

fn oddsportal_market_label(market: &serde_json::Map<String, Value>, fallback: &str) -> String {
    let betting_type = market
        .get("bettingTypeId")
        .and_then(Value::as_i64)
        .map(betting_type_name);
    let scope = market
        .get("scopeId")
        .and_then(Value::as_i64)
        .map(scope_name);
    let Some(betting_type) = betting_type else {
        return market
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| fallback.to_string());
    };
    let scope = scope.unwrap_or("Scope");
    let handicap = market
        .get("handicapValue")
        .and_then(Value::as_f64)
        .filter(|value| value.abs() > f64::EPSILON)
        .map(|value| format!(" {value:+}"))
        .unwrap_or_default();
    format!("{betting_type} · {scope}{handicap}")
}

fn betting_type_name(value: i64) -> &'static str {
    match value {
        1 => "1X2",
        2 => "Over/Under",
        3 => "Home/Away",
        4 => "Double Chance",
        5 => "Asian Handicap",
        6 => "Draw No Bet",
        8 => "Correct Score",
        9 => "Half Time / Full Time",
        10 => "Odd/Even",
        12 => "European Handicap",
        13 => "Both Teams To Score",
        _ => "Market",
    }
}

fn scope_name(value: i64) -> &'static str {
    match value {
        1 => "FT including OT",
        2 => "Full Time",
        3 => "1st Half",
        4 => "2nd Half",
        _ => "Scope",
    }
}

fn collect_oddsportal_probabilities(value: &Value, output: &mut Vec<TwoWayProbability>) {
    match value {
        Value::Array(items) => {
            if let Some(odds) = numeric_array(items) {
                if let Some(probability) = oddsportal_probabilities_from_decimal_odds(&odds) {
                    output.push(probability);
                }
                return;
            }
            for item in items {
                collect_oddsportal_probabilities(item, output);
            }
        }
        Value::Object(map) => {
            if let Some(odds) = numeric_indexed_object(map) {
                if let Some(probability) = oddsportal_probabilities_from_decimal_odds(&odds) {
                    output.push(probability);
                }
                return;
            }
            if let Some(odds_value) = map.get("odds") {
                collect_oddsportal_probabilities(odds_value, output);
            } else {
                for item in map.values() {
                    collect_oddsportal_probabilities(item, output);
                }
            }
        }
        _ => {}
    }
}

fn numeric_indexed_object(map: &serde_json::Map<String, Value>) -> Option<Vec<f64>> {
    let mut odds = Vec::new();
    for index in 0..3 {
        let Some(value) = map.get(&index.to_string()).and_then(Value::as_f64) else {
            break;
        };
        odds.push(value);
    }
    if odds.len() == 2 || odds.len() == 3 {
        Some(odds)
    } else {
        None
    }
}

fn numeric_array(items: &[Value]) -> Option<Vec<f64>> {
    let odds = items
        .iter()
        .map(Value::as_f64)
        .collect::<Option<Vec<_>>>()?;
    if odds.len() == 2 || odds.len() >= 3 {
        Some(odds)
    } else {
        None
    }
}

async fn fetch_oddsportal_feed_value(url: &str) -> Result<Value, String> {
    let html = fetch_url(url).await.map_err(|error| error.to_string())?;
    let feed_urls = discover_match_event_feed_urls(url, &html);
    if feed_urls.is_empty() {
        return Err("no_match_event_feed_found".to_string());
    }

    for feed_url in feed_urls {
        let Ok(encoded) = fetch_url(&feed_url).await else {
            continue;
        };
        let Ok(decoded) = decode_oddsportal_feed(&encoded) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&decoded) else {
            continue;
        };
        return Ok(value);
    }

    Err("oddsportal_feed_parse_failed".to_string())
}

fn discover_match_event_feed_urls(match_url: &str, html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let normalized_html = html.replace("\\/", "/").replace("&quot;", "\"");
    if let Ok(regex) = Regex::new(r#""?(?P<url>/match-event/[^"'\s<>]+?\.dat(?:\?[^"'\s<>]*)?)"#) {
        for captures in regex.captures_iter(&normalized_html) {
            if let Some(found) = captures.name("url") {
                push_unique_url(
                    &mut urls,
                    normalize_match_event_feed_url(&absolutize_oddsportal_url(found.as_str())),
                );
            }
        }
    }

    if let Some(derived) = derive_match_event_feed_url(match_url) {
        push_unique_url(&mut urls, derived);
    }
    urls
}

fn derive_match_event_feed_url(match_url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(match_url).ok()?;
    let fragment = parsed.fragment()?;
    let (event_id, scope) = fragment.split_once(':')?;
    let scope_id = scope
        .split(';')
        .nth(1)
        .filter(|value| !value.is_empty())
        .unwrap_or("2");
    let sport_id = if parsed.path().starts_with("/esports/") {
        "36"
    } else {
        "1"
    };
    Some(format!(
        "https://www.oddsportal.com/match-event/1-{sport_id}-{event_id}-1-{scope_id}-yj1dd.dat?_={}",
        chrono::Utc::now().timestamp_millis()
    ))
}

fn absolutize_oddsportal_url(value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        format!("https://www.oddsportal.com{value}")
    }
}

fn normalize_match_event_feed_url(url: &str) -> String {
    if url.ends_with("_=") {
        format!("{url}{}", chrono::Utc::now().timestamp_millis())
    } else {
        url.to_string()
    }
}

fn push_unique_url(urls: &mut Vec<String>, url: String) {
    if !urls.iter().any(|existing| existing == &url) {
        urls.push(url);
    }
}

fn decode_oddsportal_feed(encoded: &str) -> Result<String, String> {
    let envelope = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|error| error.to_string())?;
    let envelope = String::from_utf8(envelope).map_err(|error| error.to_string())?;
    let (ciphertext_b64, iv_b64) = envelope
        .split_once(':')
        .ok_or_else(|| "invalid_envelope".to_string())?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|error| error.to_string())?;
    let iv = decode_feed_iv(iv_b64)?;

    for (password, salt) in ODDSPORTAL_KEYS {
        let mut key = [0u8; 32];
        if pkcs5::pbkdf2_hmac(
            password.as_bytes(),
            salt.as_bytes(),
            1000,
            MessageDigest::sha256(),
            &mut key,
        )
        .is_err()
        {
            continue;
        }
        let Ok(decrypted) = decrypt(Cipher::aes_256_cbc(), &key, Some(&iv), &ciphertext) else {
            continue;
        };
        if let Ok(text) = inflate_or_text(&decrypted) {
            return Ok(trim_after_last_json_object(&text));
        }
    }
    Err("decode_failed".to_string())
}

fn inflate_or_text(bytes: &[u8]) -> Result<String, String> {
    let data = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = GzDecoder::new(bytes);
        let mut inflated = Vec::new();
        decoder
            .read_to_end(&mut inflated)
            .map_err(|error| error.to_string())?;
        inflated
    } else {
        bytes.to_vec()
    };
    String::from_utf8(data).map_err(|error| error.to_string())
}

fn trim_after_last_json_object(text: &str) -> String {
    match text.rfind('}') {
        Some(index) => text[..=index].trim().to_string(),
        None => text.trim().to_string(),
    }
}

async fn collect_polymarket_source(url: Option<&str>, scheduled: &ScheduledMatch) -> SourceOdds {
    let Some(url) = url.filter(|value| !value.trim().is_empty() && !value.contains("/search?"))
    else {
        return source_error("missing_or_search_fallback_url");
    };
    match fetch_polymarket_snapshot(url, scheduled).await {
        Ok(source) => source,
        Err(error) => source_error(error),
    }
}

async fn fetch_polymarket_snapshot(
    url: &str,
    scheduled: &ScheduledMatch,
) -> Result<SourceOdds, String> {
    let slug = polymarket_slug_from_url(url).ok_or_else(|| "missing_slug".to_string())?;
    let client = proxied_client().map_err(|error| error.to_string())?;
    for endpoint in ["events", "markets"] {
        let response = client
            .get(format!("https://gamma-api.polymarket.com/{endpoint}"))
            .query(&[("slug", slug.as_str())])
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            continue;
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        if let Some(source) = parse_polymarket_event_snapshot(
            &value,
            scheduled.home_team.as_deref(),
            scheduled.away_team.as_deref(),
        ) {
            return Ok(source);
        }
    }
    Err("polymarket_probability_parse_failed".to_string())
}

fn polymarket_slug_from_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .last()
        .map(ToOwned::to_owned)
}

fn proxied_client() -> Result<Client, reqwest::Error> {
    let mut builder = Client::builder().timeout(std::time::Duration::from_secs(30));
    if let Ok(config) = crate::config::load_config("config.yaml") {
        if let Some(proxy_url) = config.proxy_url() {
            if let Ok(proxy) = Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
    }
    builder.build()
}

fn decode_feed_iv(value: &str) -> Result<Vec<u8>, String> {
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(value) {
        if decoded.len() == 16 {
            return Ok(decoded);
        }
    }
    decode_hex(value)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("invalid_hex_iv".to_string());
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chars = value.as_bytes().chunks_exact(2);
    for pair in &mut chars {
        let text = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
        let byte = u8::from_str_radix(text, 16).map_err(|error| error.to_string())?;
        bytes.push(byte);
    }
    if bytes.len() == 16 {
        Ok(bytes)
    } else {
        Err("invalid_iv_length".to_string())
    }
}

pub fn parse_polymarket_event_snapshot(
    value: &Value,
    home_team: Option<&str>,
    away_team: Option<&str>,
) -> Option<SourceOdds> {
    parse_polymarket_combined_market(value, home_team, away_team)
        .or_else(|| parse_polymarket_team_win_markets(value, home_team, away_team))
}

fn parse_polymarket_combined_market(
    value: &Value,
    home_team: Option<&str>,
    away_team: Option<&str>,
) -> Option<SourceOdds> {
    polymarket_market_candidates(value).find_map(|candidate| {
        let outcomes = parse_json_string_array(candidate.get("outcomes")?)?;
        let prices = parse_json_string_array(candidate.get("outcomePrices")?)
            .or_else(|| parse_json_string_array(candidate.get("prices")?))?;
        if outcomes.len() != prices.len() || outcomes.len() < 2 {
            return None;
        }
        let home_index = outcome_index_for_team(&outcomes, home_team)?;
        let away_index = outcome_index_for_team(&outcomes, away_team)?;
        let home = prices.get(home_index)?.parse::<f64>().ok()?;
        let away = prices.get(away_index)?.parse::<f64>().ok()?;
        normalize_prices(home, away).map(|probability| SourceOdds {
            status: "ok".to_string(),
            home_probability: probability.home_probability,
            away_probability: probability.away_probability,
            sample_count: 1,
            error: None,
        })
    })
}

fn parse_polymarket_team_win_markets(
    value: &Value,
    home_team: Option<&str>,
    away_team: Option<&str>,
) -> Option<SourceOdds> {
    let home_team = home_team?;
    let away_team = away_team?;
    let mut home_price = None;
    let mut away_price = None;

    for candidate in polymarket_market_candidates(value) {
        let text = polymarket_market_text(candidate);
        if is_draw_market(&text) {
            continue;
        }
        let Some(yes_price) = polymarket_yes_price(candidate) else {
            continue;
        };
        if market_text_matches_team(&text, home_team) {
            home_price = Some(yes_price);
        }
        if market_text_matches_team(&text, away_team) {
            away_price = Some(yes_price);
        }
    }

    let probability = normalize_prices(home_price?, away_price?)?;
    Some(SourceOdds {
        status: "ok".to_string(),
        home_probability: probability.home_probability,
        away_probability: probability.away_probability,
        sample_count: 2,
        error: None,
    })
}

fn polymarket_market_text(candidate: &Value) -> String {
    [
        "question",
        "title",
        "slug",
        "groupItemTitle",
        "description",
        "sportsMarketType",
    ]
    .into_iter()
    .filter_map(|field| candidate.get(field).and_then(Value::as_str))
    .map(normalize_term)
    .collect::<Vec<_>>()
    .join(" ")
}

fn is_draw_market(text: &str) -> bool {
    let padded = format!(" {text} ");
    padded.contains(" draw ")
}

fn polymarket_yes_price(candidate: &Value) -> Option<f64> {
    let outcomes = parse_json_string_array(candidate.get("outcomes")?)?;
    let prices = parse_json_string_array(candidate.get("outcomePrices")?)
        .or_else(|| parse_json_string_array(candidate.get("prices")?))?;
    let yes_index = outcomes
        .iter()
        .position(|outcome| normalize_term(outcome) == "yes")?;
    prices.get(yes_index)?.parse::<f64>().ok()
}

fn market_text_matches_team(text: &str, team: &str) -> bool {
    polymarket_team_variants(team)
        .into_iter()
        .any(|variant| text.contains(&variant))
}

fn polymarket_team_variants(team: &str) -> Vec<String> {
    let normalized = normalize_team_alias(team);
    let mut variants = vec![normalized.clone()];
    match normalized.as_str() {
        "bosnia herzegovina" | "bosnia and herzegovina" => {
            variants.push("bosnia".to_string());
            variants.push("bosnia herzegovina".to_string());
            variants.push("bosnia and herzegovina".to_string());
        }
        _ => {}
    }
    variants
}

fn normalize_team_alias(team: &str) -> String {
    normalize_term(team).replace(" and ", " ")
}

fn polymarket_market_candidates(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    if let Some(items) = value.as_array() {
        return Box::new(items.iter().flat_map(polymarket_market_candidate_values));
    }
    Box::new(polymarket_market_candidate_values(value))
}

fn polymarket_market_candidate_values(value: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    if let Some(markets) = value.get("markets").and_then(Value::as_array) {
        return Box::new(markets.iter());
    }
    Box::new(std::iter::once(value))
}

fn parse_json_string_array(value: &Value) -> Option<Vec<String>> {
    if let Some(items) = value.as_array() {
        return Some(
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect(),
        );
    }
    let text = value.as_str()?;
    serde_json::from_str::<Vec<String>>(text).ok()
}

fn outcome_index_for_team(outcomes: &[String], team: Option<&str>) -> Option<usize> {
    let team = normalize_term(team?);
    outcomes.iter().position(|outcome| {
        normalize_term(outcome).contains(&team) || team.contains(&normalize_term(outcome))
    })
}

fn normalize_prices(home: f64, away: f64) -> Option<TwoWayProbability> {
    if home <= 0.0 || away <= 0.0 {
        return None;
    }
    let total = home + away;
    Some(TwoWayProbability {
        home_probability: home / total,
        away_probability: away / total,
    })
}

fn normalize_term(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn source_error(error: impl Into<String>) -> SourceOdds {
    SourceOdds {
        status: "error".to_string(),
        home_probability: 0.0,
        away_probability: 0.0,
        sample_count: 0,
        error: Some(error.into()),
    }
}

fn serialize_source(source: &Option<SourceOdds>) -> Result<Option<String>, StorageError> {
    source
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(StorageError::Json)
}

fn deserialize_source(value: Option<String>) -> rusqlite::Result<Option<SourceOdds>> {
    value
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

fn latest_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LatestOddsSnapshot> {
    Ok(LatestOddsSnapshot {
        match_id: row.get(0)?,
        matchup: row.get(1)?,
        home_team: row.get(2)?,
        away_team: row.get(3)?,
        start_time: row.get(4)?,
        oddsportal_url: row.get(5)?,
        polymarket_url: row.get(6)?,
        oddsportal: deserialize_source(row.get(7)?)?,
        polymarket: deserialize_source(row.get(8)?)?,
        captured_at: row.get(9)?,
    })
}
