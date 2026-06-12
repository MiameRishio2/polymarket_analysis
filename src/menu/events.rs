//! Event list parsing and API handlers for OddsPortal competition pages

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::DateTime;
use regex::Regex;
use rs_clob_client_v2::{Chain, ClobClient};
use scraper::{ElementRef, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::handlers::{ApiResponse, AppState};
use super::models::default_refreshed_at;
use super::scraper::{fetch_url, ScraperError};
use super::storage::{Storage, StorageError};

const ODDSPORTAL_BASE_URL: &str = "https://www.oddsportal.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
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
    let url = format!(
        "https://www.oddsportal.com/{}/{}/{}/",
        sport, category, league
    );
    let html = fetch_url(&url).await?;
    let mut events = extract_events_for_competition(&html, sport, category, league)?;
    enrich_events_with_polymarket_urls(&mut events, sport, category, league).await;
    Ok(events)
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
    events.push(EventRow {
        slug,
        home_team,
        away_team,
        matchup,
        start_time,
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
        return;
    }

    let Ok(client) = build_polymarket_client() else {
        return;
    };

    for event in events {
        if event.polymarket_url.is_some() {
            continue;
        }

        for slug in polymarket_slug_candidates(event, sport, category, league) {
            if let Some(url) =
                lookup_polymarket_url_by_slug(&client, &slug, sport, category, league).await
            {
                event.polymarket_url = Some(url);
                break;
            }
        }
    }
}

fn build_polymarket_client() -> rs_clob_client_v2::ClobResult<ClobClient> {
    ClobClient::new(
        "https://clob.polymarket.com".to_string(),
        "https://gamma-api.polymarket.com".to_string(),
        Chain::Polygon,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
    )
}

async fn lookup_polymarket_url_by_slug(
    client: &ClobClient,
    slug: &str,
    sport: &str,
    category: &str,
    league: &str,
) -> Option<String> {
    if let Ok(event) = client.get_event_by_slug(slug).await {
        if event.slug.as_deref() == Some(slug) {
            return polymarket_public_url_for_slug(slug, sport, category, league);
        }
    }

    if let Ok(market) = client.get_market_by_slug(slug).await {
        if market.slug.as_deref() == Some(slug) {
            return polymarket_public_url_for_slug(slug, sport, category, league);
        }
    }

    None
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
    let Some(date) = polymarket_event_date(&event.start_time) else {
        return Vec::new();
    };

    vec![format!("fifwc-{home_code}-{away_code}-{date}")]
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
        "australia" => Some("aus"),
        "austria" => Some("aut"),
        "belgium" => Some("bel"),
        "brazil" => Some("bra"),
        "canada" => Some("can"),
        "chile" => Some("chi"),
        "china" => Some("chn"),
        "colombia" => Some("col"),
        "croatia" => Some("cro"),
        "czech republic" | "czechia" => Some("cze"),
        "denmark" => Some("den"),
        "ecuador" => Some("ecu"),
        "egypt" => Some("egy"),
        "england" => Some("eng"),
        "france" => Some("fra"),
        "germany" => Some("ger"),
        "ghana" => Some("gha"),
        "greece" => Some("gre"),
        "iran" => Some("irn"),
        "italy" => Some("ita"),
        "japan" => Some("jpn"),
        "morocco" => Some("mar"),
        "mexico" => Some("mex"),
        "netherlands" => Some("ned"),
        "new zealand" => Some("nzl"),
        "nigeria" => Some("nga"),
        "norway" => Some("nor"),
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
        "wales" => Some("wal"),
        _ => None,
    }
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
