//! Event list parsing and API handlers for OddsPortal competition pages

use axum::{extract::{Path, State}, Json};
use regex::Regex;
use scraper::{ElementRef, Selector};
use serde::{Deserialize, Serialize};

use super::handlers::{ApiResponse, AppState};
use super::scraper::{fetch_url, ScraperError};
use super::storage::{Storage, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub sport: String,
    pub events: Vec<EventRow>,
    pub last_updated: String,
    pub source: String,
}

pub fn event_cache_key(sport: &str, category: &str, league: &str) -> String {
    format!("events_{}_{}_{}", sport, category, league)
}

fn event_sport_key(sport: &str, category: &str, league: &str) -> String {
    format!("{}/{}/{}", sport, category, league)
}

pub fn save_event_data(storage: &Storage, key: &str, data: &EventData) -> Result<(), StorageError> {
    let events_json = serde_json::to_string(&data.events)?;
    storage.save_raw_json(key, &events_json, &data.last_updated, &data.source)
}

pub fn load_event_data(
    storage: &Storage,
    key: &str,
    sport_key: String,
) -> Result<Option<EventData>, StorageError> {
    let Some((events_json, last_updated, source)) = storage.load_raw_json(key)? else {
        return Ok(None);
    };
    let events = serde_json::from_str::<Vec<EventRow>>(&events_json)?;
    Ok(Some(EventData {
        sport: sport_key,
        events,
        last_updated,
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

    for element in document.select(&link_selector) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        if !href.starts_with(&format!("/{}/h2h/", sport)) {
            continue;
        }

        let text_parts = element
            .text()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let text = text_parts.join(" ");
        let (home_team, away_team) = teams_from_link_or_text(href, &text_parts, &text)?;
        let start_time = nearest_time_text(&element).unwrap_or_default();
        let slug = format!("{}-vs-{}", slugify(&home_team), slugify(&away_team));
        let matchup = format!("{} VS {}", home_team, away_team);

        if events.iter().any(|event: &EventRow| event.url == href) {
            continue;
        }

        events.push(EventRow {
            slug,
            home_team,
            away_team,
            matchup,
            start_time,
            url: href.to_string(),
        });
    }

    Ok(events)
}

pub async fn fetch_events_for_competition(
    sport: &str,
    category: &str,
    league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let url = format!("https://www.oddsportal.com/{}/{}/{}/", sport, category, league);
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

async fn fetch_event_data(sport: &str, category: &str, league: &str) -> EventData {
    match fetch_events_for_competition(sport, category, league).await {
        Ok(events) => EventData {
            sport: event_sport_key(sport, category, league),
            events,
            last_updated: chrono::Utc::now().to_rfc3339(),
            source: "scraped".to_string(),
        },
        Err(e) => {
            tracing::error!("Failed to fetch events for {}/{}/{}: {}", sport, category, league, e);
            EventData {
                sport: event_sport_key(sport, category, league),
                events: Vec::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                source: "error".to_string(),
            }
        }
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
        return Err(ScraperError::Parse(format!("Could not parse event teams from {href}")));
    };
    let path_after_h2h = &href[h2h_start + h2h_prefix.len()..];
    let path_without_fragment = path_after_h2h.split('#').next().unwrap_or(path_after_h2h);
    let parts = path_without_fragment
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err(ScraperError::Parse(format!("Could not parse event teams from {href}")));
    }

    Ok((decode_team_slug(parts[0]), decode_team_slug(parts[1])))
}

fn nearest_time_text(element: &ElementRef<'_>) -> Option<String> {
    let time_regex = Regex::new(
        r"(?i)\b\d{1,2}\s+[a-z]{3}\s+\d{4},\s*\d{1,2}:\d{2}\b|\b\d{1,2}:\d{2}\b",
    )
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
    if parts.last().is_some_and(|last| looks_like_oddsportal_id(last)) {
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
