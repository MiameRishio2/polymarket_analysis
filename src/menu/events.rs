//! Event list parsing and API handlers for OddsPortal competition pages

use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use chrono::DateTime;
use regex::Regex;
use scraper::{ElementRef, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::convert::Infallible;

use super::handlers::{ApiResponse, AppState};
use super::models::default_refreshed_at;
use super::scraper::{fetch_url, ScraperError};
use super::storage::{Storage, StorageError};
use std::collections::HashMap;

const ODDSPORTAL_BASE_URL: &str = "https://www.oddsportal.com";

/// 用于匹配 World Cup 2026 的 Polymarket tag_ids
const POLYMARKET_WORLD_CUP_TAG_IDS: [&str; 3] = ["519", "102350", "102232"];
const POLYMARKET_GAMMA_KEYSET_URL: &str = "https://gamma-api.polymarket.com/events/keyset";

/// Polymarket 事件的本地索引，用于一次构造后在 O(1) 做本地匹配。
pub struct PolymarketIndex {
    pub by_slug: HashMap<String, Value>,
    pub by_ticker: HashMap<String, Value>,
    pub events: Vec<Value>,
}

impl PolymarketIndex {
    pub fn empty() -> Self {
        Self {
            by_slug: HashMap::new(),
            by_ticker: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// 从一批 `Value`（来自多个 `events/keyset` 响应的数组）构造索引。
    pub fn from_merged_events(all_events: Vec<Value>) -> Self {
        let mut by_slug: HashMap<String, Value> = HashMap::new();
        let mut by_ticker: HashMap<String, Value> = HashMap::new();
        let mut events: Vec<Value> = Vec::with_capacity(all_events.len());

        for candidate in all_events {
            let slug = candidate
                .get("slug")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let ticker = candidate
                .get("ticker")
                .and_then(Value::as_str)
                .map(|s| s.to_string());

            let dedup_key = match (slug.as_ref(), ticker.as_ref()) {
                (Some(s), _) => format!("s:{}", s),
                (_, Some(t)) => format!("t:{}", t),
                _ => continue,
            };

            if by_slug.contains_key(&dedup_key) {
                // 已去重，跳过
                continue;
            }

            if let Some(s) = slug.clone() {
                by_slug.entry(s).or_insert_with(|| candidate.clone());
            }
            if let Some(t) = ticker.clone() {
                by_ticker.entry(t).or_insert_with(|| candidate.clone());
            }
            by_slug.insert(dedup_key, candidate.clone());
            events.push(candidate);
        }

        Self {
            by_slug,
            by_ticker,
            events,
        }
    }
}

/// 对 POLYMARKET_WORLD_CUP_TAG_IDS 并发抓取 `events/keyset`，合并返回事件列表。
/// 只在世界赛白名单时被调用。任何单个请求失败都不会中断整个聚合。
async fn fetch_world_cup_candidate_events() -> Vec<Value> {
    let client = reqwest::Client::builder()
        .user_agent("polymarket-analysis/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok();
    if client.is_none() {
        return Vec::new();
    }
    let client = client.unwrap();

    fn keyset_future(
        client: reqwest::Client,
        tag_id: &'static str,
    ) -> impl std::future::Future<Output = Vec<Value>> {
        async move {
            let url = format!(
                "{}?tag_id={}&closed=false&archived=false&limit=100",
                POLYMARKET_GAMMA_KEYSET_URL, tag_id
            );
            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("polymarket keyset {}: {}", tag_id, e);
                    return Vec::new();
                }
            };
            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("polymarket keyset body {}: {}", tag_id, e);
                    return Vec::new();
                }
            };
            parse_keyset_body(&text)
        }
    }

    let mut tag_ids = POLYMARKET_WORLD_CUP_TAG_IDS.iter();
    let (a, b, c) = (
        keyset_future(client.clone(), tag_ids.next().unwrap_or(&"519")),
        keyset_future(client.clone(), tag_ids.next().unwrap_or(&"102350")),
        keyset_future(client, tag_ids.next().unwrap_or(&"102232")),
    );
    let (ra, rb, rc) = tokio::join!(a, b, c);

    let mut merged: Vec<Value> = Vec::new();
    merged.extend(ra);
    merged.extend(rb);
    merged.extend(rc);
    merged
}

fn parse_keyset_body(text: &str) -> Vec<Value> {
    // 响应常见为 `[event, ...]`，也可能是 `{"events":[...]}`；两种都兼容处理。
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    if let Some(arr) = parsed.as_array() {
        return arr.clone();
    }
    if let Some(inner) = parsed.get("events").and_then(|v| v.as_array()) {
        return inner.clone();
    }
    Vec::new()
}

/// 构造 PolymarketIndex（仅 World Cup 2026 时调用）；非白名单或失败时返回空。
async fn build_world_cup_index(sport: &str, category: &str, league: &str) -> PolymarketIndex {
    if !is_world_cup_competition(sport, category, league) {
        return PolymarketIndex::empty();
    }
    tracing::info!(
        "building PolymarketIndex with {} concurrent keyset requests",
        POLYMARKET_WORLD_CUP_TAG_IDS.len()
    );
    let merged = fetch_world_cup_candidate_events().await;
    let index = PolymarketIndex::from_merged_events(merged);
    tracing::info!(
        "PolymarketIndex done: {} merged events",
        index.events.len()
    );
    index
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    #[serde(default)]
    pub ended: bool,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub sport: String,
    pub events: Vec<EventRow>,
    pub last_updated: String,
    #[serde(default = "default_refreshed_at")]
    pub refreshed_at: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRefreshProgressMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub index: usize,
    pub total: usize,
    pub matchup: String,
    pub start_time: String,
    pub status: String,
    pub polymarket_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventRefreshCompleteMessage {
    #[serde(rename = "type")]
    message_type: String,
    ok: bool,
    event_count: usize,
    refreshed_at: String,
    events: Vec<EventRow>,
    matched_count: usize,
    fallback_search_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventRefreshErrorMessage {
    #[serde(rename = "type")]
    message_type: String,
    ok: bool,
    error: String,
}

pub fn event_cache_key(sport: &str, category: &str, league: &str) -> String {
    format!("events_{}_{}_{}", sport, category, league)
}

fn event_sport_key(sport: &str, category: &str, league: &str) -> String {
    format!("{}/{}/{}", sport, category, league)
}

pub fn save_event_data(storage: &Storage, key: &str, data: &EventData) -> Result<(), StorageError> {
    let events_json = serde_json::to_string(&data.events)?;
    storage.save_raw_json(
        key,
        &events_json,
        &data.last_updated,
        &data.source,
        &data.refreshed_at,
    )
}

pub fn load_event_data(
    storage: &Storage,
    key: &str,
    sport_key: String,
) -> Result<Option<EventData>, StorageError> {
    let Some((events_json, last_updated, source, refreshed_at)) = storage.load_raw_json(key)?
    else {
        return Ok(None);
    };
    let events = serde_json::from_str::<Vec<EventRow>>(&events_json)?;
    Ok(Some(EventData {
        sport: sport_key,
        events,
        last_updated,
        refreshed_at,
        source,
    }))
}

pub fn extract_events_for_competition(
    html: &str,
    sport: &str,
    _category: &str,
    _league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let document = scraper::Html::parse_document(html);
    let link_selector = Selector::parse("a[href]")
        .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
    let mut events = Vec::new();

    extract_json_ld_events(&document, sport, &mut events)?;

    for element in document.select(&link_selector) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let url = normalize_event_url(href);
        if !is_h2h_event_url_for_sport(&url, sport) {
            continue;
        }

        let text_parts = element
            .text()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let text = text_parts.join(" ");
        let (home_team, away_team) = teams_from_link_or_text(&url, &text_parts, &text)?;
        let start_time = nearest_time_text(&element).unwrap_or_default();

        push_event_row(&mut events, home_team, away_team, start_time, url);
    }

    Ok(events)
}

pub async fn fetch_events_for_competition(
    sport: &str,
    category: &str,
    league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let mut events =
        fetch_competition_event_rows_without_polymarket(sport, category, league).await?;
    enrich_events_with_polymarket_urls(&mut events, sport, category, league).await;
    Ok(events)
}

async fn fetch_competition_event_rows_without_polymarket(
    sport: &str,
    category: &str,
    league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let url = format!(
        "https://www.oddsportal.com/{}/{}/{}/",
        sport, category, league
    );
    let html = fetch_url(&url).await?;
    extract_events_for_competition(&html, sport, category, league)
}

pub(crate) async fn event_list_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<EventData>> {
    let cache_key = event_cache_key(&sport, &category, &league);
    let sport_key = event_sport_key(&sport, &category, &league);

    match load_event_data(state.storage.as_ref(), &cache_key, sport_key.clone()) {
        Ok(Some(data)) => return ok_response(data),
        Ok(None) | Err(_) => {}
    }

    let data = fetch_event_data(&sport, &category, &league).await;
    if should_persist_event_data(&data) {
        let _ = save_event_data(state.storage.as_ref(), &cache_key, &data);
    }
    ok_response(data)
}

pub(crate) async fn event_list_refresh_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<EventData>> {
    let cache_key = event_cache_key(&sport, &category, &league);
    let data = fetch_event_data(&sport, &category, &league).await;
    if should_persist_event_data(&data) {
        let _ = save_event_data(state.storage.as_ref(), &cache_key, &data);
    }
    ok_response(data)
}

pub(crate) async fn event_list_refresh_stream_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Response {
    let storage = state.storage.clone();
    let stream = async_stream::stream! {
        let cache_key = event_cache_key(&sport, &category, &league);
        let now = || chrono::Utc::now().to_rfc3339();
        match fetch_competition_event_rows_without_polymarket(&sport, &category, &league).await {
            Ok(mut events) => {
                let total = events.len();
                // World Cup: 并发一次构造 index，再对每个事件本地匹配；
                // 其他赛事: 完全不访问 Polymarket，直接搜索 fallback
                let index = build_world_cup_index(&sport, &category, &league).await;
                let mut matched_count: usize = 0;
                let mut fallback_count: usize = 0;

                for index_ in 0..events.len() {
                    if !is_world_cup_competition(&sport, &category, &league) {
                        if events[index_].polymarket_url.is_none() {
                            events[index_].polymarket_url =
                                polymarket_search_url_for_event(&events[index_]);
                            fallback_count += 1;
                        }
                    } else {
                        enrich_event_with_polymarket_url_from_index(
                            &mut events[index_],
                            &sport,
                            &category,
                            &league,
                            &index,
                        );
                        match events[index_].polymarket_url.as_deref() {
                            Some(u) if u.contains("/search?") => fallback_count += 1,
                            Some(_) => matched_count += 1,
                            None => {}
                        }
                    }
                    yield ndjson_bytes(event_refresh_progress_message(index_ + 1, total, &events[index_]));
                }

                let refreshed_at = now();
                let data = EventData {
                    sport: event_sport_key(&sport, &category, &league),
                    events: events.clone(),
                    last_updated: refreshed_at.clone(),
                    refreshed_at: refreshed_at.clone(),
                    source: "scraped".to_string(),
                };
                if should_persist_event_data(&data) {
                    let _ = save_event_data(storage.as_ref(), &cache_key, &data);
                }
                tracing::info!(
                    "event refresh stream done: total={}, matched={}, fallback_search={}",
                    events.len(),
                    matched_count,
                    fallback_count
                );
                yield ndjson_bytes(EventRefreshCompleteMessage {
                    message_type: "complete".to_string(),
                    ok: true,
                    event_count: events.len(),
                    refreshed_at,
                    events,
                    matched_count,
                    fallback_search_count: fallback_count,
                });
            }
            Err(error) => {
                yield ndjson_bytes(EventRefreshErrorMessage {
                    message_type: "error".to_string(),
                    ok: false,
                    error: error.to_string(),
                });
            }
        }
    };

    (
        [
            (header::CONTENT_TYPE, "application/x-ndjson; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn fetch_event_data(sport: &str, category: &str, league: &str) -> EventData {
    match fetch_events_for_competition(sport, category, league).await {
        Ok(events) => EventData {
            sport: event_sport_key(sport, category, league),
            events,
            last_updated: chrono::Utc::now().to_rfc3339(),
            refreshed_at: chrono::Utc::now().to_rfc3339(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!(
                "Failed to fetch events for {}/{}/{}: {}",
                sport,
                category,
                league,
                e
            );
            EventData {
                sport: event_sport_key(sport, category, league),
                events: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                refreshed_at: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            }
        }
    }
}

fn ndjson_bytes<T: Serialize>(message: T) -> Result<Bytes, Infallible> {
    let line = serde_json::to_string(&message).unwrap_or_else(|error| {
        serde_json::json!({
            "type": "error",
            "ok": false,
            "error": error.to_string()
        })
        .to_string()
    }) + "\n";
    Ok(Bytes::from(line))
}

pub fn event_refresh_progress_message(
    index: usize,
    total: usize,
    event: &EventRow,
) -> EventRefreshProgressMessage {
    EventRefreshProgressMessage {
        message_type: "event".to_string(),
        index,
        total,
        matchup: event.matchup.clone(),
        start_time: event.start_time.clone(),
        status: event_polymarket_status(event).to_string(),
        polymarket_url: event.polymarket_url.clone(),
    }
}

fn event_polymarket_status(event: &EventRow) -> &'static str {
    match event.polymarket_url.as_deref() {
        Some(url) if url.contains("/search?") => "search_fallback",
        Some(_) => "matched",
        None => "missing_polymarket",
    }
}

fn should_persist_event_data(data: &EventData) -> bool {
    data.source != "error" && !data.events.is_empty()
}

fn ok_response<T>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

fn extract_json_ld_events(
    document: &scraper::Html,
    sport: &str,
    events: &mut Vec<EventRow>,
) -> Result<(), ScraperError> {
    let selector = Selector::parse(r#"script[type="application/ld+json"]"#)
        .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;

    for element in document.select(&selector) {
        let json_text = element.text().collect::<Vec<_>>().join("");
        let Ok(value) = serde_json::from_str::<Value>(&json_text) else {
            continue;
        };
        collect_json_ld_event(&value, sport, events)?;
    }

    Ok(())
}

fn collect_json_ld_event(
    value: &Value,
    sport: &str,
    events: &mut Vec<EventRow>,
) -> Result<(), ScraperError> {
    if let Some(items) = value.as_array() {
        for item in items {
            collect_json_ld_event(item, sport, events)?;
        }
        return Ok(());
    }

    if !json_ld_type_matches(value, "SportsEvent") {
        return Ok(());
    }

    let Some(url_raw) = value.get("url").and_then(Value::as_str) else {
        return Ok(());
    };
    let url = normalize_event_url(url_raw);
    if !is_h2h_event_url_for_sport(&url, sport) {
        return Ok(());
    }

    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (home_team, away_team) = teams_from_link_or_text(&url, &[], name)?;
    let start_time = value
        .get("startDate")
        .and_then(Value::as_str)
        .map(format_start_date)
        .unwrap_or_default();

    push_event_row(events, home_team, away_team, start_time, url);
    Ok(())
}

fn json_ld_type_matches(value: &Value, expected: &str) -> bool {
    match value.get("@type") {
        Some(Value::String(kind)) => kind == expected,
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind.as_str() == Some(expected)),
        _ => false,
    }
}

fn push_event_row(
    events: &mut Vec<EventRow>,
    home_team: String,
    away_team: String,
    start_time: String,
    url: String,
) {
    if events.iter().any(|event| event.url == url) {
        return;
    }

    let home_team = decode_html_entities(&home_team);
    let away_team = decode_html_entities(&away_team);
    let slug = format!("{}-vs-{}", slugify(&home_team), slugify(&away_team));
    let matchup = format!("{} VS {}", home_team, away_team);
    let ended = event_has_ended_now(&start_time);
    events.push(EventRow {
        slug,
        home_team,
        away_team,
        matchup,
        start_time,
        ended,
        url,
        polymarket_url: None,
    });
}

async fn enrich_events_with_polymarket_urls(
    events: &mut [EventRow],
    sport: &str,
    category: &str,
    league: &str,
) {
    if !is_world_cup_competition(sport, category, league) {
        // 非 World Cup: 使用搜索 URL fallback，不发起任何 Polymarket API 请求
        for event in events {
            if event.polymarket_url.is_none() {
                event.polymarket_url = polymarket_search_url_for_event(event);
            }
        }
        return;
    }

    // World Cup: 并发一次构造索引，再对每个事件做本地匹配
    let index = build_world_cup_index(sport, category, league).await;
    for event in events {
        enrich_event_with_polymarket_url_from_index(event, sport, category, league, &index);
    }
}

fn enrich_event_with_polymarket_url_from_index(
    event: &mut EventRow,
    sport: &str,
    category: &str,
    league: &str,
    index: &PolymarketIndex,
) {
    if event.polymarket_url.is_some() {
        return;
    }

    // 1) 先在 index.by_slug 精确查找
    for candidate_slug in polymarket_slug_candidates(event, sport, category, league) {
        if let Some(candidate) = index.by_slug.get(&candidate_slug) {
            let actual_slug = candidate
                .get("slug")
                .and_then(Value::as_str)
                .unwrap_or(&candidate_slug);
            if let Some(url) = polymarket_public_url_for_slug(actual_slug, sport, category, league) {
                event.polymarket_url = Some(url);
                return;
            }
        }
    }

    // 2) 退而在 index.events 做本地搜索匹配（原有的文本+日期匹配）
    let dates = polymarket_event_date_candidates(&event.start_time);
    if !dates.is_empty() {
        for candidate in &index.events {
            if polymarket_search_result_matches_event(candidate, event, &dates) {
                if let Some(slug) = candidate.get("slug").and_then(Value::as_str) {
                    if let Some(url) =
                        polymarket_public_url_for_slug(slug, sport, category, league)
                    {
                        event.polymarket_url = Some(url);
                        return;
                    }
                }
            }
        }
    }

    // 3) 最终 fallback 到 Polymarket 搜索 URL
    event.polymarket_url = polymarket_search_url_for_event(event);
}

pub fn polymarket_slug_lookup_result_matches(results: &Value, slug: &str) -> bool {
    polymarket_result_events(results).any(|candidate| {
        candidate.get("slug").and_then(Value::as_str) == Some(slug)
            || candidate.get("ticker").and_then(Value::as_str) == Some(slug)
    })
}

pub fn polymarket_search_url_for_event(event: &EventRow) -> Option<String> {
    let query = format!("{} {}", event.home_team.trim(), event.away_team.trim())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        return None;
    }
    let mut url = reqwest::Url::parse("https://polymarket.com/search").ok()?;
    url.query_pairs_mut().append_pair("query", &query);
    Some(url.to_string())
}

pub fn polymarket_slug_candidates(
    event: &EventRow,
    sport: &str,
    category: &str,
    league: &str,
) -> Vec<String> {
    if !is_world_cup_competition(sport, category, league) {
        return Vec::new();
    }

    let Some(home_code) = world_cup_team_code(&event.home_team) else {
        return Vec::new();
    };
    let Some(away_code) = world_cup_team_code(&event.away_team) else {
        return Vec::new();
    };
    let dates = polymarket_event_date_candidates(&event.start_time);
    if dates.is_empty() {
        return Vec::new();
    };

    let mut slugs = Vec::new();
    for date in dates {
        for home in polymarket_team_slug_terms(&event.home_team, home_code) {
            for away in polymarket_team_slug_terms(&event.away_team, away_code) {
                push_unique(&mut slugs, format!("fifwc-{home}-{away}-{date}"));
            }
        }
    }
    slugs
}

pub fn polymarket_url_from_search_results(
    event: &EventRow,
    results: &Value,
    sport: &str,
    category: &str,
    league: &str,
) -> Option<String> {
    if !is_world_cup_competition(sport, category, league) {
        return None;
    }

    let dates = polymarket_event_date_candidates(&event.start_time);
    if dates.is_empty() {
        return None;
    }
    polymarket_result_events(results).find_map(|candidate| {
        if !polymarket_search_result_matches_event(candidate, event, &dates) {
            return None;
        }
        let slug = candidate.get("slug").and_then(Value::as_str)?;
        polymarket_public_url_for_slug(slug, sport, category, league)
    })
}

fn polymarket_result_events(results: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    if let Some(events) = results.as_array() {
        return Box::new(events.iter());
    }
    if let Some(events) = results.get("events").and_then(Value::as_array) {
        return Box::new(events.iter());
    }
    if results.as_object().is_some() {
        return Box::new(std::iter::once(results));
    }
    Box::new(std::iter::empty())
}

pub fn polymarket_public_url_for_slug(
    slug: &str,
    sport: &str,
    category: &str,
    league: &str,
) -> Option<String> {
    if is_world_cup_competition(sport, category, league) {
        return Some(format!("https://polymarket.com/sports/world-cup/{slug}"));
    }

    None
}

fn is_world_cup_competition(sport: &str, category: &str, league: &str) -> bool {
    sport == "football" && category == "world" && league == "world-championship-2026"
}

fn world_cup_team_code(team: &str) -> Option<&'static str> {
    match normalize_polymarket_team_name(team).as_str() {
        "argentina" => Some("arg"),
        "algeria" => Some("alg"),
        "australia" => Some("aus"),
        "austria" => Some("aut"),
        "belgium" => Some("bel"),
        "bosnia herzegovina" | "bosnia and herzegovina" => Some("bih"),
        "brazil" => Some("bra"),
        "canada" => Some("can"),
        "cape verde" => Some("cpv"),
        "chile" => Some("chi"),
        "china" => Some("chn"),
        "colombia" => Some("col"),
        "curacao" | "cura ao" => Some("cuw"),
        "croatia" => Some("cro"),
        "czech republic" | "czechia" => Some("cze"),
        "denmark" => Some("den"),
        "d r congo" | "dr congo" | "democratic republic congo" => Some("cod"),
        "ecuador" => Some("ecu"),
        "egypt" => Some("egy"),
        "england" => Some("eng"),
        "france" => Some("fra"),
        "germany" => Some("ger"),
        "ghana" => Some("gha"),
        "greece" => Some("gre"),
        "haiti" => Some("hai"),
        "iran" => Some("irn"),
        "iraq" => Some("irq"),
        "italy" => Some("ita"),
        "japan" => Some("jpn"),
        "ivory coast" | "cote d ivoire" => Some("civ"),
        "jordan" => Some("jor"),
        "morocco" => Some("mar"),
        "mexico" => Some("mex"),
        "netherlands" => Some("ned"),
        "new zealand" => Some("nzl"),
        "nigeria" => Some("nga"),
        "norway" => Some("nor"),
        "panama" => Some("pan"),
        "paraguay" => Some("par"),
        "peru" => Some("per"),
        "poland" => Some("pol"),
        "portugal" => Some("por"),
        "qatar" => Some("qat"),
        "saudi arabia" => Some("ksa"),
        "scotland" => Some("sco"),
        "senegal" => Some("sen"),
        "serbia" => Some("srb"),
        "south africa" => Some("rsa"),
        "south korea" | "korea republic" => Some("kor"),
        "spain" => Some("esp"),
        "sweden" => Some("swe"),
        "switzerland" => Some("sui"),
        "tunisia" => Some("tun"),
        "turkey" | "turkiye" => Some("tur"),
        "ukraine" => Some("ukr"),
        "united states" | "usa" | "usmnt" => Some("usa"),
        "uruguay" => Some("uru"),
        "uzbekistan" => Some("uzb"),
        "wales" => Some("wal"),
        _ => None,
    }
}

fn polymarket_team_slug_terms(team: &str, code: &str) -> Vec<String> {
    let mut terms = vec![code.to_string()];
    for alias in polymarket_team_aliases(team, code) {
        push_unique(&mut terms, alias.replace(' ', "-"));
    }
    terms
}

fn polymarket_search_result_matches_event(
    candidate: &Value,
    event: &EventRow,
    dates: &[String],
) -> bool {
    let searchable_text = polymarket_searchable_text(candidate);
    if !contains_polymarket_team(&searchable_text, &event.home_team) {
        return false;
    }
    if !contains_polymarket_team(&searchable_text, &event.away_team) {
        return false;
    }

    polymarket_search_result_dates(candidate)
        .into_iter()
        .any(|candidate_date| dates.iter().any(|date| date == &candidate_date))
}

fn polymarket_searchable_text(candidate: &Value) -> String {
    ["slug", "title", "ticker", "description"]
        .into_iter()
        .filter_map(|field| candidate.get(field).and_then(Value::as_str))
        .map(normalize_polymarket_team_name)
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_polymarket_team(searchable_text: &str, team: &str) -> bool {
    let Some(code) = world_cup_team_code(team) else {
        return false;
    };
    polymarket_team_aliases(team, code)
        .into_iter()
        .any(|alias| contains_normalized_term(searchable_text, &alias))
}

fn contains_normalized_term(searchable_text: &str, term: &str) -> bool {
    let haystack = format!(" {searchable_text} ");
    let needle = format!(" {} ", normalize_polymarket_team_name(term));
    haystack.contains(&needle)
}

fn polymarket_team_aliases(team: &str, code: &str) -> Vec<String> {
    let normalized = normalize_polymarket_team_name(team);
    let mut aliases = Vec::new();
    push_unique(&mut aliases, code.to_string());
    push_unique(&mut aliases, normalized.clone());
    match normalized.as_str() {
        "usa" | "usmnt" | "united states" => {
            push_unique(&mut aliases, "usa".to_string());
            push_unique(&mut aliases, "united states".to_string());
            push_unique(&mut aliases, "usmnt".to_string());
        }
        "bosnia herzegovina" | "bosnia and herzegovina" => {
            push_unique(&mut aliases, "bosnia herzegovina".to_string());
            push_unique(&mut aliases, "bosnia and herzegovina".to_string());
        }
        "curacao" | "cura ao" => {
            push_unique(&mut aliases, "curacao".to_string());
            push_unique(&mut aliases, "cuw".to_string());
            push_unique(&mut aliases, "cur".to_string());
        }
        "d r congo" | "dr congo" | "democratic republic congo" => {
            push_unique(&mut aliases, "d r congo".to_string());
            push_unique(&mut aliases, "dr congo".to_string());
            push_unique(&mut aliases, "democratic republic congo".to_string());
        }
        "ivory coast" | "cote d ivoire" => {
            push_unique(&mut aliases, "ivory coast".to_string());
            push_unique(&mut aliases, "cote d ivoire".to_string());
        }
        "south korea" | "korea republic" => {
            push_unique(&mut aliases, "south korea".to_string());
            push_unique(&mut aliases, "korea republic".to_string());
        }
        "turkey" | "turkiye" => {
            push_unique(&mut aliases, "turkey".to_string());
            push_unique(&mut aliases, "turkiye".to_string());
        }
        "czech republic" | "czechia" => {
            push_unique(&mut aliases, "czech republic".to_string());
            push_unique(&mut aliases, "czechia".to_string());
        }
        _ => {}
    }
    aliases
}

fn polymarket_search_result_dates(candidate: &Value) -> Vec<String> {
    let mut dates = Vec::new();
    for field in ["startDate", "endDate", "creationDate"] {
        if let Some(value) = candidate.get(field).and_then(Value::as_str) {
            if let Some(date) = polymarket_event_date(value) {
                push_unique(&mut dates, date);
            }
        }
    }
    for field in ["slug", "title"] {
        if let Some(value) = candidate.get(field).and_then(Value::as_str) {
            for date in extract_iso_dates(value) {
                push_unique(&mut dates, date);
            }
        }
    }
    dates
}

fn extract_iso_dates(value: &str) -> Vec<String> {
    Regex::new(r"\b\d{4}-\d{2}-\d{2}\b")
        .ok()
        .map(|regex| {
            regex
                .find_iter(value)
                .map(|match_| match_.as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_polymarket_team_name(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn polymarket_event_date(value: &str) -> Option<String> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(value) {
        return Some(datetime.format("%Y-%m-%d").to_string());
    }

    let date_part = value.split(',').next().unwrap_or(value).trim();
    let mut parts = date_part.split_whitespace();
    let day = parts.next()?.parse::<u32>().ok()?;
    let month = month_number(parts.next()?)?;
    let year = parts.next()?.parse::<i32>().ok()?;

    Some(format!("{year:04}-{month:02}-{day:02}"))
}

pub fn event_has_ended_at(start_time: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Some(event_time) = parse_event_start_time(start_time) else {
        return false;
    };
    event_time < now
}

fn event_has_ended_now(start_time: &str) -> bool {
    event_has_ended_at(start_time, chrono::Utc::now())
}

fn parse_event_start_time(start_time: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(start_time) {
        return Some(datetime.with_timezone(&chrono::Utc));
    }

    let naive = chrono::NaiveDateTime::parse_from_str(start_time.trim(), "%d %b %Y, %H:%M").ok()?;
    Some(naive.and_utc())
}

fn polymarket_event_date_candidates(value: &str) -> Vec<String> {
    let Some(date) = polymarket_event_date(value) else {
        return Vec::new();
    };
    let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") else {
        return vec![date];
    };

    let mut dates = Vec::new();
    if let Some(previous) = naive_date.pred_opt() {
        push_unique(&mut dates, previous.format("%Y-%m-%d").to_string());
    }
    push_unique(&mut dates, date);
    if let Some(next) = naive_date.succ_opt() {
        push_unique(&mut dates, next.format("%Y-%m-%d").to_string());
    }
    dates
}

fn month_number(value: &str) -> Option<u32> {
    match value.to_ascii_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn teams_from_link_or_text(
    href: &str,
    text_parts: &[String],
    text: &str,
) -> Result<(String, String), ScraperError> {
    if text_parts.len() >= 2 {
        return Ok((text_parts[0].clone(), text_parts[1].clone()));
    }

    for separator in [" vs ", " VS ", " - ", " – "] {
        if let Some((home, away)) = text.split_once(separator) {
            let home = home.trim();
            let away = away.trim();
            if !home.is_empty() && !away.is_empty() {
                return Ok((home.to_string(), away.to_string()));
            }
        }
    }

    let h2h_prefix = "/h2h/";
    let Some(h2h_start) = href.find(h2h_prefix) else {
        return Err(ScraperError::Parse(format!(
            "Could not parse event teams from {href}"
        )));
    };
    let path_after_h2h = &href[h2h_start + h2h_prefix.len()..];
    let path_without_fragment = path_after_h2h.split('#').next().unwrap_or(path_after_h2h);
    let parts = path_without_fragment
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err(ScraperError::Parse(format!(
            "Could not parse event teams from {href}"
        )));
    }

    Ok((decode_team_slug(parts[0]), decode_team_slug(parts[1])))
}

fn normalize_event_url(value: &str) -> String {
    let path = strip_oddsportal_host(value.trim());
    let Some((path, fragment)) = path.split_once('#') else {
        return absolutize_oddsportal_path(path);
    };
    let fragment = fragment.trim_matches('/');
    let normalized_path = if fragment.is_empty() || fragment.contains(':') {
        format!("{path}#{fragment}")
    } else {
        format!("{path}#{fragment}:1X2;2")
    };
    absolutize_oddsportal_path(&normalized_path)
}

fn is_h2h_event_url_for_sport(url: &str, sport: &str) -> bool {
    strip_oddsportal_host(url).starts_with(&format!("/{}/h2h/", sport))
}

fn strip_oddsportal_host(value: &str) -> &str {
    for host in [
        "https://www.oddsportal.com",
        "https://oddsportal.com",
        "http://www.oddsportal.com",
        "http://oddsportal.com",
    ] {
        if let Some(relative) = value.strip_prefix(host) {
            return relative;
        }
    }
    value
}

fn absolutize_oddsportal_path(path: &str) -> String {
    if path.starts_with('/') {
        format!("{ODDSPORTAL_BASE_URL}{path}")
    } else {
        path.to_string()
    }
}

fn format_start_date(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.format("%-d %b %Y, %H:%M").to_string())
        .unwrap_or_else(|_| value.to_string())
}

fn nearest_time_text(element: &ElementRef<'_>) -> Option<String> {
    let time_regex =
        Regex::new(r"(?i)\b\d{1,2}\s+[a-z]{3}\s+\d{4},\s*\d{1,2}:\d{2}\b|\b\d{1,2}:\d{2}\b")
            .ok()?;

    for ancestor in element.ancestors().take(4) {
        let Some(parent) = ElementRef::wrap(ancestor) else {
            continue;
        };
        let text = parent
            .text()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if let Some(found) = time_regex.find(&text) {
            return Some(found.as_str().trim().to_string());
        }
    }

    None
}

fn decode_team_slug(segment: &str) -> String {
    let mut parts = segment.split('-').collect::<Vec<_>>();
    if parts
        .last()
        .is_some_and(|last| looks_like_oddsportal_id(last))
    {
        parts.pop();
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(title_case_ascii)
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_oddsportal_id(value: &str) -> bool {
    value.len() >= 6 && value.chars().any(|c| c.is_ascii_digit())
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
    }
}

fn slugify(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
