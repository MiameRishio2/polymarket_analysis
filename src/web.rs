use anyhow::Result;
use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    response::{Html, Json},
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
};
use tokio::{
    net::TcpListener,
    sync::Mutex,
    time::{Duration, sleep},
};
use tracing::{info, warn};

use crate::config::{AppConfig, SportConfig};
use crate::http::build_http_client;
use crate::providers::sports_scraper;
use crate::scheduler::{NewScheduledMatch, SchedulerCache};
use crate::storage::{
    AnalysisDebugPoint, AnalysisLatestOdds, AnalysisMatchSummary, AnalysisOddsSeriesPoint,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SportMatchesData {
    pub sport_name: String,
    pub sections: Vec<MatchSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchSection {
    pub section_name: String,
    pub matches: Vec<MatchInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchInfo {
    pub team1: String,
    pub team2: String,
    pub match_time: String,
    #[serde(default)]
    pub end_time: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub is_finished: bool,
    pub score: Option<String>,
    pub partial_score: Option<String>,
    pub polymarket_url: Option<String>,
    pub oddsportal_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogSport {
    pub sport_name: String,
    pub sport_slug: String,
    pub section_count: usize,
    pub cached_match_count: usize,
    pub last_loaded_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogSection {
    pub game_name: String,
    pub game_slug: String,
    pub section_name: String,
    pub section_slug: String,
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub match_count: usize,
    pub last_loaded_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TournamentSection {
    pub section_name: String,
    pub section_slug: String,
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub match_count: usize,
    pub last_loaded_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameSection {
    pub game_name: String,
    pub game_slug: String,
    pub group_slug: String,
    pub oddsportal_url: String,
    pub tournament_count: usize,
    pub match_count: usize,
    pub last_loaded_at: Option<String>,
    pub tournaments: Vec<TournamentSection>,
}

#[derive(Clone, Debug, Serialize)]
struct SportSectionsResponse {
    sport_name: String,
    sport_slug: String,
    last_loaded_at: Option<String>,
    sections: Vec<CatalogSection>,
}

#[derive(Clone, Debug, Serialize)]
struct EsportsSectionsResponse {
    sport_name: String,
    sport_slug: String,
    last_loaded_at: Option<String>,
    games: Vec<GameSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchCache {
    sections: HashMap<String, MatchCacheEntry>,
}

impl MatchCache {
    pub fn empty() -> Self {
        Self {
            sections: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MatchCacheEntry {
    last_loaded_at: String,
    matches: Vec<MatchInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SectionCache {
    sports: HashMap<String, SectionCacheEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SectionCacheEntry {
    last_loaded_at: String,
    sections: Vec<CatalogSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CatalogCache {
    last_loaded_at: String,
    sports: Vec<CatalogSport>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TournamentCache {
    groups: HashMap<String, TournamentCacheEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TournamentCacheEntry {
    last_loaded_at: String,
    tournaments: Vec<TournamentSection>,
}

#[derive(Clone, Debug, Deserialize)]
struct SectionQuery {
    refresh: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
struct CatalogQuery {
    refresh: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
struct SectionResponse {
    section_name: String,
    section_slug: String,
    last_loaded_at: Option<String>,
    matches: Vec<MatchInfo>,
}

#[derive(Clone, Debug, Serialize)]
struct TournamentsResponse {
    group_name: String,
    group_slug: String,
    oddsportal_url: String,
    last_loaded_at: Option<String>,
    tournaments: Vec<TournamentSection>,
}

#[derive(Clone, Debug, Serialize)]
struct SchedulerResponse {
    matches: Vec<crate::scheduler::ScheduledMatch>,
}

#[derive(Clone, Debug, Serialize)]
struct ForceSchedulerResponse {
    matches: Vec<crate::scheduler::ScheduledMatch>,
    started: bool,
    schedule_id: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct AnalysisResponse {
    db_path: String,
    scheduled_matches: Vec<crate::scheduler::ScheduledMatch>,
    collected_matches: Vec<AnalysisMatchSummary>,
    debug_points: Vec<AnalysisDebugPoint>,
}

#[derive(Clone, Debug, Serialize)]
struct DeleteMatchResponse {
    deleted_rows: u64,
    analysis: AnalysisResponse,
}

#[derive(Clone, Debug, Serialize)]
struct AnalysisLatestOddsResponse {
    success: bool,
    latest: Option<AnalysisLatestOdds>,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct AnalysisOddsSeriesResponse {
    success: bool,
    points: Vec<AnalysisOddsSeriesPoint>,
    error: Option<String>,
}

pub fn group_matches_by_category(entries: Vec<(String, Vec<MatchInfo>)>) -> Vec<SportMatchesData> {
    let mut categories: Vec<SportMatchesData> = Vec::new();

    for (raw_name, matches) in entries {
        if matches.is_empty() {
            continue;
        }

        let (category_name, section_name, _) = display_hierarchy_for(&raw_name);

        let category_index = match categories
            .iter()
            .position(|category| category.sport_name == category_name)
        {
            Some(index) => index,
            None => {
                categories.push(SportMatchesData {
                    sport_name: category_name.clone(),
                    sections: Vec::new(),
                });
                categories.len() - 1
            }
        };

        categories[category_index].sections.push(MatchSection {
            section_name,
            matches,
        });
    }

    categories
}

pub fn group_matches_by_config(
    configs: &[SportConfig],
    entries: Vec<(String, Vec<MatchInfo>)>,
) -> Vec<SportMatchesData> {
    let mut categories: Vec<SportMatchesData> = Vec::new();

    for (index, (raw_name, matches)) in entries.into_iter().enumerate() {
        let (category_name, section_name, _) = configs
            .get(index)
            .map(|config| display_hierarchy_for_config(config, &raw_name))
            .unwrap_or_else(|| display_hierarchy_for(&raw_name));

        let category_index = match categories
            .iter()
            .position(|category| category.sport_name == category_name)
        {
            Some(index) => index,
            None => {
                categories.push(SportMatchesData {
                    sport_name: category_name.clone(),
                    sections: Vec::new(),
                });
                categories.len() - 1
            }
        };

        categories[category_index].sections.push(MatchSection {
            section_name,
            matches,
        });
    }

    categories
}

fn display_hierarchy_for_config(
    config: &SportConfig,
    raw_name: &str,
) -> (String, String, Option<String>) {
    if let Ok(url) = url::Url::parse(&config.oddsportal_url) {
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();

        if segments.first() == Some(&"esports") {
            let game = segments.get(1).copied().unwrap_or(raw_name);
            let tournament = if segments.len() >= 3 {
                Some(segments.get(2).copied().unwrap_or(raw_name))
            } else {
                None
            };

            return (
                "Esports".to_string(),
                titleize_game(game),
                tournament.map(titleize),
            );
        }

        if segments.first() == Some(&"football") {
            return ("Football".to_string(), "Football".to_string(), None);
        }

        if segments.first() == Some(&"basketball") {
            return ("Basketball".to_string(), "Basketball".to_string(), None);
        }

        if let Some(sport_slug) = segments.first() {
            let sport_name = sport_name_for_slug(sport_slug);
            let group_name = segments
                .get(1)
                .map(|segment| {
                    if *sport_slug == "esports" {
                        titleize_game(segment)
                    } else {
                        titleize(segment)
                    }
                })
                .unwrap_or_else(|| sport_name.clone());
            let tournament = segments.get(2).map(|segment| titleize(segment));
            return (sport_name, group_name, tournament);
        }
    }

    let (sport, section, _) = display_hierarchy_for(raw_name);
    (sport, section, None)
}

fn display_hierarchy_for(raw_name: &str) -> (String, String, Option<String>) {
    let normalized = raw_name.trim().to_lowercase().replace('_', "-");

    match normalized.as_str() {
        "football" => ("Football".to_string(), "Football".to_string(), None),
        "basketball" => ("Basketball".to_string(), "Basketball".to_string(), None),
        "dota-2" | "dota2" => ("Esports".to_string(), "Dota 2".to_string(), None),
        "league-of-legends" | "lol" => {
            ("Esports".to_string(), "League of Legends".to_string(), None)
        }
        "counter-strike" | "cs2" | "csgo" => {
            ("Esports".to_string(), "Counter-Strike".to_string(), None)
        }
        "esports" => ("Esports".to_string(), "Esports".to_string(), None),
        _ => (titleize(raw_name), titleize(raw_name), None),
    }
}

fn titleize(name: &str) -> String {
    name.split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn titleize_game(name: &str) -> String {
    match name.trim().to_lowercase().replace('_', "-").as_str() {
        "dota-2" | "dota2" => "Dota 2".to_string(),
        "league-of-legends" | "lol" => "League of Legends".to_string(),
        "counter-strike" | "cs2" | "csgo" => "Counter-Strike".to_string(),
        _ => titleize(name),
    }
}

fn canonical_tournament_slug(segments: &[&str], tournament_slug: &str, link_text: &str) -> String {
    if segments.first() == Some(&"esports")
        && let Some(game_slug) = segments.get(1)
        && let Some(stripped) = tournament_slug.strip_prefix(&format!("{}-", game_slug))
    {
        return stripped.to_string();
    }

    if segments == ["football", "world"] {
        let normalized = tournament_slug.trim_matches('/').to_lowercase();
        if normalized == "football-world-world-cup-2026" {
            return "world-cup-2026".to_string();
        }
    }

    if segments.first() == Some(&"tennis") {
        if let Some(canonical) = canonical_tennis_tournament_slug(tournament_slug, link_text) {
            return canonical;
        }
    }

    tournament_slug.to_string()
}

fn canonical_tennis_tournament_slug(tournament_slug: &str, link_text: &str) -> Option<String> {
    let slug = tournament_slug.trim_matches('/').to_lowercase();
    let text = link_text.to_lowercase();
    let gender = if slug.ends_with("-women") || text.contains("women - singles") {
        "women"
    } else if slug.ends_with("-men") || text.contains("men - singles") {
        "men"
    } else {
        return None;
    };

    let prefix = if gender == "men" { "itf-m" } else { "itf-w" };
    let suffix = if gender == "men" { "-men" } else { "-women" };
    let rest = slug.strip_prefix(prefix)?.strip_suffix(suffix)?;

    if rest.is_empty() {
        return None;
    }

    Some(format!("itf-{}-singles-{}{}", gender, &prefix[4..], rest))
}

fn decode_web_text(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&rsquo;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn cache_key_for_config(config: &SportConfig) -> String {
    // 使用 URL 路径来生成唯一的缓存键，而不是只使用 name
    if let Ok(url) = url::Url::parse(&config.oddsportal_url) {
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();

        // 对于电竞路径，使用完整的路径段
        if segments.first() == Some(&"esports") {
            return segments.join("-");
        }
    }

    // 对于其他情况，回退到原始方式
    config.name.trim().to_lowercase().replace('_', "-")
}

fn cache_path(config: &AppConfig) -> std::path::PathBuf {
    config
        .db
        .parent()
        .map(|parent| parent.join("web_match_cache.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/web_match_cache.json"))
}

fn section_cache_path(config: &AppConfig) -> std::path::PathBuf {
    config
        .db
        .parent()
        .map(|parent| parent.join("web_section_cache.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/web_section_cache.json"))
}

fn catalog_cache_path(config: &AppConfig) -> std::path::PathBuf {
    config
        .db
        .parent()
        .map(|parent| parent.join("web_catalog_cache.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/web_catalog_cache.json"))
}

fn tournament_cache_path(config: &AppConfig) -> std::path::PathBuf {
    config
        .db
        .parent()
        .map(|parent| parent.join("web_tournament_cache.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/web_tournament_cache.json"))
}

async fn read_match_cache(config: &AppConfig) -> MatchCache {
    let path = cache_path(config);
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return MatchCache {
            sections: HashMap::new(),
        };
    };

    serde_json::from_str(&content).unwrap_or_else(|_| MatchCache {
        sections: HashMap::new(),
    })
}

async fn read_section_cache(config: &AppConfig) -> SectionCache {
    let path = section_cache_path(config);
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return SectionCache {
            sports: HashMap::new(),
        };
    };

    serde_json::from_str(&content).unwrap_or_else(|_| SectionCache {
        sports: HashMap::new(),
    })
}

async fn write_section_cache(config: &AppConfig, cache: &SectionCache) -> Result<()> {
    let path = section_cache_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(cache)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

async fn read_catalog_cache(config: &AppConfig) -> Option<CatalogCache> {
    let path = catalog_cache_path(config);
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return None;
    };
    serde_json::from_str(&content).ok()
}

async fn write_catalog_cache(config: &AppConfig, cache: &CatalogCache) -> Result<()> {
    let path = catalog_cache_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(cache)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

async fn read_tournament_cache(config: &AppConfig) -> TournamentCache {
    let path = tournament_cache_path(config);
    let Ok(content) = tokio::fs::read_to_string(path).await else {
        return TournamentCache {
            groups: HashMap::new(),
        };
    };
    serde_json::from_str(&content).unwrap_or_else(|_| TournamentCache {
        groups: HashMap::new(),
    })
}

async fn write_tournament_cache(config: &AppConfig, cache: &TournamentCache) -> Result<()> {
    let path = tournament_cache_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(cache)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

async fn write_match_cache(config: &AppConfig, cache: &MatchCache) -> Result<()> {
    let path = cache_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(cache)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

fn build_catalog(config: &AppConfig, cache: &MatchCache) -> Vec<CatalogSport> {
    let mut catalog: Vec<CatalogSport> = Vec::new();

    for sport_config in &config.scrape_sports.sports {
        let (sport_name, _, _) = display_hierarchy_for_config(sport_config, &sport_config.name);
        let sport_slug = slug_for_sport_name(&sport_name);

        if catalog
            .iter()
            .position(|sport| sport.sport_name == sport_name)
            .is_some()
        {
            continue;
        }

        let cached_sections = config
            .scrape_sports
            .sports
            .iter()
            .filter(|config| display_hierarchy_for_config(config, &config.name).0 == sport_name)
            .filter_map(|config| cache.sections.get(&cache_key_for_config(config)))
            .collect::<Vec<_>>();

        catalog.push(CatalogSport {
            sport_name,
            sport_slug,
            section_count: 0,
            cached_match_count: cached_sections
                .iter()
                .map(|entry| entry.matches.len())
                .sum(),
            last_loaded_at: cached_sections
                .iter()
                .filter_map(|entry| Some(entry.last_loaded_at.clone()))
                .max(),
        });
    }

    catalog
}

async fn load_or_refresh_catalog(config: &AppConfig, refresh: bool) -> Vec<CatalogSport> {
    let cache = read_match_cache(config).await;
    if !refresh {
        if let Some(entry) = read_catalog_cache(config).await {
            return merge_configured_catalog_sports(
                refresh_catalog_counts(entry.sports, &cache),
                config,
                &cache,
            );
        }
        return refresh_catalog_counts(build_catalog(config, &cache), &cache);
    }

    let client = match build_http_client(config.proxy_enabled, &config.proxy) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!("failed to build HTTP client for catalog refresh: {}", error);
            return build_catalog(config, &cache);
        }
    };

    let html = match client
        .get("https://www.oddsportal.com/")
        .send()
        .await
        .and_then(|response| response.error_for_status())
    {
        Ok(response) => response.text().await.unwrap_or_default(),
        Err(error) => {
            tracing::warn!("failed to refresh OddsPortal catalog: {}", error);
            return build_catalog(config, &cache);
        }
    };

    let mut catalog = parse_catalog_sports(&html, config, &cache);
    if catalog.is_empty() {
        catalog = build_catalog(config, &cache);
    }
    catalog = refresh_catalog_counts(catalog, &cache);
    catalog = merge_configured_catalog_sports(catalog, config, &cache);

    if !catalog.is_empty() {
        let catalog_cache = CatalogCache {
            last_loaded_at: Utc::now().to_rfc3339(),
            sports: catalog.clone(),
        };
        if let Err(error) = write_catalog_cache(config, &catalog_cache).await {
            tracing::warn!("failed to write web catalog cache: {}", error);
        }
    }
    catalog
}

pub fn parse_catalog_sports(
    html: &str,
    config: &AppConfig,
    cache: &MatchCache,
) -> Vec<CatalogSport> {
    let Ok(link_re) = regex::Regex::new(r#"href="(/([a-z][a-z0-9-]*)/)""#) else {
        return Vec::new();
    };
    let known = [
        "football",
        "basketball",
        "tennis",
        "baseball",
        "hockey",
        "american-football",
        "aussie-rules",
        "badminton",
        "beach-soccer",
        "beach-volleyball",
        "boxing",
        "cricket",
        "darts",
        "esports",
        "futsal",
        "handball",
        "mma",
        "rugby-league",
        "rugby-union",
        "snooker",
        "table-tennis",
        "volleyball",
        "water-polo",
    ];

    let mut seen = std::collections::HashSet::new();
    let mut catalog = Vec::new();
    for cap in link_re.captures_iter(html) {
        let slug = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if !known.contains(&slug) || !seen.insert(slug.to_string()) {
            continue;
        }

        let sport_name = sport_name_for_slug(slug);
        let cached_sections = config
            .scrape_sports
            .sports
            .iter()
            .filter(|sport_config| {
                let (config_sport_name, _, _) =
                    display_hierarchy_for_config(sport_config, &sport_config.name);
                slug_for_sport_name(&config_sport_name) == slug
            })
            .filter_map(|sport_config| cache.sections.get(&cache_key_for_config(sport_config)))
            .collect::<Vec<_>>();

        catalog.push(CatalogSport {
            sport_name,
            sport_slug: slug.to_string(),
            section_count: 0,
            cached_match_count: cached_sections
                .iter()
                .map(|entry| entry.matches.len())
                .sum(),
            last_loaded_at: cached_sections
                .iter()
                .map(|entry| entry.last_loaded_at.clone())
                .max(),
        });
    }

    catalog.sort_by(|a, b| a.sport_name.cmp(&b.sport_name));
    catalog
}

fn slug_for_sport_name(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
}

fn path_key(segments: &[&str]) -> String {
    segments
        .iter()
        .filter(|segment| !segment.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("__")
}

fn path_segments_from_key(key: &str) -> Vec<&str> {
    if key.contains("__") {
        key.split("__")
            .filter(|segment| !segment.is_empty())
            .collect()
    } else {
        key.split('-')
            .filter(|segment| !segment.is_empty())
            .collect()
    }
}

fn match_count_for_sport(sport_slug: &str, match_cache: &MatchCache) -> usize {
    match_cache
        .sections
        .iter()
        .filter(|(key, _)| cache_key_belongs_to_sport(key, sport_slug))
        .map(|(_, entry)| entry.matches.len())
        .sum()
}

fn last_loaded_for_sport(sport_slug: &str, match_cache: &MatchCache) -> Option<String> {
    match_cache
        .sections
        .iter()
        .filter(|(key, _)| cache_key_belongs_to_sport(key, sport_slug))
        .map(|(_, entry)| entry.last_loaded_at.clone())
        .max()
}

fn cache_key_belongs_to_sport(key: &str, sport_slug: &str) -> bool {
    if key == sport_slug || key.starts_with(&format!("{}__", sport_slug)) {
        return true;
    }

    sport_slug == "esports"
        && (key.starts_with("esports-")
            || matches!(key, "dota-2" | "league-of-legends" | "counter-strike"))
}

fn match_count_for_group(group_slug: &str, match_cache: &MatchCache) -> usize {
    let legacy_prefix = group_slug.replace("__", "-");
    match_cache
        .sections
        .iter()
        .filter(|(key, _)| {
            key == &group_slug
                || key.starts_with(&format!("{}__", group_slug))
                || key == &&legacy_prefix
                || key.starts_with(&format!("{}-", legacy_prefix))
        })
        .map(|(_, entry)| entry.matches.len())
        .sum()
}

fn last_loaded_for_group(group_slug: &str, match_cache: &MatchCache) -> Option<String> {
    let legacy_prefix = group_slug.replace("__", "-");
    match_cache
        .sections
        .iter()
        .filter(|(key, _)| {
            key == &group_slug
                || key.starts_with(&format!("{}__", group_slug))
                || key == &&legacy_prefix
                || key.starts_with(&format!("{}-", legacy_prefix))
        })
        .map(|(_, entry)| entry.last_loaded_at.clone())
        .max()
}

fn match_count_for_section(section_slug: &str, match_cache: &MatchCache) -> usize {
    match_cache
        .sections
        .get(section_slug)
        .map(|entry| entry.matches.len())
        .or_else(|| {
            match_cache
                .sections
                .get(&section_slug.replace("__", "-"))
                .map(|entry| entry.matches.len())
        })
        .unwrap_or(0)
}

fn last_loaded_for_section(section_slug: &str, match_cache: &MatchCache) -> Option<String> {
    match_cache
        .sections
        .get(section_slug)
        .map(|entry| entry.last_loaded_at.clone())
        .or_else(|| {
            match_cache
                .sections
                .get(&section_slug.replace("__", "-"))
                .map(|entry| entry.last_loaded_at.clone())
        })
}

fn refresh_catalog_counts(
    mut catalog: Vec<CatalogSport>,
    match_cache: &MatchCache,
) -> Vec<CatalogSport> {
    for sport in &mut catalog {
        sport.cached_match_count = match_count_for_sport(&sport.sport_slug, match_cache);
        sport.last_loaded_at = last_loaded_for_sport(&sport.sport_slug, match_cache);
    }
    catalog
}

fn merge_configured_catalog_sports(
    mut catalog: Vec<CatalogSport>,
    config: &AppConfig,
    match_cache: &MatchCache,
) -> Vec<CatalogSport> {
    for configured in build_catalog(config, match_cache) {
        if catalog
            .iter()
            .any(|sport| sport.sport_slug == configured.sport_slug)
        {
            continue;
        }
        catalog.push(configured);
    }

    catalog.sort_by(|a, b| a.sport_name.cmp(&b.sport_name));
    catalog
}

fn refresh_game_counts(mut games: Vec<GameSection>, match_cache: &MatchCache) -> Vec<GameSection> {
    for game in &mut games {
        game.match_count = match_count_for_group(&game.group_slug, match_cache);
        game.last_loaded_at = last_loaded_for_group(&game.group_slug, match_cache);
        for tournament in &mut game.tournaments {
            tournament.match_count = match_count_for_section(&tournament.section_slug, match_cache);
            tournament.last_loaded_at =
                last_loaded_for_section(&tournament.section_slug, match_cache);
        }
    }
    games
}

fn refresh_tournament_counts(
    mut tournaments: Vec<TournamentSection>,
    match_cache: &MatchCache,
) -> Vec<TournamentSection> {
    for tournament in &mut tournaments {
        tournament.match_count = match_count_for_section(&tournament.section_slug, match_cache);
        tournament.last_loaded_at = last_loaded_for_section(&tournament.section_slug, match_cache);
    }
    tournaments
}

fn polymarket_url_for_path(segments: &[&str]) -> String {
    if segments.first() == Some(&"esports") {
        let game_slug = segments.get(1).copied().unwrap_or("esports");
        if let Some(tournament_slug) = segments.get(2) {
            let polymarket_tournament_slug = tournament_slug
                .strip_prefix(&format!("{}-", game_slug))
                .unwrap_or(tournament_slug);
            format!(
                "https://polymarket.com/esports/{}/{}",
                game_slug, polymarket_tournament_slug
            )
        } else {
            format!("https://polymarket.com/esports/{}/games", game_slug)
        }
    } else {
        format!(
            "https://polymarket.com/sports/{}",
            segments.first().copied().unwrap_or("sports")
        )
    }
}

fn oddsportal_esports_tournament_slug(game_slug: &str, tournament_slug: &str) -> String {
    if game_slug == "dota-2" && !tournament_slug.starts_with("dota-2-") {
        format!("dota-2-{tournament_slug}")
    } else {
        tournament_slug.to_string()
    }
}

fn sport_name_for_slug(slug: &str) -> String {
    match slug {
        "football" => "Football".to_string(),
        "basketball" => "Basketball".to_string(),
        "tennis" => "Tennis".to_string(),
        "baseball" => "Baseball".to_string(),
        "hockey" => "Hockey".to_string(),
        "american-football" => "American Football".to_string(),
        "aussie-rules" => "Aussie Rules".to_string(),
        "badminton" => "Badminton".to_string(),
        "beach-soccer" => "Beach Soccer".to_string(),
        "beach-volleyball" => "Beach Volleyball".to_string(),
        "volleyball" => "Volleyball".to_string(),
        "water-polo" => "Water Polo".to_string(),
        "boxing" => "Boxing".to_string(),
        "cricket" => "Cricket".to_string(),
        "darts" => "Darts".to_string(),
        "esports" => "Esports".to_string(),
        "futsal" => "Futsal".to_string(),
        "handball" => "Handball".to_string(),
        "mma" => "MMA".to_string(),
        "rugby-league" => "Rugby League".to_string(),
        "rugby-union" => "Rugby Union".to_string(),
        "snooker" => "Snooker".to_string(),
        "table-tennis" => "Table Tennis".to_string(),
        _ => titleize(slug),
    }
}

async fn load_or_refresh_sport_sections(
    config: &AppConfig,
    sport_slug: &str,
    refresh: bool,
) -> EsportsSectionsResponse {
    let mut section_cache = read_section_cache(config).await;
    let match_cache = read_match_cache(config).await;

    if !refresh {
        if let Some(entry) = section_cache.sports.get(sport_slug) {
            let games = refresh_game_counts(
                entry
                    .sections
                    .iter()
                    .map(game_section_from_catalog_section)
                    .collect(),
                &match_cache,
            );
            return EsportsSectionsResponse {
                sport_name: sport_name_for_slug(sport_slug),
                sport_slug: sport_slug.to_string(),
                last_loaded_at: Some(entry.last_loaded_at.clone()),
                games,
            };
        }
    }

    let games = refresh_game_counts(
        fetch_sport_groups(config, sport_slug, &match_cache).await,
        &match_cache,
    );
    let sections: Vec<CatalogSection> = games
        .iter()
        .map(catalog_section_from_game_section)
        .collect();

    let last_loaded_at = Utc::now().to_rfc3339();
    section_cache.sports.insert(
        sport_slug.to_string(),
        SectionCacheEntry {
            last_loaded_at: last_loaded_at.clone(),
            sections: sections.clone(),
        },
    );
    if let Err(error) = write_section_cache(config, &section_cache).await {
        tracing::warn!("failed to write web section cache: {}", error);
    }

    EsportsSectionsResponse {
        sport_name: sport_name_for_slug(sport_slug),
        sport_slug: sport_slug.to_string(),
        last_loaded_at: Some(last_loaded_at),
        games,
    }
}

fn game_section_from_catalog_section(section: &CatalogSection) -> GameSection {
    GameSection {
        game_name: section.game_name.clone(),
        game_slug: section.game_slug.clone(),
        group_slug: section.section_slug.clone(),
        oddsportal_url: section.oddsportal_url.clone(),
        tournament_count: section.match_count,
        match_count: section.match_count,
        last_loaded_at: section.last_loaded_at.clone(),
        tournaments: Vec::new(),
    }
}

fn catalog_section_from_game_section(game: &GameSection) -> CatalogSection {
    CatalogSection {
        game_name: game.game_name.clone(),
        game_slug: game.game_slug.clone(),
        section_name: game.game_name.clone(),
        section_slug: game.group_slug.clone(),
        oddsportal_url: game.oddsportal_url.clone(),
        polymarket_url: polymarket_url_for_path(&path_segments_from_key(&game.group_slug)),
        match_count: game.tournament_count,
        last_loaded_at: game.last_loaded_at.clone(),
    }
}

async fn fetch_sport_groups(
    config: &AppConfig,
    sport_slug: &str,
    match_cache: &MatchCache,
) -> Vec<GameSection> {
    let client = match build_http_client(config.proxy_enabled, &config.proxy) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!("failed to build HTTP client for sport sections: {}", error);
            return configured_sport_groups(config, sport_slug, match_cache);
        }
    };

    let sport_url = format!("https://www.oddsportal.com/{}/", sport_slug);
    let html = match client
        .get(&sport_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
    {
        Ok(response) => response.text().await.unwrap_or_default(),
        Err(error) => {
            tracing::warn!("failed to load sport sections {}: {}", sport_url, error);
            return configured_sport_groups(config, sport_slug, match_cache);
        }
    };

    let mut games = parse_sport_groups(&html, sport_slug, match_cache);

    if games.is_empty() {
        games = configured_sport_groups(config, sport_slug, match_cache);
    }
    games.sort_by(|a, b| a.game_name.cmp(&b.game_name));
    games
}

fn configured_sport_groups(
    config: &AppConfig,
    sport_slug: &str,
    match_cache: &MatchCache,
) -> Vec<GameSection> {
    let mut games_map: std::collections::HashMap<String, GameSection> =
        std::collections::HashMap::new();

    for sport_config in &config.scrape_sports.sports {
        let (sport_name, _, _) = display_hierarchy_for_config(sport_config, &sport_config.name);
        if slug_for_sport_name(&sport_name) != sport_slug {
            continue;
        }

        if let Ok(url) = url::Url::parse(&sport_config.oddsportal_url) {
            let segments: Vec<&str> = url
                .path_segments()
                .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
                .unwrap_or_default();

            if segments.len() >= 2 {
                let group_slug = path_key(&segments[0..2]);
                let child_slug = segments.get(1).copied().unwrap_or("");
                games_map
                    .entry(group_slug.clone())
                    .or_insert_with(|| GameSection {
                        game_name: if sport_slug == "esports" {
                            titleize_game(child_slug)
                        } else {
                            titleize(child_slug)
                        },
                        game_slug: child_slug.to_string(),
                        group_slug: group_slug.clone(),
                        oddsportal_url: format!(
                            "https://www.oddsportal.com/{}/{}/",
                            sport_slug, child_slug
                        ),
                        tournament_count: 0,
                        match_count: 0,
                        last_loaded_at: match_cache
                            .sections
                            .iter()
                            .filter(|(key, _)| key.starts_with(&group_slug))
                            .filter_map(|(_, entry)| Some(entry.last_loaded_at.clone()))
                            .max(),
                        tournaments: Vec::new(),
                    });
            }
        }
    }

    let mut games: Vec<GameSection> = games_map.into_values().collect();
    games.sort_by(|a, b| a.game_name.cmp(&b.game_name));

    if games.is_empty() {
        return default_sport_groups(sport_slug);
    }

    games
}

fn default_sport_groups(sport_slug: &str) -> Vec<GameSection> {
    vec![GameSection {
        game_name: sport_name_for_slug(sport_slug),
        game_slug: sport_slug.to_string(),
        group_slug: path_key(&[sport_slug]),
        oddsportal_url: format!("https://www.oddsportal.com/{}/", sport_slug),
        tournament_count: 0,
        match_count: 0,
        last_loaded_at: None,
        tournaments: Vec::new(),
    }]
}

pub fn parse_sport_groups(
    html: &str,
    sport_slug: &str,
    match_cache: &MatchCache,
) -> Vec<GameSection> {
    let Ok(link_re) = regex::Regex::new(&format!(
        r#"href="(?:\.\./)*(?:/)?{}/([^/"?#]+)/"#,
        regex::escape(sport_slug)
    )) else {
        return Vec::new();
    };

    let skip = [
        "",
        "results",
        "standings",
        "fixtures",
        "outrights",
        "draw",
        "archive",
        "news",
        "h2h",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut games = Vec::new();
    for cap in link_re.captures_iter(html) {
        let child_slug = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if skip.contains(&child_slug) || !seen.insert(child_slug.to_string()) {
            continue;
        }

        let group_slug = path_key(&[sport_slug, child_slug]);
        games.push(GameSection {
            game_name: if sport_slug == "esports" {
                titleize_game(child_slug)
            } else {
                titleize(child_slug)
            },
            game_slug: child_slug.to_string(),
            group_slug: group_slug.clone(),
            oddsportal_url: format!("https://www.oddsportal.com/{}/{}/", sport_slug, child_slug),
            tournament_count: 0,
            match_count: match_cache
                .sections
                .iter()
                .filter(|(key, _)| key.starts_with(&group_slug))
                .map(|(_, entry)| entry.matches.len())
                .sum(),
            last_loaded_at: match_cache
                .sections
                .iter()
                .filter(|(key, _)| key.starts_with(&group_slug))
                .filter_map(|(_, entry)| Some(entry.last_loaded_at.clone()))
                .max(),
            tournaments: Vec::new(),
        });
    }

    games
}

pub fn parse_esports_sections(html: &str, match_cache: &MatchCache) -> Vec<CatalogSection> {
    let Ok(link_re) = regex::Regex::new(r#"href="/esports/([^/"?#]+)/""#) else {
        return Vec::new();
    };

    let mut sections = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cap in link_re.captures_iter(html) {
        let slug = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if matches!(slug, "" | "results" | "standings") || !seen.insert(slug.to_string()) {
            continue;
        }

        let cache_entry = match_cache.sections.get(slug);
        sections.push(CatalogSection {
            game_name: titleize_game(slug),
            game_slug: slug.to_string(),
            section_name: titleize_game(slug),
            section_slug: slug.to_string(),
            oddsportal_url: format!("https://www.oddsportal.com/esports/{}/", slug),
            polymarket_url: format!("https://polymarket.com/esports/{}/games", slug),
            match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
            last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
        });
    }

    sections
}

pub fn parse_game_tournaments(
    html: &str,
    game_slug: &str,
    game_name: &str,
    match_cache: &MatchCache,
) -> Vec<CatalogSection> {
    let Ok(link_re) = regex::Regex::new(&format!(
        r#"href="/esports/{}/([^/"?#]+)/""#,
        regex::escape(game_slug)
    )) else {
        return Vec::new();
    };

    let mut sections = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for cap in link_re.captures_iter(html) {
        if let Some(full_slug) = cap.get(0) {
            let full_slug_str = full_slug.as_str();
            if full_slug_str.contains("results") || full_slug_str.contains("standings") {
                continue;
            }
        }

        let raw_slug = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let slug = canonical_tournament_slug(&["esports", game_slug], raw_slug, "");
        if slug.is_empty() || !seen.insert(slug.clone()) {
            continue;
        }

        let section_slug = format!("esports-{}-{}", game_slug, slug);
        let cache_entry = match_cache.sections.get(&section_slug);
        sections.push(CatalogSection {
            game_name: game_name.to_string(),
            game_slug: game_slug.to_string(),
            section_name: titleize(&slug),
            section_slug,
            oddsportal_url: format!(
                "https://www.oddsportal.com/esports/{}/{}/",
                game_slug,
                oddsportal_esports_tournament_slug(game_slug, &slug)
            ),
            polymarket_url: format!("https://polymarket.com/esports/{}/{}", game_slug, slug),
            match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
            last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
        });
    }

    sections
}

pub fn parse_group_tournaments(
    html: &str,
    group_segments: &[&str],
    group_name: &str,
    match_cache: &MatchCache,
) -> Vec<TournamentSection> {
    if group_segments.is_empty() {
        return Vec::new();
    }

    let prefix = group_segments.join("/");
    let Ok(link_re) = regex::Regex::new(&format!(
        r#"href="/{}/([^/"?#]+)/"[^>]*>([^<]*)</a>"#,
        regex::escape(&prefix)
    )) else {
        return Vec::new();
    };

    let skip = [
        "",
        "results",
        "standings",
        "fixtures",
        "outrights",
        "draw",
        "archive",
        "news",
        "h2h",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut tournaments = Vec::new();
    for cap in link_re.captures_iter(html) {
        let raw_tournament_slug = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let link_text = cap
            .get(2)
            .map(|m| decode_web_text(m.as_str()))
            .unwrap_or_default();
        let tournament_slug =
            canonical_tournament_slug(group_segments, raw_tournament_slug, &link_text);
        if skip.contains(&tournament_slug.as_str()) || !seen.insert(tournament_slug.clone()) {
            continue;
        }

        let mut section_segments = group_segments.to_vec();
        section_segments.push(&tournament_slug);
        let section_slug = path_key(&section_segments);
        let cache_entry = match_cache.sections.get(&section_slug);

        let oddsportal_tournament_slug = if group_segments.first() == Some(&"esports") {
            group_segments
                .get(1)
                .map(|game_slug| oddsportal_esports_tournament_slug(game_slug, &tournament_slug))
                .unwrap_or_else(|| tournament_slug.clone())
        } else {
            tournament_slug.clone()
        };

        tournaments.push(TournamentSection {
            section_name: if !link_text.is_empty() {
                link_text
            } else if group_segments.first() == Some(&"esports") {
                titleize(&tournament_slug)
            } else {
                titleize(&tournament_slug)
            },
            section_slug,
            oddsportal_url: format!(
                "https://www.oddsportal.com/{}/{}/",
                prefix, oddsportal_tournament_slug
            ),
            polymarket_url: polymarket_url_for_path(&section_segments),
            match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
            last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
        });
    }

    if tournaments.is_empty() && group_segments.len() >= 3 {
        let section_slug = path_key(group_segments);
        let cache_entry = match_cache.sections.get(&section_slug);
        tournaments.push(TournamentSection {
            section_name: group_name.to_string(),
            section_slug,
            oddsportal_url: format!("https://www.oddsportal.com/{}/", prefix),
            polymarket_url: polymarket_url_for_path(group_segments),
            match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
            last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
        });
    }

    tournaments.sort_by(|a, b| a.section_name.cmp(&b.section_name));
    tournaments
}

async fn load_or_refresh_group_tournaments(
    config: &AppConfig,
    group_slug: &str,
    refresh: bool,
) -> TournamentsResponse {
    let mut tournament_cache = read_tournament_cache(config).await;
    let match_cache = read_match_cache(config).await;
    let group_segments = path_segments_from_key(group_slug);
    let oddsportal_url = format!("https://www.oddsportal.com/{}/", group_segments.join("/"));
    let group_name = group_segments
        .last()
        .map(|segment| {
            if group_segments.first() == Some(&"esports") {
                titleize_game(segment)
            } else {
                titleize(segment)
            }
        })
        .unwrap_or_else(|| titleize(group_slug));

    if !refresh {
        if let Some(entry) = tournament_cache.groups.get(group_slug) {
            let tournaments = refresh_tournament_counts(entry.tournaments.clone(), &match_cache);
            return TournamentsResponse {
                group_name,
                group_slug: group_slug.to_string(),
                oddsportal_url,
                last_loaded_at: Some(entry.last_loaded_at.clone()),
                tournaments,
            };
        }
    }

    let client = match build_http_client(config.proxy_enabled, &config.proxy) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!("failed to build HTTP client for tournaments: {}", error);
            return TournamentsResponse {
                group_name,
                group_slug: group_slug.to_string(),
                oddsportal_url,
                last_loaded_at: None,
                tournaments: Vec::new(),
            };
        }
    };

    let html = match client
        .get(&oddsportal_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
    {
        Ok(response) => response.text().await.unwrap_or_default(),
        Err(error) => {
            tracing::warn!("failed to load tournaments {}: {}", oddsportal_url, error);
            String::new()
        }
    };

    let mut tournaments =
        parse_group_tournaments(&html, &group_segments, &group_name, &match_cache);
    if tournaments.is_empty() {
        tournaments = configured_tournaments_for_group(config, group_slug, &match_cache);
    }
    tournaments = refresh_tournament_counts(tournaments, &match_cache);

    let last_loaded_at = Utc::now().to_rfc3339();
    tournament_cache.groups.insert(
        group_slug.to_string(),
        TournamentCacheEntry {
            last_loaded_at: last_loaded_at.clone(),
            tournaments: tournaments.clone(),
        },
    );
    if let Err(error) = write_tournament_cache(config, &tournament_cache).await {
        tracing::warn!("failed to write tournament cache: {}", error);
    }

    TournamentsResponse {
        group_name,
        group_slug: group_slug.to_string(),
        oddsportal_url,
        last_loaded_at: Some(last_loaded_at),
        tournaments,
    }
}

fn configured_tournaments_for_group(
    config: &AppConfig,
    group_slug: &str,
    match_cache: &MatchCache,
) -> Vec<TournamentSection> {
    let group_segments = path_segments_from_key(group_slug);
    let mut tournaments = Vec::new();
    for sport_config in &config.scrape_sports.sports {
        let Ok(url) = url::Url::parse(&sport_config.oddsportal_url) else {
            continue;
        };
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();
        if segments.len() < group_segments.len() + 1 {
            continue;
        }
        if segments[..group_segments.len()] != group_segments[..] {
            continue;
        }

        let section_slug = path_key(&segments);
        let cache_entry = match_cache.sections.get(&section_slug);
        tournaments.push(TournamentSection {
            section_name: titleize(segments.last().copied().unwrap_or("matches")),
            section_slug,
            oddsportal_url: sport_config.oddsportal_url.clone(),
            polymarket_url: sport_config.polymarket_url.clone(),
            match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
            last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
        });
    }

    tournaments.sort_by(|a, b| a.section_name.cmp(&b.section_name));
    tournaments
}

async fn load_or_refresh_section(
    config: &AppConfig,
    section_slug: &str,
    refresh: bool,
) -> SectionResponse {
    let mut cache = read_match_cache(config).await;

    if !refresh {
        if let Some(entry) = cache.sections.get(section_slug) {
            // 使用 display_hierarchy_for_config 来正确获取 section_name，或者回退到 titleize
            let section_name = {
                // 尝试找到对应的 sport_config 来获取正确的名称
                if let Some(sport_config) = config
                    .scrape_sports
                    .sports
                    .iter()
                    .find(|config| cache_key_for_config(config) == section_slug)
                {
                    let (_, _, tournament) = display_hierarchy_for_config(sport_config, "");
                    tournament.unwrap_or_else(|| titleize(section_slug))
                } else if section_slug.contains("__") {
                    path_segments_from_key(section_slug)
                        .last()
                        .map(|segment| titleize(segment))
                        .unwrap_or_else(|| titleize(section_slug))
                } else {
                    titleize(section_slug)
                }
            };

            return SectionResponse {
                section_name,
                section_slug: section_slug.to_string(),
                last_loaded_at: Some(entry.last_loaded_at.clone()),
                matches: entry.matches.clone(),
            };
        }
    }

    let sport_config = config
        .scrape_sports
        .sports
        .iter()
        .find(|config| cache_key_for_config(config) == section_slug)
        .cloned()
        .unwrap_or_else(|| {
            let (oddsportal_url, game_slug) = reconstruct_section_url(section_slug, config)
                .unwrap_or_else(|| {
                    (
                        format!("https://www.oddsportal.com/esports/{}/", section_slug),
                        section_slug.to_string(),
                    )
                });

            SportConfig {
                name: section_slug.to_string(),
                oddsportal_url,
                polymarket_url: if section_slug.contains("__") {
                    let segments = path_segments_from_key(section_slug);
                    if segments.first() == Some(&"esports") {
                        format!("https://polymarket.com/esports/{}/games", game_slug)
                    } else {
                        polymarket_url_for_path(&segments)
                    }
                } else {
                    format!("https://polymarket.com/esports/{}/games", game_slug)
                },
            }
        });

    let scraped = sports_scraper::scrape_all_sports(
        std::slice::from_ref(&sport_config),
        config.proxy_enabled,
        &config.proxy,
    )
    .await
    .unwrap_or_default();

    let mut matches = scraped
        .into_iter()
        .next()
        .map(|(_, matches)| {
            matches
                .into_iter()
                .map(|m| MatchInfo {
                    team1: m.team1,
                    team2: m.team2,
                    match_time: m.match_time,
                    end_time: m.end_time,
                    status: m.status,
                    is_finished: m.is_finished,
                    score: m.score,
                    partial_score: m.partial_score,
                    polymarket_url: m.polymarket_url,
                    oddsportal_url: m.oddsportal_url,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if sport_config.oddsportal_url.contains("www.oddsportal.com") {
        let is_esports_section = url::Url::parse(&sport_config.oddsportal_url)
            .ok()
            .and_then(|url| {
                url.path_segments()
                    .and_then(|mut segments| segments.next().map(str::to_string))
            })
            .as_deref()
            == Some("esports");

        if !is_esports_section || matches.iter().any(|m| m.oddsportal_url.is_some()) {
            matches.retain(|m| m.oddsportal_url.is_some());
        }
    }

    let (_, _, section_name) = display_hierarchy_for_config(&sport_config, "");
    let section_name = section_name.unwrap_or_else(|| titleize(section_slug));
    let last_loaded_at = Utc::now().to_rfc3339();
    cache.sections.insert(
        section_slug.to_string(),
        MatchCacheEntry {
            last_loaded_at: last_loaded_at.clone(),
            matches: matches.clone(),
        },
    );
    if let Err(error) = write_match_cache(config, &cache).await {
        tracing::warn!("failed to write web match cache: {}", error);
    }

    SectionResponse {
        section_name,
        section_slug: section_slug.to_string(),
        last_loaded_at: Some(last_loaded_at),
        matches,
    }
}

fn reconstruct_section_url(section_slug: &str, config: &AppConfig) -> Option<(String, String)> {
    if section_slug.contains("__") {
        let segments = path_segments_from_key(section_slug);
        if segments.is_empty() {
            return None;
        }
        let game_slug = if segments.first() == Some(&"esports") {
            segments.get(1).copied().unwrap_or("esports").to_string()
        } else {
            segments.first().copied().unwrap_or("sports").to_string()
        };
        let mut url_segments = segments
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>();
        if segments.len() >= 3 {
            let group_segments = segments[..segments.len() - 1].to_vec();
            let tournament_slug = segments.last().copied().unwrap_or_default();
            let canonical_slug = canonical_tournament_slug(&group_segments, tournament_slug, "");
            if canonical_slug != tournament_slug {
                url_segments.pop();
                url_segments.push(canonical_slug);
            }
            if segments.first() == Some(&"esports")
                && let Some(game_slug) = segments.get(1)
                && let Some(last) = url_segments.last_mut()
            {
                *last = oddsportal_esports_tournament_slug(game_slug, last);
            }
        }
        return Some((
            format!("https://www.oddsportal.com/{}/", url_segments.join("/")),
            game_slug,
        ));
    }

    if !section_slug.starts_with("esports-") {
        return Some((
            format!("https://www.oddsportal.com/{}/", section_slug),
            section_slug.to_string(),
        ));
    }

    let remainder = section_slug.strip_prefix("esports-")?;
    let mut game_slugs = config
        .scrape_sports
        .sports
        .iter()
        .filter_map(|sport_config| {
            url::Url::parse(&sport_config.oddsportal_url)
                .ok()
                .and_then(|url| {
                    let segments: Vec<&str> = url
                        .path_segments()
                        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
                        .unwrap_or_default();
                    if segments.first() == Some(&"esports") {
                        segments.get(1).map(|segment| segment.to_string())
                    } else {
                        None
                    }
                })
        })
        .collect::<Vec<_>>();
    game_slugs.extend([
        "dota-2".to_string(),
        "counter-strike".to_string(),
        "league-of-legends".to_string(),
    ]);
    game_slugs.sort();
    game_slugs.dedup();
    game_slugs.sort_by_key(|slug| std::cmp::Reverse(slug.len()));

    for game_slug in game_slugs {
        if remainder == game_slug {
            return Some((
                format!("https://www.oddsportal.com/esports/{}/", game_slug),
                game_slug,
            ));
        }

        let prefix = format!("{}-", game_slug);
        if let Some(tournament_slug) = remainder.strip_prefix(&prefix) {
            let oddsportal_tournament_slug =
                oddsportal_esports_tournament_slug(&game_slug, tournament_slug);
            return Some((
                format!(
                    "https://www.oddsportal.com/esports/{}/{}/",
                    game_slug, oddsportal_tournament_slug
                ),
                game_slug,
            ));
        }
    }

    None
}

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sports Matches - Polymarket Analysis</title>
    <style>
        *, *::before, *::after {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background-color: #f3f4f6;
            color: #1f2937;
            line-height: 1.6;
            min-height: 100vh;
        }
        header {
            background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%);
            color: white;
            padding: 1.5rem 2rem;
            box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);
        }
        header h1 {
            font-size: 1.875rem;
            font-weight: 700;
        }
        header p {
            opacity: 0.85;
            margin-top: 0.25rem;
        }
        .header-row {
            display: flex;
            align-items: flex-start;
            justify-content: space-between;
            gap: 1rem;
        }
        .top-nav {
            display: flex;
            gap: 0.75rem;
            margin-top: 1rem;
        }
        .top-nav a {
            color: white;
            text-decoration: none;
            border: 1px solid rgba(255,255,255,0.45);
            border-radius: 6px;
            padding: 0.375rem 0.75rem;
            font-size: 0.875rem;
            font-weight: 600;
        }
        .top-nav a:hover {
            background: rgba(255,255,255,0.12);
        }
        .debug-toggle {
            border: 1px solid rgba(255,255,255,0.55);
            border-radius: 6px;
            background: rgba(255,255,255,0.12);
            color: white;
            cursor: pointer;
            font-size: 0.75rem;
            font-weight: 800;
            padding: 0.375rem 0.625rem;
            white-space: nowrap;
        }
        .debug-toggle.active {
            background: #facc15;
            border-color: #facc15;
            color: #713f12;
        }
        main {
            max-width: 1280px;
            margin: 0 auto;
            padding: 2rem 1rem;
        }
        #loading {
            text-align: center;
            padding: 4rem 0;
            color: #6b7280;
            font-size: 1.125rem;
        }
        .breadcrumb {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            margin-bottom: 1rem;
            color: #64748b;
            font-size: 0.875rem;
        }
        .breadcrumb button {
            background: transparent;
            color: #2563eb;
            cursor: pointer;
            font: inherit;
        }
        .category-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
            gap: 1rem;
        }
        .category-button {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            width: 100%;
            min-height: 72px;
            background: white;
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            padding: 1rem 1.25rem;
            color: #1f2937;
            cursor: pointer;
            text-align: left;
            box-shadow: 0 1px 2px rgba(0,0,0,0.06);
        }
        .category-button:hover {
            border-color: #93c5fd;
            background: #f8fafc;
        }
        .category-name {
            font-size: 1rem;
            font-weight: 700;
        }
        .category-count {
            color: #64748b;
            font-size: 0.8rem;
            font-weight: 600;
            white-space: nowrap;
        }
        .sport-card {
            background: white;
            border-radius: 12px;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1), 0 1px 2px rgba(0,0,0,0.06);
            margin-bottom: 1.5rem;
            overflow: hidden;
        }
        .sport-card-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            background: #334155;
            color: white;
            padding: 1rem 1.5rem;
            font-size: 1.25rem;
            font-weight: 600;
        }
        .refresh-button {
            background: #dbeafe;
            color: #1d4ed8;
            border-radius: 6px;
            cursor: pointer;
            font-size: 0.8rem;
            font-weight: 700;
            padding: 0.45rem 0.75rem;
        }
        .refresh-button:hover {
            background: #bfdbfe;
        }
        .section-meta {
            background: #f8fafc;
            border-bottom: 1px solid #e5e7eb;
            color: #64748b;
            font-size: 0.85rem;
            padding: 0.75rem 1.5rem;
        }
        .section-block {
            border-top: 1px solid #e5e7eb;
        }
        .section-block:first-of-type {
            border-top: none;
        }
        .section-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            background: #eef2ff;
            color: #3730a3;
            padding: 0.75rem 1.5rem;
            font-size: 0.95rem;
            font-weight: 700;
        }
        .section-count {
            color: #64748b;
            font-size: 0.75rem;
            font-weight: 600;
            white-space: nowrap;
        }
        .match-table {
            width: 100%;
            border-collapse: collapse;
        }
        .match-table thead th {
            background: #f9fafb;
            padding: 0.75rem 1rem;
            text-align: left;
            font-weight: 500;
            font-size: 0.75rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
            color: #6b7280;
            border-bottom: 1px solid #e5e7eb;
        }
        .match-table tbody td {
            padding: 0.875rem 1rem;
            border-bottom: 1px solid #f3f4f6;
            font-size: 0.875rem;
        }
        .match-table tbody tr:hover {
            background: #f9fafb;
        }
        .match-table tbody tr:last-child td {
            border-bottom: none;
        }
        .match-teams {
            font-weight: 600;
            color: #374151;
        }
        .match-teams .vs {
            color: #9ca3af;
            margin: 0 0.375rem;
            font-weight: 400;
        }
        .match-time {
            color: #6b7280;
            white-space: nowrap;
        }
        .match-status {
            display: inline-flex;
            align-items: center;
            width: max-content;
            padding: 0.25rem 0.5rem;
            border-radius: 999px;
            background: #fee2e2;
            color: #991b1b;
            font-size: 0.75rem;
            font-weight: 700;
            white-space: nowrap;
        }
        .match-status.pending {
            background: #e0f2fe;
            color: #075985;
        }
        .match-score {
            font-weight: 700;
            color: #111827;
            white-space: nowrap;
        }
        .match-score-detail {
            margin-top: 0.25rem;
            color: #6b7280;
            font-size: 0.75rem;
            white-space: nowrap;
        }
        .match-links {
            display: flex;
            gap: 0.5rem;
        }
        .link-btn {
            display: inline-flex;
            align-items: center;
            padding: 0.375rem 0.75rem;
            border-radius: 6px;
            font-size: 0.75rem;
            font-weight: 500;
            text-decoration: none;
            transition: background-color 0.15s;
        }
        .link-btn.pm {
            background: #ede9fe;
            color: #7c3aed;
        }
        .link-btn.pm:hover {
            background: #ddd6fe;
        }
        .link-btn.op {
            background: #dbeafe;
            color: #2563eb;
        }
        .link-btn.op:hover {
            background: #bfdbfe;
        }
        .no-data {
            text-align: center;
            padding: 4rem 0;
            color: #6b7280;
        }
        .error-msg {
            background: #fef2f2;
            border: 1px solid #fecaca;
            border-radius: 8px;
            padding: 1rem 1.5rem;
            color: #dc2626;
            margin-bottom: 1.5rem;
        }
        .scheduler-panel {
            background: #ffffff;
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            margin-bottom: 1.5rem;
            overflow: hidden;
            box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
        }
        .scheduler-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            padding: 0.875rem 1rem;
            background: #f8fafc;
            border-bottom: 1px solid #e5e7eb;
            font-weight: 700;
            color: #111827;
        }
        .scheduler-list {
            display: grid;
        }
        .scheduler-item {
            display: grid;
            grid-template-columns: 1fr auto;
            gap: 1rem;
            align-items: center;
            padding: 0.75rem 1rem;
            border-bottom: 1px solid #f3f4f6;
        }
        .scheduler-item:last-child {
            border-bottom: none;
        }
        .scheduler-teams {
            font-weight: 600;
            color: #374151;
        }
        .scheduler-meta {
            margin-top: 0.25rem;
            color: #6b7280;
            font-size: 0.75rem;
        }
        .pagination-row {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 0.75rem;
            padding: 0.75rem 1rem;
            border-top: 1px solid #e5e7eb;
            color: #6b7280;
            font-size: 0.875rem;
        }
        .pagination-controls {
            display: inline-flex;
            align-items: center;
            gap: 0.5rem;
        }
        .page-button {
            border: 1px solid #d1d5db;
            border-radius: 6px;
            background: #fff;
            color: #374151;
            cursor: pointer;
            font-size: 0.8125rem;
            font-weight: 700;
            padding: 0.375rem 0.625rem;
        }
        .page-button:hover:not(:disabled) {
            background: #f9fafb;
            border-color: #9ca3af;
        }
        .page-button:disabled {
            cursor: default;
            opacity: 0.5;
        }
        .icon-button {
            display: inline-flex;
            align-items: center;
            justify-content: center;
            width: 2rem;
            height: 2rem;
            border: 1px solid #d1d5db;
            border-radius: 6px;
            background: #fff;
            color: #374151;
            cursor: pointer;
            font-weight: 700;
            transition: background-color 0.15s, border-color 0.15s;
        }
        .icon-button:hover:not(:disabled) {
            background: #f9fafb;
            border-color: #9ca3af;
        }
        .icon-button:disabled {
            cursor: default;
            opacity: 0.55;
        }
        .icon-button.play {
            color: #047857;
        }
        .icon-button.remove {
            color: #b91c1c;
        }
        @media (max-width: 768px) {
            header {
                padding: 1rem;
            }
            header h1 {
                font-size: 1.5rem;
            }
            main {
                padding: 1rem 0.75rem;
            }
            .match-table thead th,
            .match-table tbody td {
                padding: 0.625rem 0.75rem;
            }
            .pagination-row {
                align-items: flex-start;
                flex-direction: column;
            }
            .match-table thead th:nth-child(6),
            .match-table tbody td:nth-child(6) {
                display: none;
            }
        }
    </style>
</head>
<body>
    <header>
        <div class="header-row">
            <div>
                <h1>Polymarket Analysis</h1>
                <p>Sports Matches Dashboard</p>
                <nav class="top-nav">
                    <a href="/">Schedule</a>
                    <a href="/analysis">Analysis</a>
                </nav>
            </div>
            <button type="button" class="debug-toggle" id="debug-toggle">Debug: Off</button>
        </div>
    </header>
    <main>
        <div id="loading">Loading matches...</div>
    </main>
    <script>
        let allMatches = [];
        let scheduledMatches = [];
        let schedulerPollingStarted = false;
        let debugMode = localStorage.getItem('schedulerDebugMode') === 'true';
        const matchPageSize = 10;
        const matchPageBySection = new Map();

        async function loadMatches(refresh) {
            const main = document.querySelector('main');
            try {
                bindDebugToggle();
                await loadScheduler();
                startSchedulerPolling();
                const res = await fetch('/api/catalog' + (refresh ? '?refresh=true' : ''));
                if (!res.ok) throw new Error('Failed to fetch matches');
                const data = await res.json();
                allMatches = data || [];

                if (allMatches.length === 0) {
                    main.innerHTML = '<div class="no-data">No matches found.</div>';
                    return;
                }

                renderRoot();
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading matches: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function bindDebugToggle() {
            const button = document.getElementById('debug-toggle');
            if (!button || button.dataset.bound === 'true') return;
            button.dataset.bound = 'true';
            updateDebugToggle();
            button.addEventListener('click', () => {
                debugMode = !debugMode;
                localStorage.setItem('schedulerDebugMode', String(debugMode));
                updateDebugToggle();
                refreshDebugPlayButtons();
            });
        }

        function updateDebugToggle() {
            const button = document.getElementById('debug-toggle');
            if (!button) return;
            button.textContent = debugMode ? 'Debug: On' : 'Debug: Off';
            button.classList.toggle('active', debugMode);
            button.title = debugMode ? 'Debug mode allows testing play buttons on existing or finished matches.' : 'Enable debug mode to test play buttons on existing matches.';
        }

        function refreshDebugPlayButtons() {
            document.querySelectorAll('[data-schedule-match-index]').forEach((button) => {
                const scheduled = button.dataset.scheduled === 'true';
                const finished = button.dataset.finished === 'true';
                button.disabled = debugMode ? false : (scheduled || finished);
                button.title = debugMode ? 'Debug add to scheduler' : 'Add to scheduler';
            });
        }

        function renderRoot() {
            const main = document.querySelector('main');
            let html = renderSchedulerPanel();
            html += '<div class="sport-card">';
            html += '<div class="sport-card-header">';
            html += '<span>Sports</span>';
            html += '<button type="button" class="refresh-button" id="refresh-catalog">Refresh sports</button>';
            html += '</div>';
            html += '<div class="section-meta">First layer is loaded from OddsPortal when refreshed; cached match counts come from local files.</div>';
            html += '</div>';
            html += '<div class="category-grid">';
            allMatches.forEach((sport, index) => {
                html += '<button type="button" class="category-button" data-sport-index="' + index + '">';
                html += '<span class="category-name">' + escapeHtml(sport.sport_name) + '</span>';
                html += '<span class="category-count">' + sport.cached_match_count + ' cached</span>';
                html += '</button>';
            });
            html += '</div>';
            main.innerHTML = html;
            bindSchedulerControls(main);
            document.getElementById('refresh-catalog').addEventListener('click', () => loadMatches(true));
            main.querySelectorAll('[data-sport-index]').forEach((button) => {
                button.addEventListener('click', () => renderSport(Number(button.dataset.sportIndex)));
            });
        }

        async function loadScheduler() {
            try {
                const res = await fetch('/api/scheduler');
                if (!res.ok) {
                    scheduledMatches = [];
                    return;
                }
                const payload = await res.json();
                scheduledMatches = payload.matches || [];
            } catch (_err) {
                scheduledMatches = [];
            }
        }

        function startSchedulerPolling() {
            if (schedulerPollingStarted) return;
            schedulerPollingStarted = true;
            setInterval(async () => {
                await loadScheduler();
                const panel = document.getElementById('scheduler-panel-container');
                if (panel) {
                    panel.outerHTML = renderSchedulerPanel();
                    bindSchedulerControls(document);
                }
            }, 15000);
        }

        function renderSchedulerPanel() {
            let html = '<div class="scheduler-panel" id="scheduler-panel-container">';
            html += '<div class="scheduler-header"><span>Scheduler</span><span class="section-count">' + scheduledMatches.length + ' scheduled</span></div>';
            if (scheduledMatches.length === 0) {
                html += '<div class="section-meta">No matches scheduled.</div>';
            } else {
                html += '<div class="scheduler-list">';
                scheduledMatches.forEach((item) => {
                    html += '<div class="scheduler-item">';
                    html += '<div>';
                    html += '<div class="scheduler-teams">' + escapeHtml(item.team1) + ' vs ' + escapeHtml(item.team2) + '</div>';
                    html += '<div class="scheduler-meta">' + escapeHtml(item.match_time || 'Unknown time');
                    if (item.end_time) html += ' · End: ' + escapeHtml(item.end_time);
                    if (!item.is_finished) html += ' · Collecting';
                    if (item.status) html += ' · ' + escapeHtml(item.status);
                    if (item.score) html += ' · ' + escapeHtml(item.score);
                    html += '</div>';
                    html += '</div>';
                    html += '<button type="button" class="icon-button remove" title="Remove from scheduler" data-remove-schedule="' + escapeHtml(item.id) + '">−</button>';
                    html += '</div>';
                });
                html += '</div>';
            }
            html += '</div>';
            return html;
        }

        function bindSchedulerControls(root) {
            root.querySelectorAll('[data-remove-schedule]').forEach((button) => {
                button.addEventListener('click', async () => {
                    await removeScheduledMatch(button.dataset.removeSchedule);
                });
            });
        }

        async function removeScheduledMatch(scheduleId) {
            const res = await fetch('/api/scheduler/' + encodeURIComponent(scheduleId), { method: 'DELETE' });
            if (!res.ok) throw new Error('Failed to remove scheduled match');
            const payload = await res.json();
            scheduledMatches = payload.matches || [];
            const panel = document.getElementById('scheduler-panel-container');
            if (panel) {
                panel.outerHTML = renderSchedulerPanel();
                bindSchedulerControls(document);
            }
        }

        async function renderSport(sportIndex, refresh) {
            const sport = allMatches[sportIndex];
            if (!sport) return renderRoot();

            const main = document.querySelector('main');
            let html = renderBreadcrumb([{ label: 'All', action: 'root' }, { label: sport.sport_name }]);
            html += '<div class="sport-card">';
            html += '<div class="sport-card-header"><span>' + escapeHtml(sport.sport_name) + '</span></div>';
            html += '<div class="section-meta">' + (refresh ? 'Refreshing sections...' : 'Loading sections...') + '</div>';
            html += '</div>';
            main.innerHTML = html;
            bindBreadcrumb(main);

            try {
                const url = '/api/sport/' + encodeURIComponent(sport.sport_slug) + '/sections' + (refresh ? '?refresh=true' : '');
                const res = await fetch(url);
                if (!res.ok) throw new Error('Failed to load sections');
                const payload = await res.json();

                let games = [];
                if (payload.games) {
                    games = payload.games;
                    sport.games = games;
                    sport.sections = games.flatMap(g => g.tournaments || []);
                } else if (payload.sections) {
                    sport.sections = payload.sections;
                    payload.sections.forEach((section) => {
                        const gameName = section.game_name || 'Other';
                        const gameSlug = section.game_slug || 'other';
                        const existingGame = games.find(g => g.game_slug === gameSlug);
                        if (existingGame) {
                            existingGame.tournaments.push(section);
                            existingGame.tournament_count++;
                            existingGame.match_count += section.match_count || 0;
                        } else {
                            games.push({
                                game_name: gameName,
                                game_slug: gameSlug,
                                tournament_count: 1,
                                match_count: section.match_count || 0,
                                tournaments: [section]
                            });
                        }
                    });
                    sport.games = games;
                }
                sport.section_last_loaded_at = payload.last_loaded_at;

                html = renderBreadcrumb([{ label: 'All', action: 'root' }, { label: sport.sport_name }]);
                html += '<div class="sport-card">';
                html += '<div class="sport-card-header">';
                html += '<span>' + escapeHtml(sport.sport_name) + '</span>';
                html += '<button type="button" class="refresh-button" id="refresh-sections">Refresh sections</button>';
                html += '</div>';
                html += '<div class="section-meta">Second layer loaded: ' + escapeHtml(formatLoadedAt(payload.last_loaded_at)) + '</div>';
                html += '</div>';
                html += '<div class="category-grid">';
                games.forEach((game) => {
                    html += '<button type="button" class="category-button" data-game-slug="' + escapeHtml(game.game_slug) + '">';
                    html += '<span class="category-name">' + escapeHtml(game.game_name) + '</span>';
                    const tournamentCount = game.tournament_count || ((game.tournaments || []).length);
                    html += '<span class="category-count">' + tournamentCount + ' tournaments, ' + (game.match_count || 0) + ' cached</span>';
                    html += '</button>';
                });
                html += '</div>';
                main.innerHTML = html;
                bindBreadcrumb(main);
                document.getElementById('refresh-sections').addEventListener('click', () => renderSport(sportIndex, true));
                main.querySelectorAll('[data-game-slug]').forEach((button) => {
                    button.addEventListener('click', () => renderGame(sportIndex, button.dataset.gameSlug));
                });
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading sections: ' + escapeHtml(err.message) + '</div>';
            }
        }

        async function renderGame(sportIndex, gameSlug, refresh) {
            const sport = allMatches[sportIndex];
            if (!sport || !sport.games) return renderSport(sportIndex, false);

            sport.selectedGameSlug = gameSlug;
            const game = sport.games.find(g => g.game_slug === gameSlug || g.group_slug === gameSlug);
            if (!game) return renderSport(sportIndex, false);

            const gameName = game.game_name || gameSlug;
            const main = document.querySelector('main');
            let html = renderBreadcrumb([
                { label: 'All', action: 'root' },
                { label: sport.sport_name, action: 'sport', sportIndex },
                { label: gameName }
            ]);
            html += '<div class="sport-card">';
            html += '<div class="sport-card-header"><span>' + escapeHtml(gameName) + '</span></div>';
            html += '<div class="section-meta">' + (refresh ? 'Refreshing tournaments...' : 'Loading tournaments...') + '</div>';
            html += '</div>';
            main.innerHTML = html;
            bindBreadcrumb(main);

            try {
                if (!game.tournaments || game.tournaments.length === 0 || refresh) {
                    const groupSlug = game.group_slug || game.game_slug;
                    const res = await fetch('/api/group/' + encodeURIComponent(groupSlug) + '/tournaments' + (refresh ? '?refresh=true' : ''));
                    if (!res.ok) throw new Error('Failed to load tournaments');
                    const payload = await res.json();
                    game.tournaments = payload.tournaments || [];
                    game.tournament_count = game.tournaments.length;
                    game.last_loaded_at = payload.last_loaded_at;
                }

                const gameSections = game.tournaments || [];
                sport.sections = sport.games.flatMap(g => g.tournaments || []);

                html = renderBreadcrumb([
                    { label: 'All', action: 'root' },
                    { label: sport.sport_name, action: 'sport', sportIndex },
                    { label: gameName }
                ]);
                html += '<div class="sport-card">';
                html += '<div class="sport-card-header">';
                html += '<span>' + escapeHtml(gameName) + ' - Tournaments</span>';
                html += '<button type="button" class="refresh-button" id="refresh-tournaments">Refresh tournaments</button>';
                html += '</div>';
                html += '<div class="section-meta">' + gameSections.length + ' tournaments available. Loaded: ' + escapeHtml(formatLoadedAt(game.last_loaded_at)) + '</div>';
                html += '</div>';
                html += '<div class="category-grid">';
                gameSections.forEach((section, index) => {
                html += '<button type="button" class="category-button" data-section-index="' + index + '" data-game-slug="' + escapeHtml(gameSlug) + '">';
                html += '<span class="category-name">' + escapeHtml(section.section_name) + '</span>';
                html += '<span class="category-count">' + section.match_count + ' cached</span>';
                html += '</button>';
                });
                html += '</div>';
                main.innerHTML = html;
                bindBreadcrumb(main);
                document.getElementById('refresh-tournaments').addEventListener('click', () => renderGame(sportIndex, gameSlug, true));
                main.querySelectorAll('[data-section-index]').forEach((button) => {
                    button.addEventListener('click', () => renderMatches(sportIndex, gameSlug, Number(button.dataset.sectionIndex), false));
                });
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading tournaments: ' + escapeHtml(err.message) + '</div>';
            }
        }

        async function renderMatches(sportIndex, gameSlug, sectionIndex, refresh) {
            const sport = allMatches[sportIndex];
            if (!sport || !sport.games) return renderSport(sportIndex, false);

            const currentGameSlug = gameSlug || sport.selectedGameSlug;
            const game = sport.games.find(g => g.game_slug === currentGameSlug || g.group_slug === currentGameSlug);
            const gameSections = (game && game.tournaments) || [];
            const section = gameSections[sectionIndex];
            if (!section) return renderGame(sportIndex, currentGameSlug);

            const gameName = (game && game.game_name) || gameSlug;
            const main = document.querySelector('main');
            let html = renderBreadcrumb([
                { label: 'All', action: 'root' },
                { label: sport.sport_name, action: 'sport', sportIndex },
                { label: gameName, action: 'game', sportIndex, gameSlug },
                { label: section.section_name }
            ]);
            html += '<div class="sport-card">';
            html += '<div class="sport-card-header"><span>' + escapeHtml(section.section_name) + '</span></div>';
            html += '<div id="section-loading" class="section-meta">' + (refresh ? 'Refreshing matches...' : 'Loading matches...') + '</div>';
            html += '</div>';
            main.innerHTML = html;
            bindBreadcrumb(main);

            try {
                const url = '/api/section/' + encodeURIComponent(section.section_slug) + (refresh ? '?refresh=true' : '');
                const res = await fetch(url);
                if (!res.ok) throw new Error('Failed to load matches');
                const payload = await res.json();

                // 更新 section 对象数据
                section.loaded_matches = payload.matches || [];
                section.match_count = payload.matches.length;
                section.last_loaded_at = payload.last_loaded_at;

                const pageKey = section.section_slug || String(sectionIndex);
                if (refresh || !matchPageBySection.has(pageKey)) {
                    matchPageBySection.set(pageKey, 0);
                }
                const currentPage = matchPageBySection.get(pageKey) || 0;

                html = renderSchedulerPanel();
                html += renderBreadcrumb([
                    { label: 'All', action: 'root' },
                    { label: sport.sport_name, action: 'sport', sportIndex },
                    { label: gameName, action: 'game', sportIndex, gameSlug },
                    { label: payload.section_name || section.section_name }
                ]);
                html += '<div class="sport-card">';
                html += '<div class="sport-card-header">';
                html += '<span>' + escapeHtml(payload.section_name || section.section_name) + ' (' + payload.matches.length + ' matches)</span>';
                html += '<button type="button" class="refresh-button" id="refresh-section">Refresh</button>';
                html += '</div>';
                html += '<div class="section-meta">Last loaded: ' + escapeHtml(formatLoadedAt(payload.last_loaded_at)) + '</div>';
                html += '<div id="match-table-container">';
                html += renderMatchTable(payload.matches, currentPage, pageKey);
                html += '</div>';
                html += '</div>';
                main.innerHTML = html;
                bindMatchTableControls(main, sportIndex, gameSlug, sectionIndex, pageKey);
                bindBreadcrumb(main);
                document.getElementById('refresh-section').addEventListener('click', () => renderMatches(sportIndex, gameSlug, sectionIndex, true));
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading matches: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function renderMatchTable(matches, page, pageKey) {
            const total = matches.length;
            const pageCount = Math.max(1, Math.ceil(total / matchPageSize));
            const safePage = Math.min(Math.max(0, page || 0), pageCount - 1);
            const start = safePage * matchPageSize;
            const pageMatches = matches.slice(start, start + matchPageSize);
            const end = Math.min(total, start + pageMatches.length);
            let html = '<table class="match-table">';
            html += '<thead><tr><th></th><th>Matchup</th><th>Time</th><th>End</th><th>Status</th><th>Score</th><th>Links</th></tr></thead>';
            html += '<tbody>';
            pageMatches.forEach((m, pageIndex) => {
                const index = start + pageIndex;
                const statusLabel = m.is_finished ? 'Finished' : (m.status || '');
                const statusClass = m.is_finished ? 'match-status' : 'match-status pending';
                const scheduled = isMatchScheduled(m);
                const disabled = !debugMode && (scheduled || m.is_finished);
                const buttonTitle = debugMode ? 'Debug add to scheduler' : 'Add to scheduler';
                html += '<tr>';
                html += '<td><button type="button" class="icon-button play" title="' + buttonTitle + '" data-schedule-match-index="' + index + '" data-scheduled="' + scheduled + '" data-finished="' + Boolean(m.is_finished) + '"' + (disabled ? ' disabled' : '') + '>' + (scheduled && !debugMode ? '✓' : '▶') + '</button></td>';
                html += '<td class="match-teams">' + escapeHtml(m.team1) + '<span class="vs">vs</span>' + escapeHtml(m.team2) + '</td>';
                html += '<td class="match-time">' + escapeHtml(m.match_time) + '</td>';
                html += '<td class="match-time">' + escapeHtml(m.end_time || 'Unknown') + '</td>';
                html += '<td>' + (statusLabel ? '<span class="' + statusClass + '">' + escapeHtml(statusLabel) + '</span>' : '') + '</td>';
                html += '<td>';
                if (m.score) {
                    html += '<div class="match-score">' + escapeHtml(m.score) + '</div>';
                }
                if (m.partial_score) {
                    html += '<div class="match-score-detail">' + escapeHtml(m.partial_score) + '</div>';
                }
                html += '</td>';
                html += '<td><div class="match-links">';
                if (m.polymarket_url) {
                    html += '<a href="' + escapeHtml(m.polymarket_url) + '" target="_blank" rel="noopener" class="link-btn pm">Polymarket</a>';
                }
                if (m.oddsportal_url) {
                    html += '<a href="' + escapeHtml(m.oddsportal_url) + '" target="_blank" rel="noopener" class="link-btn op">OddsPortal</a>';
                }
                html += '</div></td>';
                html += '</tr>';
            });
            html += '</tbody></table>';
            html += '<div class="pagination-row">';
            html += '<span>Showing ' + (total === 0 ? 0 : start + 1) + '-' + end + ' of ' + total + ' loaded · Page ' + (safePage + 1) + ' / ' + pageCount + '</span>';
            html += '<div class="pagination-controls">';
            html += '<button type="button" class="page-button" data-match-page="' + (safePage - 1) + '" data-page-key="' + escapeHtml(pageKey) + '"' + (safePage <= 0 ? ' disabled' : '') + '>Prev</button>';
            html += '<button type="button" class="page-button" data-match-page="' + (safePage + 1) + '" data-page-key="' + escapeHtml(pageKey) + '"' + (safePage >= pageCount - 1 ? ' disabled' : '') + '>Next</button>';
            html += '</div></div>';
            return html;
        }

        function bindMatchTableControls(root, sportIndex, gameSlug, sectionIndex, pageKey) {
            bindSchedulerControls(root);
            const sport = allMatches[sportIndex];
            const game = sport && sport.games && sport.games.find(g => g.game_slug === gameSlug || g.group_slug === gameSlug);
            const section = game && game.tournaments && game.tournaments[sectionIndex];
            const matches = (section && section.loaded_matches) || [];
            root.querySelectorAll('[data-schedule-match-index]').forEach((button) => {
                button.addEventListener('click', () => addScheduledMatch(matches[Number(button.dataset.scheduleMatchIndex)], button));
            });
            root.querySelectorAll('[data-match-page]').forEach((button) => {
                button.addEventListener('click', () => {
                    const nextPage = Number(button.dataset.matchPage || 0);
                    matchPageBySection.set(pageKey, nextPage);
                    const container = document.getElementById('match-table-container');
                    if (!container) return;
                    container.innerHTML = renderMatchTable(matches, nextPage, pageKey);
                    bindMatchTableControls(container, sportIndex, gameSlug, sectionIndex, pageKey);
                });
            });
            refreshDebugPlayButtons();
        }

        function isMatchScheduled(match) {
            return scheduledMatches.some((item) => {
                if (match.oddsportal_url && item.oddsportal_url === match.oddsportal_url) return true;
                if (match.polymarket_url && item.polymarket_url === match.polymarket_url) return true;
                return item.team1 === match.team1 && item.team2 === match.team2 && item.match_time === match.match_time;
            });
        }

        async function addScheduledMatch(match, button) {
            if (!match || (match.is_finished && !debugMode)) return;
            button.disabled = true;
            const res = await fetch('/api/scheduler', {
                method: 'POST',
                headers: { 'content-type': 'application/json' },
                body: JSON.stringify({
                    team1: match.team1,
                    team2: match.team2,
                    match_time: match.match_time || '',
                    end_time: match.end_time || null,
                    oddsportal_url: match.oddsportal_url || null,
                    polymarket_url: match.polymarket_url || null,
                    status: match.status || null,
                    is_finished: Boolean(match.is_finished),
                    score: match.score || null,
                    partial_score: match.partial_score || null
                })
            });
            if (!res.ok) {
                button.disabled = false;
                throw new Error('Failed to add scheduled match');
            }
            const payload = await res.json();
            scheduledMatches = payload.matches || [];
            button.dataset.scheduled = 'true';
            button.textContent = debugMode ? '▶' : '✓';
            button.disabled = !debugMode;
            const panel = document.getElementById('scheduler-panel-container');
            if (panel) {
                panel.outerHTML = renderSchedulerPanel();
                bindSchedulerControls(document);
            }
            refreshDebugPlayButtons();
        }

        function renderBreadcrumb(items) {
            let html = '<div class="breadcrumb">';
            items.forEach((item, index) => {
                if (index > 0) html += '<span>/</span>';
                if (item.action) {
                    html += '<button type="button" data-action="' + item.action + '"';
                    if (item.sportIndex !== undefined) html += ' data-sport-index="' + item.sportIndex + '"';
                    if (item.gameSlug !== undefined) html += ' data-game-slug="' + item.gameSlug + '"';
                    html += '>' + escapeHtml(item.label) + '</button>';
                } else {
                    html += '<span>' + escapeHtml(item.label) + '</span>';
                }
            });
            html += '</div>';
            return html;
        }

        function bindBreadcrumb(root) {
            root.querySelectorAll('[data-action="root"]').forEach((button) => {
                button.addEventListener('click', renderRoot);
            });
            root.querySelectorAll('[data-action="sport"]').forEach((button) => {
                button.addEventListener('click', () => renderSport(Number(button.dataset.sportIndex), false));
            });
            root.querySelectorAll('[data-action="game"]').forEach((button) => {
                button.addEventListener('click', () => renderGame(Number(button.dataset.sportIndex), button.dataset.gameSlug));
            });
        }

        function countSportMatches(sport) {
            return sport.cached_match_count || 0;
        }

        function formatLoadedAt(value) {
            if (!value) return 'Never';
            const date = new Date(value);
            if (Number.isNaN(date.getTime())) return value;
            return date.toLocaleString();
        }

        function escapeHtml(str) {
            if (!str) return '';
            const div = document.createElement('div');
            div.appendChild(document.createTextNode(str));
            return div.innerHTML;
        }

        document.addEventListener('DOMContentLoaded', loadMatches);
    </script>
</body>
</html>"#;

const ANALYSIS_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Analysis - Polymarket Analysis</title>
    <style>
        *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #f3f4f6;
            color: #1f2937;
            min-height: 100vh;
        }
        header {
            background: linear-gradient(135deg, #0f766e 0%, #115e59 100%);
            color: white;
            padding: 1.5rem 2rem;
            box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1);
        }
        header h1 { font-size: 1.875rem; font-weight: 700; }
        header p { opacity: 0.85; margin-top: 0.25rem; }
        .header-row {
            display: flex;
            align-items: flex-start;
            justify-content: space-between;
            gap: 1rem;
        }
        .top-nav { display: flex; gap: 0.75rem; margin-top: 1rem; }
        .top-nav a {
            color: white;
            text-decoration: none;
            border: 1px solid rgba(255,255,255,0.45);
            border-radius: 6px;
            padding: 0.375rem 0.75rem;
            font-size: 0.875rem;
            font-weight: 600;
        }
        .top-nav a:hover { background: rgba(255,255,255,0.12); }
        main { max-width: 1280px; margin: 0 auto; padding: 2rem 1rem; }
        .panel {
            background: #fff;
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            overflow: hidden;
            margin-bottom: 1.5rem;
            box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
        }
        .panel-header {
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 1rem;
            padding: 0.875rem 1rem;
            background: #f8fafc;
            border-bottom: 1px solid #e5e7eb;
            font-weight: 700;
        }
        .panel-body { padding: 1rem; }
        .meta { color: #6b7280; font-size: 0.875rem; }
        .summary-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
            gap: 0.75rem;
        }
        .metric {
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            padding: 0.875rem;
            background: #fff;
        }
        .metric-value { font-size: 1.5rem; font-weight: 800; color: #111827; }
        .metric-label { color: #6b7280; font-size: 0.75rem; margin-top: 0.25rem; }
        table { width: 100%; border-collapse: collapse; }
        th {
            background: #f9fafb;
            color: #6b7280;
            font-size: 0.75rem;
            font-weight: 700;
            text-align: left;
            text-transform: uppercase;
            padding: 0.75rem;
            border-bottom: 1px solid #e5e7eb;
        }
        td {
            padding: 0.75rem;
            border-bottom: 1px solid #f3f4f6;
            font-size: 0.875rem;
            vertical-align: top;
        }
        tr:last-child td { border-bottom: none; }
        .teams { font-weight: 700; color: #374151; }
        .subtle { color: #6b7280; font-size: 0.75rem; margin-top: 0.25rem; }
        .link-row { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.375rem; }
        .link-row a {
            text-decoration: none;
            border-radius: 6px;
            padding: 0.25rem 0.5rem;
            background: #e0f2fe;
            color: #0369a1;
            font-size: 0.75rem;
            font-weight: 700;
        }
        .badge {
            display: inline-flex;
            width: max-content;
            padding: 0.25rem 0.5rem;
            border-radius: 999px;
            background: #ecfeff;
            color: #155e75;
            font-size: 0.75rem;
            font-weight: 700;
        }
        .empty { color: #6b7280; padding: 2rem 1rem; text-align: center; }
        .error { color: #b91c1c; background: #fef2f2; border: 1px solid #fecaca; border-radius: 8px; padding: 1rem; }
        .debug-toggle {
            border: 1px solid rgba(255,255,255,0.5);
            border-radius: 6px;
            background: rgba(255,255,255,0.12);
            color: #fff;
            cursor: pointer;
            font-size: 0.875rem;
            font-weight: 700;
            padding: 0.5rem 0.75rem;
            white-space: nowrap;
        }
        .debug-toggle.active {
            background: #fef3c7;
            border-color: #fde68a;
            color: #92400e;
        }
        .debug-chart-wrap {
            width: 100%;
            overflow-x: auto;
        }
        .debug-chart {
            display: block;
            width: 100%;
            min-width: 720px;
            height: 280px;
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            background: #ffffff;
        }
        .debug-legend {
            display: flex;
            flex-wrap: wrap;
            gap: 0.75rem;
            margin-top: 0.75rem;
            color: #4b5563;
            font-size: 0.75rem;
            font-weight: 700;
        }
        .legend-item::before {
            content: "";
            display: inline-block;
            width: 0.75rem;
            height: 0.75rem;
            border-radius: 999px;
            margin-right: 0.375rem;
            vertical-align: -0.1rem;
            background: var(--legend-color);
        }
        .delete-button {
            border: 1px solid #fecaca;
            border-radius: 6px;
            background: #fef2f2;
            color: #b91c1c;
            cursor: pointer;
            font-size: 0.75rem;
            font-weight: 700;
            padding: 0.375rem 0.625rem;
        }
        .delete-button:hover {
            background: #fee2e2;
        }
        .force-button {
            border: 1px solid #bbf7d0;
            border-radius: 6px;
            background: #f0fdf4;
            color: #047857;
            cursor: pointer;
            font-size: 0.75rem;
            font-weight: 800;
            padding: 0.375rem 0.625rem;
            white-space: nowrap;
        }
        .force-button:hover:not(:disabled) {
            background: #dcfce7;
        }
        .force-button:disabled {
            cursor: wait;
            opacity: 0.6;
        }
        .view-button {
            border: 1px solid #bfdbfe;
            border-radius: 6px;
            background: #eff6ff;
            color: #1d4ed8;
            cursor: pointer;
            font-size: 0.75rem;
            font-weight: 700;
            padding: 0.375rem 0.625rem;
        }
        .view-button:hover, .view-button.active {
            background: #dbeafe;
            border-color: #60a5fa;
        }
        .detail-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
            gap: 0.75rem;
        }
        .detail-card {
            border: 1px solid #e5e7eb;
            border-radius: 8px;
            padding: 0.875rem;
            background: #fff;
        }
        .detail-label {
            color: #6b7280;
            font-size: 0.75rem;
            font-weight: 700;
            text-transform: uppercase;
        }
        .detail-value {
            color: #111827;
            font-size: 1rem;
            font-weight: 750;
            margin-top: 0.35rem;
            overflow-wrap: anywhere;
        }
        tr.selected-row td {
            background: #f0f9ff;
        }
        @media (max-width: 768px) {
            header { padding: 1rem; }
            main { padding: 1rem 0.75rem; }
            .header-row { align-items: stretch; flex-direction: column; }
            .debug-toggle { width: max-content; }
            th:nth-child(4), td:nth-child(4),
            th:nth-child(5), td:nth-child(5) { display: none; }
        }
    </style>
</head>
<body>
    <header>
        <div class="header-row">
            <div>
                <h1>Polymarket Analysis</h1>
                <p>Collected Data Analysis</p>
            </div>
            <button type="button" class="debug-toggle" id="analysis-debug-toggle">Debug: Off</button>
        </div>
        <nav class="top-nav">
            <a href="/">Schedule</a>
            <a href="/analysis">Analysis</a>
        </nav>
    </header>
    <main>
        <div id="loading" class="panel"><div class="panel-body meta">Loading analysis...</div></div>
    </main>
    <script>
        let analysisDebugMode = localStorage.getItem('analysisDebugMode') === 'true';
        let selectedAnalysisKey = localStorage.getItem('selectedAnalysisKey') || '';
        let lastAnalysisPayload = null;

        async function loadAnalysis() {
            const main = document.querySelector('main');
            try {
                const res = await fetch('/api/analysis');
                if (!res.ok) throw new Error('Failed to load analysis');
                const payload = await res.json();
                renderAnalysis(payload);
            } catch (err) {
                main.innerHTML = '<div class="error">Error loading analysis: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function renderAnalysis(payload) {
            lastAnalysisPayload = payload;
            const scheduled = payload.scheduled_matches || [];
            const collected = payload.collected_matches || [];
            const snapshots = collected.reduce((sum, item) => sum + Number(item.snapshot_count || 0), 0);
            const failed = collected.reduce((sum, item) => sum + Number(item.failed_snapshot_count || 0), 0);
            let html = '<section class="panel">';
            html += '<div class="panel-header"><span>Offline Summary</span><span class="meta">' + escapeHtml(payload.db_path || '') + '</span></div>';
            html += '<div class="panel-body"><div class="summary-grid">';
            html += metric(collected.length, 'Collected matches');
            html += metric(scheduled.length, 'Scheduled records');
            html += metric(snapshots, 'Snapshots');
            html += metric(failed, 'Failed snapshots');
            html += '</div></div></section>';
            const selected = selectedAnalysisItem(payload);
            html += renderSelectedMatch(selected, payload);
            html += renderOddsChartPanel(selected);
            if (analysisDebugMode) html += renderDebugCharts(payload, selected && selected.collected);
            html += renderScheduled(scheduled, selected);
            html += renderCollected(collected, selected);
            document.querySelector('main').innerHTML = html;
            bindAnalysisControls();
            bindAnalysisDebugToggle();
            loadSelectedOddsSeries();
        }

        function metric(value, label) {
            return '<div class="metric"><div class="metric-value">' + escapeHtml(String(value)) + '</div><div class="metric-label">' + escapeHtml(label) + '</div></div>';
        }

        function renderSelectedMatch(selected, payload) {
            if (!selected) return '<section class="panel"><div class="panel-header"><span>Selected Match</span><span class="meta">None</span></div><div class="empty">Select a match from Scheduler History or Collected Matches.</div></section>';
            const item = selected.scheduled || selected.collected;
            const collected = selected.collected;
            let html = '<section class="panel"><div class="panel-header"><span>Selected Match</span><span class="meta">' + escapeHtml(selected.key) + '</span></div><div class="panel-body">';
            html += '<div class="teams">' + escapeHtml(item.team1) + ' vs ' + escapeHtml(item.team2) + '</div>';
            html += '<div class="subtle">' + escapeHtml(item.match_time || 'Unknown time') + (item.end_time ? ' · End: ' + escapeHtml(item.end_time) : '') + '</div>';
            html += '<div class="detail-grid" style="margin-top:0.875rem">';
            html += detailCard('Status', item.status || (collected && collected.snapshot_count ? 'Collected' : 'No data'));
            html += detailCard('Snapshots', collected ? String(collected.snapshot_count || 0) : '0');
            html += detailCard('Polymarket', collected ? String(collected.polymarket_snapshot_count || 0) : '0');
            html += detailCard('OddsPortal', collected ? String(collected.oddsportal_snapshot_count || 0) : '0');
            html += detailCard('Empty snapshots', collected ? String(collected.empty_snapshot_count || 0) : '0');
            html += detailCard('Last collected', collected ? formatDate(collected.last_collected_at) : '');
            html += detailCard('Latest PM', collected ? ((collected.latest_polymarket_outcome || '') + ' ' + formatNumber(collected.latest_polymarket_price)).trim() : '');
            html += detailCard('Latest OP', collected ? oddsText(collected) : '');
            html += '</div>';
            html += '<div style="margin-top:0.875rem">' + renderLinks(Object.assign({}, collected || {}, item || {})) + '</div>';
            if (!collected || Number(collected.snapshot_count || 0) === 0) {
                html += '<div class="subtle" style="margin-top:0.75rem">No SQLite snapshots are linked to this scheduled match yet.</div>';
            }
            html += '</div></section>';
            return html;
        }

        function detailCard(label, value) {
            return '<div class="detail-card"><div class="detail-label">' + escapeHtml(label) + '</div><div class="detail-value">' + escapeHtml(value || '-') + '</div></div>';
        }

        function renderOddsChartPanel(selected) {
            if (!selected) {
                return '<section class="panel"><div class="panel-header"><span>Odds Chart</span><span class="meta">No match selected</span></div><div class="empty">Select a match to show odds history.</div></section>';
            }
            const item = selected.collected || selected.scheduled || {};
            const matchId = selected.collected ? selected.collected.match_id : teamKey(item.team1 || '', item.team2 || '');
            const hasSeries = Boolean(selected.collected && selected.collected.match_id);
            return '<section class="panel"><div class="panel-header"><span>Odds Chart</span><span class="meta">' + escapeHtml(matchId || selected.key || '') + '</span></div>' +
                '<div class="panel-body" id="odds-series-chart" data-series-match-id="' + escapeHtml(hasSeries ? matchId : '') + '">' +
                (hasSeries ? '<div class="meta">Loading odds chart...</div>' : '<div class="empty">No SQLite snapshots are linked to this scheduled match yet. The chart will appear after the scheduler collects odds.</div>') +
                '</div></section>';
        }

        function renderScheduled(items, selected) {
            let html = '<section class="panel"><div class="panel-header"><span>Scheduler History</span><span class="meta">' + items.length + ' records</span></div>';
            if (items.length === 0) return html + '<div class="empty">No scheduler records.</div></section>';
            html += '<table><thead><tr><th>View</th><th>Match</th><th>Status</th><th>Score</th><th>Added</th><th>Links</th></tr></thead><tbody>';
            items.forEach((item, index) => {
                const key = scheduledKey(item);
                const active = selected && selected.key === key;
                html += '<tr class="' + (active ? 'selected-row' : '') + '"><td><button type="button" class="view-button ' + (active ? 'active' : '') + '" data-select-analysis-key="' + escapeHtml(key) + '">View</button></td>';
                html += '<td><div class="teams">' + escapeHtml(item.team1) + ' vs ' + escapeHtml(item.team2) + '</div><div class="subtle">' + escapeHtml(item.match_time || 'Unknown time') + (item.end_time ? ' · End: ' + escapeHtml(item.end_time) : '') + '</div></td>';
                html += '<td>' + (item.status ? '<span class="badge">' + escapeHtml(item.status) + '</span>' : '');
                if (analysisDebugMode) html += '<div style="margin-top:0.5rem"><button type="button" class="force-button" data-force-scheduled-index="' + index + '">Force monitor</button></div>';
                html += '</td>';
                html += '<td>' + escapeHtml(item.score || '') + '<div class="subtle">' + escapeHtml(item.partial_score || '') + '</div></td>';
                html += '<td><div class="subtle">' + escapeHtml(formatDate(item.added_at)) + '</div></td>';
                html += '<td>' + renderLinks(item) + '</td></tr>';
            });
            html += '</tbody></table></section>';
            return html;
        }

        function renderCollected(items, selected) {
            let html = '<section class="panel"><div class="panel-header"><span>Collected Matches</span><span class="meta">' + items.length + ' matches</span></div>';
            if (items.length === 0) return html + '<div class="empty">No collected match data found.</div></section>';
            html += '<table><thead><tr><th>View</th><th>Match</th><th>Snapshots</th><th>Latest Polymarket</th><th>Latest OddsPortal</th><th>Last Collected</th><th>Links</th><th>Delete</th></tr></thead><tbody>';
            items.forEach((item, index) => {
                const key = collectedKey(item);
                const active = selected && (selected.key === key || selected.collected === item);
                html += '<tr class="' + (active ? 'selected-row' : '') + '"><td><button type="button" class="view-button ' + (active ? 'active' : '') + '" data-select-analysis-key="' + escapeHtml(key) + '">View</button></td>';
                html += '<td><div class="teams">' + escapeHtml(item.team1) + ' vs ' + escapeHtml(item.team2) + '</div><div class="subtle">' + escapeHtml(item.match_id) + '</div></td>';
                html += '<td>' + escapeHtml(String(item.snapshot_count || 0)) + '<div class="subtle">PM ' + escapeHtml(String(item.polymarket_snapshot_count || 0)) + ' / OP ' + escapeHtml(String(item.oddsportal_snapshot_count || 0)) + '</div></td>';
                html += '<td>' + escapeHtml(item.latest_polymarket_outcome || '') + '<div class="subtle">' + formatNumber(item.latest_polymarket_price) + ' · vol ' + formatNumber(item.latest_polymarket_volume) + '</div></td>';
                html += '<td>' + escapeHtml(item.latest_oddsportal_bookmaker || '') + '<div class="subtle">' + oddsText(item) + '</div></td>';
                html += '<td><div class="subtle">' + escapeHtml(formatDate(item.last_collected_at)) + '</div></td>';
                html += '<td>' + renderLinks(item);
                if (analysisDebugMode) html += '<div style="margin-top:0.5rem"><button type="button" class="force-button" data-force-collected-index="' + index + '">Force monitor</button></div>';
                html += '</td>';
                html += '<td><button type="button" class="delete-button" data-delete-match-id="' + escapeHtml(item.match_id) + '" data-delete-label="' + escapeHtml(item.team1 + ' vs ' + item.team2) + '">Delete</button></td></tr>';
            });
            html += '</tbody></table></section>';
            return html;
        }

        function bindAnalysisControls() {
            document.querySelectorAll('[data-select-analysis-key]').forEach((button) => {
                button.addEventListener('click', () => {
                    selectedAnalysisKey = button.dataset.selectAnalysisKey || '';
                    localStorage.setItem('selectedAnalysisKey', selectedAnalysisKey);
                    if (lastAnalysisPayload) renderAnalysis(lastAnalysisPayload);
                });
            });
            document.querySelectorAll('[data-delete-match-id]').forEach((button) => {
                button.addEventListener('click', () => {
                    deleteMatchData(button.dataset.deleteMatchId, button.dataset.deleteLabel);
                });
            });
            document.querySelectorAll('[data-force-scheduled-index]').forEach((button) => {
                button.addEventListener('click', () => {
                    const item = (lastAnalysisPayload && lastAnalysisPayload.scheduled_matches || [])[Number(button.dataset.forceScheduledIndex)];
                    forceMonitorMatch(item, button);
                });
            });
            document.querySelectorAll('[data-force-collected-index]').forEach((button) => {
                button.addEventListener('click', () => {
                    const item = (lastAnalysisPayload && lastAnalysisPayload.collected_matches || [])[Number(button.dataset.forceCollectedIndex)];
                    forceMonitorMatch(item, button);
                });
            });
        }

        async function forceMonitorMatch(item, button) {
            if (!analysisDebugMode || !item) return;
            button.disabled = true;
            const body = {
                team1: item.team1 || '',
                team2: item.team2 || '',
                match_time: item.match_time || '',
                end_time: item.end_time || null,
                oddsportal_url: item.oddsportal_url || null,
                polymarket_url: item.polymarket_url || null,
                status: 'Debug forced',
                is_finished: false,
                score: item.score || null,
                partial_score: item.partial_score || null
            };
            try {
                const res = await fetch('/api/scheduler/force', {
                    method: 'POST',
                    headers: { 'content-type': 'application/json' },
                    body: JSON.stringify(body)
                });
                const payload = await res.json();
                if (!res.ok || payload.error) throw new Error(payload.error || 'Failed to force monitor');
                lastAnalysisPayload.scheduled_matches = payload.matches || [];
                button.textContent = payload.started ? 'Started' : 'Queued';
                renderAnalysis(lastAnalysisPayload);
            } catch (err) {
                button.disabled = false;
                window.alert(err.message);
            }
        }

        async function loadSelectedLatestOdds() {
            const panel = document.getElementById('latest-odds-panel');
            if (!panel) return;
            const matchId = panel.dataset.latestMatchId || '';
            if (!matchId) return;
            try {
                const res = await fetch('/api/analysis/match/' + encodeURIComponent(matchId) + '/latest');
                const payload = await res.json();
                if (!res.ok || !payload.success || !payload.latest) {
                    throw new Error(payload.error || 'Failed to load latest odds');
                }
                panel.innerHTML = renderLatestOdds(payload.latest);
            } catch (err) {
                panel.innerHTML = '<div class="error">Error loading latest odds: ' + escapeHtml(err.message) + '</div>';
            }
        }

        async function loadSelectedOddsSeries() {
            const panel = document.getElementById('odds-series-chart');
            if (!panel) return;
            const matchId = panel.dataset.seriesMatchId || '';
            if (!matchId) return;
            try {
                const res = await fetch('/api/analysis/match/' + encodeURIComponent(matchId) + '/series');
                const payload = await res.json();
                if (!res.ok || !payload.success) {
                    throw new Error(payload.error || 'Failed to load odds series');
                }
                panel.innerHTML = renderOddsSeriesChart(payload.points || []);
            } catch (err) {
                panel.innerHTML = '<div class="error">Error loading odds chart: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function renderOddsSeriesChart(points) {
            const usable = points
                .map((point) => ({
                    time: point.collected_at,
                    label: point.label || point.source || 'Series',
                    value: normalizeProbability(point.value),
                    source: point.source || '',
                }))
                .filter((point) => point.time && point.value !== null);
            if (usable.length === 0) {
                return '<div class="empty">No parsed odds series for this match yet.</div>';
            }

            const labels = Array.from(new Set(usable.map((point) => point.label))).slice(0, 8);
            const times = Array.from(new Set(usable.map((point) => point.time))).sort();
            const latestByLabel = new Map();
            usable.forEach((point) => latestByLabel.set(point.label, point));
            let html = '<div class="debug-chart-wrap">' + renderOddsSeriesSvg(usable, labels, times) + '</div>';
            html += '<div class="debug-legend">';
            labels.forEach((label, index) => {
                const latest = latestByLabel.get(label);
                html += '<span class="legend-item" style="--legend-color:' + seriesColor(index) + '">' + escapeHtml(label) + (latest ? ' ' + escapeHtml(formatNumber(latest.value)) : '') + '</span>';
            });
            html += '</div>';
            html += '<div class="subtle">Y-axis uses probability scale. OddsPortal decimal odds are shown as implied probability so they can be compared with Polymarket prices.</div>';
            return html;
        }

        function renderOddsSeriesSvg(points, labels, times) {
            const width = 920;
            const height = 300;
            const pad = { left: 46, right: 18, top: 18, bottom: 34 };
            const timeIndex = new Map(times.map((time, index) => [time, index]));
            const xForTime = (time) => {
                const index = timeIndex.get(time) || 0;
                const denominator = Math.max(1, times.length - 1);
                return pad.left + (index / denominator) * (width - pad.left - pad.right);
            };
            const yFor = (value) => pad.top + (1 - value) * (height - pad.top - pad.bottom);
            const grid = [0, 0.25, 0.5, 0.75, 1].map((value) => {
                const y = yFor(value);
                return '<line x1="' + pad.left + '" y1="' + y + '" x2="' + (width - pad.right) + '" y2="' + y + '" stroke=\'#e5e7eb\'/><text x="12" y="' + (y + 4) + '" fill=\'#6b7280\' font-size="11">' + Math.round(value * 100) + '%</text>';
            }).join('');
            const lines = labels.map((label, index) => {
                const labelPoints = points
                    .filter((point) => point.label === label)
                    .sort((a, b) => a.time.localeCompare(b.time));
                const path = labelPoints
                    .map((point) => xForTime(point.time).toFixed(1) + ',' + yFor(point.value).toFixed(1))
                    .join(' ');
                const circles = labelPoints
                    .map((point) => '<circle cx="' + xForTime(point.time).toFixed(1) + '" cy="' + yFor(point.value).toFixed(1) + '" r="3" fill="' + seriesColor(index) + '"/>')
                    .join('');
                return '<polyline points="' + path + '" fill="none" stroke="' + seriesColor(index) + '" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>' + circles;
            }).join('');
            const firstLabel = formatDate(times[0]);
            const lastLabel = formatDate(times[times.length - 1]);
            return '<svg class="debug-chart" viewBox="0 0 ' + width + ' ' + height + '" role="img" aria-label="Odds series chart">' +
                grid + lines +
                '<line x1="' + pad.left + '" y1="' + (height - pad.bottom) + '" x2="' + (width - pad.right) + '" y2="' + (height - pad.bottom) + '" stroke=\'#9ca3af\'/>' +
                '<text x="' + pad.left + '" y="' + (height - 10) + '" fill=\'#6b7280\' font-size="11">' + escapeHtml(firstLabel) + '</text>' +
                '<text x="' + (width - pad.right) + '" y="' + (height - 10) + '" fill=\'#6b7280\' font-size="11" text-anchor="end">' + escapeHtml(lastLabel) + '</text>' +
                '</svg>';
        }

        function seriesColor(index) {
            return ['#2563eb', '#dc2626', '#059669', '#9333ea', '#ea580c', '#0891b2', '#be123c', '#4f46e5'][index % 8];
        }

        function renderLatestOdds(latest) {
            const pm = latest.polymarket || [];
            const op = latest.oddsportal || [];
            let html = '<div class="panel-body">';
            html += '<div class="detail-label">Latest Odds</div>';
            if (pm.length === 0 && op.length === 0) {
                return html + '<div class="subtle" style="margin-top:0.5rem">No parsed odds in the latest snapshots.</div></div>';
            }
            if (pm.length > 0) {
                html += '<div class="subtle" style="margin-top:0.5rem">Polymarket · ' + escapeHtml(formatDate(pm[0].collected_at)) + '</div>';
                html += '<table style="margin-top:0.5rem"><thead><tr><th>Market</th><th>Outcome</th><th>Price</th><th>Volume</th><th>Active</th></tr></thead><tbody>';
                pm.forEach((price) => {
                    html += '<tr><td>' + escapeHtml(price.market_title || '') + '</td>';
                    html += '<td>' + escapeHtml(price.outcome || '') + '</td>';
                    html += '<td>' + escapeHtml(formatNumber(price.price)) + '</td>';
                    html += '<td>' + escapeHtml(formatNumber(price.volume)) + '</td>';
                    html += '<td>' + escapeHtml(price.active === null || price.active === undefined ? '' : String(Boolean(price.active))) + '</td></tr>';
                });
                html += '</tbody></table>';
            }
            if (op.length > 0) {
                html += '<div class="subtle" style="margin-top:0.875rem">OddsPortal · ' + escapeHtml(formatDate(op[0].collected_at)) + '</div>';
                html += '<table style="margin-top:0.5rem"><thead><tr><th>Bookmaker</th><th>Home</th><th>Draw</th><th>Away</th></tr></thead><tbody>';
                op.forEach((odds) => {
                    html += '<tr><td>' + escapeHtml(odds.bookmaker || '') + '</td>';
                    html += '<td>' + escapeHtml(formatNumber(odds.home)) + '</td>';
                    html += '<td>' + escapeHtml(formatNumber(odds.draw)) + '</td>';
                    html += '<td>' + escapeHtml(formatNumber(odds.away)) + '</td></tr>';
                });
                html += '</tbody></table>';
            }
            html += '</div>';
            return html;
        }

        function bindAnalysisDebugToggle() {
            const button = document.getElementById('analysis-debug-toggle');
            if (!button || button.dataset.bound === 'true') {
                updateAnalysisDebugToggle();
                return;
            }
            button.dataset.bound = 'true';
            button.addEventListener('click', () => {
                analysisDebugMode = !analysisDebugMode;
                localStorage.setItem('analysisDebugMode', String(analysisDebugMode));
                updateAnalysisDebugToggle();
                if (lastAnalysisPayload) renderAnalysis(lastAnalysisPayload);
            });
            updateAnalysisDebugToggle();
        }

        function updateAnalysisDebugToggle() {
            const button = document.getElementById('analysis-debug-toggle');
            if (!button) return;
            button.textContent = analysisDebugMode ? 'Debug: On' : 'Debug: Off';
            button.classList.toggle('active', analysisDebugMode);
        }

        function renderDebugCharts(payload, selectedCollected) {
            const collected = payload.collected_matches || [];
            const sourcePoints = selectedCollected
                ? (payload.debug_points || []).filter((point) => point.match_id === selectedCollected.match_id)
                : (payload.debug_points || []);
            const debugPoints = buildDebugSeries(sourcePoints, selectedCollected ? [selectedCollected] : collected);
            let html = '<section class="panel"><div class="panel-header"><span>Debug Chart</span><span class="meta">' + escapeHtml(debugPoints.mode) + '</span></div>';
            if (debugPoints.points.length < 2) return html + '<div class="empty">Not enough collected data for chart rendering.</div></section>';
            html += '<div class="panel-body">';
            html += '<div class="debug-chart-wrap">' + renderDebugSvg(debugPoints.points) + '</div>';
            html += '<div class="debug-legend">';
            html += '<span class="legend-item" style="--legend-color:#2563eb">Polymarket price</span>';
            html += '<span class="legend-item" style="--legend-color:#dc2626">OddsPortal implied home</span>';
            html += '<span class="legend-item" style="--legend-color:#059669">Volume index</span>';
            html += '</div>';
            html += '<div class="subtle">Match: ' + escapeHtml(debugPoints.label) + '</div>';
            html += '</div></section>';
            return html;
        }

        function buildDebugSeries(rawPoints, collected) {
            const usable = rawPoints
                .map((point) => {
                    const pm = normalizeProbability(point.polymarket_price);
                    const odds = impliedProbability(point.odds_home);
                    const volume = normalizeVolume(point.polymarket_volume);
                    return {
                        time: point.collected_at,
                        matchId: point.match_id,
                        polymarket: pm,
                        oddsportal: odds,
                        volume,
                    };
                })
                .filter((point) => point.time && (point.polymarket !== null || point.oddsportal !== null || point.volume !== null));

            const priceCount = usable.filter((point) => point.polymarket !== null || point.oddsportal !== null).length;
            if (priceCount >= 2) {
                const label = usable[0].matchId || 'SQLite snapshots';
                return { mode: 'SQLite debug data', label, points: usable.slice(-80) };
            }

            const baseMatch = collected[0] || {
                match_id: 'debug_dota2_blast_slam_vii',
                team1: 'Falcons Dota 2',
                team2: 'Team Yandex Dota 2',
                snapshot_count: 18,
                first_collected_at: new Date(Date.now() - 90 * 60000).toISOString(),
                last_collected_at: new Date().toISOString(),
            };
            const count = Math.max(12, Math.min(36, Number(baseMatch.snapshot_count || 0) + 10));
            const start = parseDate(baseMatch.first_collected_at) || new Date(Date.now() - count * 60000);
            const end = parseDate(baseMatch.last_collected_at) || new Date(start.getTime() + count * 60000);
            const span = Math.max(60000, end.getTime() - start.getTime());
            const points = [];
            for (let i = 0; i < count; i += 1) {
                const ratio = count === 1 ? 0 : i / (count - 1);
                const wave = Math.sin(i * 0.67);
                const drift = (ratio - 0.5) * 0.05;
                points.push({
                    time: new Date(start.getTime() + span * ratio).toISOString(),
                    matchId: baseMatch.match_id,
                    polymarket: clamp(0.49 + wave * 0.045 + drift, 0.05, 0.95),
                    oddsportal: clamp(0.52 - Math.sin(i * 0.51) * 0.035 - drift * 0.7, 0.05, 0.95),
                    volume: clamp(0.18 + ratio * 0.7 + Math.sin(i * 0.37) * 0.08, 0, 1),
                });
            }
            return {
                mode: collected[0] ? 'Simulated from collected history' : 'Demo Dota2 debug data',
                label: baseMatch.team1 + ' vs ' + baseMatch.team2,
                points,
            };
        }

        function renderDebugSvg(points) {
            const width = 920;
            const height = 280;
            const pad = { left: 46, right: 18, top: 18, bottom: 34 };
            const xFor = (index) => pad.left + (index / Math.max(1, points.length - 1)) * (width - pad.left - pad.right);
            const yFor = (value) => pad.top + (1 - value) * (height - pad.top - pad.bottom);
            const grid = [0, 0.25, 0.5, 0.75, 1].map((value) => {
                const y = yFor(value);
                return '<line x1="' + pad.left + '" y1="' + y + '" x2="' + (width - pad.right) + '" y2="' + y + '" stroke=\'#e5e7eb\'/><text x="12" y="' + (y + 4) + '" fill=\'#6b7280\' font-size="11">' + Math.round(value * 100) + '%</text>';
            }).join('');
            const pmPath = polyline(points, 'polymarket', xFor, yFor);
            const oddsPath = polyline(points, 'oddsportal', xFor, yFor);
            const volumePath = polyline(points, 'volume', xFor, yFor);
            const firstLabel = formatDate(points[0].time);
            const lastLabel = formatDate(points[points.length - 1].time);
            return '<svg class="debug-chart" viewBox="0 0 ' + width + ' ' + height + '" role="img" aria-label="Analysis debug chart">' +
                grid +
                '<polyline points="' + pmPath + '" fill="none" stroke=\'#2563eb\' stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>' +
                '<polyline points="' + oddsPath + '" fill="none" stroke=\'#dc2626\' stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>' +
                '<polyline points="' + volumePath + '" fill="none" stroke=\'#059669\' stroke-width="2" stroke-dasharray="5 5" stroke-linecap="round" stroke-linejoin="round"/>' +
                '<line x1="' + pad.left + '" y1="' + (height - pad.bottom) + '" x2="' + (width - pad.right) + '" y2="' + (height - pad.bottom) + '" stroke=\'#9ca3af\'/>' +
                '<text x="' + pad.left + '" y="' + (height - 10) + '" fill=\'#6b7280\' font-size="11">' + escapeHtml(firstLabel) + '</text>' +
                '<text x="' + (width - pad.right) + '" y="' + (height - 10) + '" fill=\'#6b7280\' font-size="11" text-anchor="end">' + escapeHtml(lastLabel) + '</text>' +
                '</svg>';
        }

        function polyline(points, key, xFor, yFor) {
            return points
                .map((point, index) => point[key] === null || point[key] === undefined ? null : xFor(index).toFixed(1) + ',' + yFor(point[key]).toFixed(1))
                .filter(Boolean)
                .join(' ');
        }

        function selectedAnalysisItem(payload) {
            const scheduled = payload.scheduled_matches || [];
            const collected = payload.collected_matches || [];
            const keyed = [];
            scheduled.forEach((item) => keyed.push({
                key: scheduledKey(item),
                scheduled: item,
                collected: findCollectedForScheduled(item, collected),
            }));
            collected.forEach((item) => keyed.push({
                key: collectedKey(item),
                scheduled: findScheduledForCollected(item, scheduled),
                collected: item,
            }));

            let selected = keyed.find((item) => item.key === selectedAnalysisKey);
            if (!selected && scheduled.length === 1) {
                selected = keyed.find((item) => item.scheduled === scheduled[0]);
            }
            if (!selected && collected.length === 1) {
                selected = keyed.find((item) => item.collected === collected[0]);
            }
            if (!selected && keyed.length > 0) {
                selected = keyed[0];
            }
            if (selected) {
                selectedAnalysisKey = selected.key;
                localStorage.setItem('selectedAnalysisKey', selectedAnalysisKey);
            }
            return selected || null;
        }

        function findCollectedForScheduled(scheduled, collected) {
            return collected.find((item) => {
                if (scheduled.oddsportal_url && item.oddsportal_url === scheduled.oddsportal_url) return true;
                if (scheduled.polymarket_url && item.polymarket_url === scheduled.polymarket_url) return true;
                return sameTeams(scheduled.team1, scheduled.team2, item.team1, item.team2);
            }) || null;
        }

        function findScheduledForCollected(collected, scheduled) {
            return scheduled.find((item) => {
                if (item.oddsportal_url && item.oddsportal_url === collected.oddsportal_url) return true;
                if (item.polymarket_url && item.polymarket_url === collected.polymarket_url) return true;
                return sameTeams(item.team1, item.team2, collected.team1, collected.team2);
            }) || null;
        }

        function scheduledKey(item) {
            return 'schedule:' + (item.id || item.oddsportal_url || item.polymarket_url || teamKey(item.team1, item.team2));
        }

        function collectedKey(item) {
            return 'collected:' + (item.match_id || item.oddsportal_url || item.polymarket_url || teamKey(item.team1, item.team2));
        }

        function sameTeams(a1, a2, b1, b2) {
            return (sameTeam(a1, b1) && sameTeam(a2, b2)) || (sameTeam(a1, b2) && sameTeam(a2, b1));
        }

        function sameTeam(a, b) {
            const left = normalizeTeam(a);
            const right = normalizeTeam(b);
            if (!left || !right) return false;
            if (left === right) return true;
            if (left.startsWith('team') && left.slice(4) === right) return true;
            if (right.startsWith('team') && right.slice(4) === left) return true;
            return left.includes(right) || right.includes(left);
        }

        function normalizeTeam(value) {
            return String(value || '').toLowerCase().replace(/[^a-z0-9]/g, '');
        }

        function teamKey(a, b) {
            return normalizeTeam(a) + '_vs_' + normalizeTeam(b);
        }

        async function deleteMatchData(matchId, label) {
            const confirmed = window.confirm('Delete collected data for ' + label + '? This cannot be undone.');
            if (!confirmed) return;
            const res = await fetch('/api/analysis/match/' + encodeURIComponent(matchId), { method: 'DELETE' });
            if (!res.ok) throw new Error('Failed to delete match data');
            const payload = await res.json();
            renderAnalysis(payload.analysis);
        }

        function renderLinks(item) {
            let html = '<div class="link-row">';
            if (item.polymarket_url) html += '<a href="' + escapeHtml(item.polymarket_url) + '" target="_blank" rel="noopener">Polymarket</a>';
            if (item.oddsportal_url) html += '<a href="' + escapeHtml(item.oddsportal_url) + '" target="_blank" rel="noopener">OddsPortal</a>';
            html += '</div>';
            return html;
        }

        function oddsText(item) {
            const parts = [];
            if (item.latest_oddsportal_home !== null && item.latest_oddsportal_home !== undefined) parts.push('H ' + formatNumber(item.latest_oddsportal_home));
            if (item.latest_oddsportal_draw !== null && item.latest_oddsportal_draw !== undefined) parts.push('D ' + formatNumber(item.latest_oddsportal_draw));
            if (item.latest_oddsportal_away !== null && item.latest_oddsportal_away !== undefined) parts.push('A ' + formatNumber(item.latest_oddsportal_away));
            return parts.join(' / ');
        }

        function formatNumber(value) {
            if (value === null || value === undefined || value === '') return '';
            const number = Number(value);
            if (Number.isNaN(number)) return String(value);
            return number.toFixed(3).replace(/0+$/, '').replace(/\.$/, '');
        }

        function normalizeProbability(value) {
            if (value === null || value === undefined || value === '') return null;
            const number = Number(value);
            if (Number.isNaN(number)) return null;
            if (number > 1 && number <= 100) return clamp(number / 100, 0, 1);
            return clamp(number, 0, 1);
        }

        function impliedProbability(value) {
            if (value === null || value === undefined || value === '') return null;
            const number = Number(value);
            if (Number.isNaN(number) || number <= 0) return null;
            return clamp(1 / number, 0, 1);
        }

        function normalizeVolume(value) {
            if (value === null || value === undefined || value === '') return null;
            const number = Number(value);
            if (Number.isNaN(number) || number < 0) return null;
            return clamp(Math.log10(number + 1) / 6, 0, 1);
        }

        function parseDate(value) {
            if (!value) return null;
            const date = new Date(value);
            return Number.isNaN(date.getTime()) ? null : date;
        }

        function clamp(value, min, max) {
            return Math.max(min, Math.min(max, value));
        }

        function formatDate(value) {
            if (!value) return '';
            const date = new Date(value);
            if (Number.isNaN(date.getTime())) return value;
            return date.toLocaleString();
        }

        function escapeHtml(str) {
            if (!str) return '';
            const div = document.createElement('div');
            div.appendChild(document.createTextNode(str));
            return div.innerHTML;
        }

        document.addEventListener('DOMContentLoaded', () => {
            bindAnalysisDebugToggle();
            loadAnalysis();
        });
    </script>
</body>
</html>"#;

async fn serve_html() -> Html<&'static str> {
    Html(HTML_TEMPLATE)
}

async fn serve_analysis_html() -> Html<&'static str> {
    Html(ANALYSIS_HTML_TEMPLATE)
}

async fn serve_catalog(
    State(config): State<AppConfig>,
    Query(query): Query<CatalogQuery>,
) -> Json<Vec<CatalogSport>> {
    Json(load_or_refresh_catalog(&config, query.refresh.unwrap_or(false)).await)
}

async fn serve_sport_sections(
    State(config): State<AppConfig>,
    AxumPath(sport_slug): AxumPath<String>,
    Query(query): Query<SectionQuery>,
) -> axum::response::Result<Json<serde_json::Value>> {
    let response =
        load_or_refresh_sport_sections(&config, &sport_slug, query.refresh.unwrap_or(false)).await;
    Ok(Json(serde_json::json!(response)))
}

async fn serve_group_tournaments(
    State(config): State<AppConfig>,
    AxumPath(group_slug): AxumPath<String>,
    Query(query): Query<SectionQuery>,
) -> Json<TournamentsResponse> {
    Json(
        load_or_refresh_group_tournaments(&config, &group_slug, query.refresh.unwrap_or(false))
            .await,
    )
}

async fn serve_section(
    State(config): State<AppConfig>,
    AxumPath(section_slug): AxumPath<String>,
    Query(query): Query<SectionQuery>,
) -> Json<SectionResponse> {
    Json(load_or_refresh_section(&config, &section_slug, query.refresh.unwrap_or(false)).await)
}

async fn serve_scheduler(State(config): State<AppConfig>) -> Json<SchedulerResponse> {
    let cache = crate::scheduler::read_scheduler_cache(&config).await;
    Json(SchedulerResponse {
        matches: cache.matches,
    })
}

async fn serve_analysis(State(config): State<AppConfig>) -> Json<AnalysisResponse> {
    Json(load_analysis_response(&config).await)
}

async fn serve_analysis_latest_odds(
    State(config): State<AppConfig>,
    AxumPath(match_id): AxumPath<String>,
) -> Json<AnalysisLatestOddsResponse> {
    if !config.db.exists() {
        return Json(AnalysisLatestOddsResponse {
            success: false,
            latest: None,
            error: Some("analysis database does not exist".to_string()),
        });
    }

    let db_url = format!("sqlite://{}", config.db.display());
    match crate::storage::connect_sqlite(&db_url).await {
        Ok(pool) => match crate::storage::load_analysis_latest_odds(&pool, &match_id, 80).await {
            Ok(latest) => Json(AnalysisLatestOddsResponse {
                success: true,
                latest: Some(latest),
                error: None,
            }),
            Err(error) => Json(AnalysisLatestOddsResponse {
                success: false,
                latest: None,
                error: Some(error.to_string()),
            }),
        },
        Err(error) => Json(AnalysisLatestOddsResponse {
            success: false,
            latest: None,
            error: Some(error.to_string()),
        }),
    }
}

async fn serve_analysis_odds_series(
    State(config): State<AppConfig>,
    AxumPath(match_id): AxumPath<String>,
) -> Json<AnalysisOddsSeriesResponse> {
    if !config.db.exists() {
        return Json(AnalysisOddsSeriesResponse {
            success: false,
            points: Vec::new(),
            error: Some("analysis database does not exist".to_string()),
        });
    }

    let db_url = format!("sqlite://{}", config.db.display());
    match crate::storage::connect_sqlite(&db_url).await {
        Ok(pool) => match crate::storage::load_analysis_odds_series(&pool, &match_id, 600).await {
            Ok(points) => Json(AnalysisOddsSeriesResponse {
                success: true,
                points,
                error: None,
            }),
            Err(error) => Json(AnalysisOddsSeriesResponse {
                success: false,
                points: Vec::new(),
                error: Some(error.to_string()),
            }),
        },
        Err(error) => Json(AnalysisOddsSeriesResponse {
            success: false,
            points: Vec::new(),
            error: Some(error.to_string()),
        }),
    }
}

async fn delete_analysis_match(
    State(config): State<AppConfig>,
    AxumPath(match_id): AxumPath<String>,
) -> Json<DeleteMatchResponse> {
    if let Err(error) =
        crate::scheduler::remove_scheduled_matches_for_match_id(&config, &match_id).await
    {
        tracing::warn!(match_id = %match_id, "failed to remove scheduled matches for deleted analysis match: {}", error);
    }

    let deleted_rows = if config.db.exists() {
        let db_url = format!("sqlite://{}", config.db.display());
        match crate::storage::connect_sqlite(&db_url).await {
            Ok(pool) => crate::storage::delete_match_data(&pool, &match_id)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(match_id = %match_id, "failed to delete match data: {}", error);
                    0
                }),
            Err(error) => {
                tracing::warn!("failed to connect analysis database for delete: {}", error);
                0
            }
        }
    } else {
        0
    };

    Json(DeleteMatchResponse {
        deleted_rows,
        analysis: load_analysis_response(&config).await,
    })
}

async fn load_analysis_response(config: &AppConfig) -> AnalysisResponse {
    let scheduled = crate::scheduler::read_scheduler_cache(&config).await;
    let (collected_matches, debug_points) = if config.db.exists() {
        let db_url = format!("sqlite://{}", config.db.display());
        match crate::storage::connect_sqlite(&db_url).await {
            Ok(pool) => {
                let collected_matches = crate::storage::load_analysis_summaries(&pool)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::warn!("failed to load analysis summaries: {}", error);
                        Vec::new()
                    });
                let debug_points = crate::storage::load_analysis_debug_points(&pool, 300)
                    .await
                    .unwrap_or_else(|error| {
                        tracing::warn!("failed to load analysis debug points: {}", error);
                        Vec::new()
                    });
                (collected_matches, debug_points)
            }
            Err(error) => {
                tracing::warn!("failed to connect analysis database: {}", error);
                (Vec::new(), Vec::new())
            }
        }
    } else {
        (Vec::new(), Vec::new())
    };

    AnalysisResponse {
        db_path: config.db.display().to_string(),
        scheduled_matches: scheduled.matches,
        collected_matches,
        debug_points,
    }
}

async fn add_scheduler_match(
    State(config): State<AppConfig>,
    Json(new_match): Json<NewScheduledMatch>,
) -> Json<SchedulerResponse> {
    let cache = crate::scheduler::add_scheduled_match(&config, new_match)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!("failed to add scheduled match: {}", error);
            SchedulerCache::default()
        });
    Json(SchedulerResponse {
        matches: cache.matches,
    })
}

async fn force_scheduler_match(
    State(config): State<AppConfig>,
    Json(mut new_match): Json<NewScheduledMatch>,
) -> Json<ForceSchedulerResponse> {
    new_match.is_finished = false;
    new_match.status = Some(
        new_match
            .status
            .filter(|status| !status.trim().is_empty())
            .unwrap_or_else(|| "Debug forced".to_string()),
    );
    let requested_match = new_match.clone();

    let existing_cache = crate::scheduler::read_scheduler_cache(&config).await;
    if let Some(existing_match) = existing_cache
        .matches
        .iter()
        .find(|item| scheduled_match_matches_request(item, &requested_match))
        .cloned()
    {
        if let Err(error) = crate::scheduler::update_scheduled_match_state(
            &config,
            &existing_match.id,
            Some("Debug forced".to_string()),
            false,
            None,
            None,
            requested_match.end_time.clone(),
        )
        .await
        {
            return Json(ForceSchedulerResponse {
                matches: existing_cache.matches,
                started: false,
                schedule_id: Some(existing_match.id),
                error: Some(error.to_string()),
            });
        }

        let cache = crate::scheduler::read_scheduler_cache(&config).await;
        let scheduled_match = cache
            .matches
            .iter()
            .find(|item| item.id == existing_match.id)
            .cloned()
            .unwrap_or(existing_match);
        let schedule_id = scheduled_match.id.clone();
        let started = spawn_scheduled_match_collection(config.clone(), scheduled_match).await;

        return Json(ForceSchedulerResponse {
            matches: cache.matches,
            started,
            schedule_id: Some(schedule_id),
            error: None,
        });
    }

    match crate::scheduler::add_scheduled_match(&config, new_match).await {
        Ok(cache) => {
            let scheduled_match = cache
                .matches
                .iter()
                .find(|item| scheduled_match_matches_request(item, &requested_match))
                .cloned();
            let (started, schedule_id) = if let Some(scheduled_match) = scheduled_match {
                let schedule_id = scheduled_match.id.clone();
                let started =
                    spawn_scheduled_match_collection(config.clone(), scheduled_match).await;
                (started, Some(schedule_id))
            } else {
                (false, None)
            };

            Json(ForceSchedulerResponse {
                matches: cache.matches,
                started,
                schedule_id,
                error: None,
            })
        }
        Err(error) => Json(ForceSchedulerResponse {
            matches: crate::scheduler::read_scheduler_cache(&config)
                .await
                .matches,
            started: false,
            schedule_id: None,
            error: Some(error.to_string()),
        }),
    }
}

async fn remove_scheduler_match(
    State(config): State<AppConfig>,
    AxumPath(schedule_id): AxumPath<String>,
) -> Json<SchedulerResponse> {
    let cache = crate::scheduler::remove_scheduled_match(&config, &schedule_id)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!("failed to remove scheduled match: {}", error);
            SchedulerCache::default()
        });
    Json(SchedulerResponse {
        matches: cache.matches,
    })
}

fn scheduled_match_matches_request(
    scheduled_match: &crate::scheduler::ScheduledMatch,
    requested_match: &NewScheduledMatch,
) -> bool {
    let has_requested_url =
        requested_match.oddsportal_url.is_some() || requested_match.polymarket_url.is_some();

    if let Some(url) = requested_match.oddsportal_url.as_deref()
        && scheduled_match.oddsportal_url.as_deref() == Some(url)
    {
        return true;
    }
    if let Some(url) = requested_match.polymarket_url.as_deref()
        && scheduled_match.polymarket_url.as_deref() == Some(url)
    {
        return true;
    }
    if let Some(requested_slug) = requested_match
        .polymarket_url
        .as_deref()
        .and_then(last_url_segment)
        && scheduled_match
            .polymarket_url
            .as_deref()
            .and_then(last_url_segment)
            .as_deref()
            == Some(requested_slug.as_str())
    {
        return true;
    }
    if has_requested_url {
        return false;
    }

    scheduled_match.team1 == requested_match.team1
        && scheduled_match.team2 == requested_match.team2
        && scheduled_match.match_time == requested_match.match_time
}

fn last_url_segment(value: &str) -> Option<String> {
    url::Url::parse(value)
        .ok()?
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .next_back()
        .map(str::to_string)
}

async fn serve_json_with_data(
    State(data): State<Vec<SportMatchesData>>,
) -> Json<Vec<SportMatchesData>> {
    Json(data)
}

async fn serve_catalog_with_data(
    State(data): State<Vec<SportMatchesData>>,
) -> Json<Vec<CatalogSport>> {
    let catalog = data
        .into_iter()
        .map(|sport| {
            let cached_match_count = sport
                .sections
                .iter()
                .map(|section| section.matches.len())
                .sum();

            CatalogSport {
                sport_slug: slug_for_sport_name(&sport.sport_name),
                sport_name: sport.sport_name,
                section_count: sport.sections.len(),
                cached_match_count,
                last_loaded_at: None,
            }
        })
        .collect();
    Json(catalog)
}

async fn serve_sport_sections_with_data(
    State(data): State<Vec<SportMatchesData>>,
    AxumPath(sport_slug): AxumPath<String>,
) -> Json<SportSectionsResponse> {
    let sport_name = sport_name_for_slug(&sport_slug);
    for sport in data {
        if slug_for_sport_name(&sport.sport_name) == sport_slug {
            let sections = sport
                .sections
                .into_iter()
                .map(|section| CatalogSection {
                    game_name: sport.sport_name.clone(),
                    game_slug: sport_slug.clone(),
                    section_slug: slug_for_sport_name(&section.section_name),
                    section_name: section.section_name,
                    oddsportal_url: String::new(),
                    polymarket_url: String::new(),
                    match_count: section.matches.len(),
                    last_loaded_at: None,
                })
                .collect();
            return Json(SportSectionsResponse {
                sport_name,
                sport_slug,
                last_loaded_at: None,
                sections,
            });
        }
    }

    Json(SportSectionsResponse {
        sport_name,
        sport_slug,
        last_loaded_at: None,
        sections: Vec::new(),
    })
}

async fn serve_section_with_data(
    State(data): State<Vec<SportMatchesData>>,
    AxumPath(section_slug): AxumPath<String>,
) -> Json<SectionResponse> {
    for sport in data {
        for section in sport.sections {
            if slug_for_sport_name(&section.section_name) == section_slug {
                return Json(SectionResponse {
                    section_slug,
                    section_name: section.section_name,
                    last_loaded_at: None,
                    matches: section.matches,
                });
            }
        }
    }

    Json(SectionResponse {
        section_slug,
        section_name: "Unknown".to_string(),
        last_loaded_at: None,
        matches: Vec::new(),
    })
}

pub async fn serve_matches_config(config: AppConfig, port: u16) -> Result<()> {
    start_scheduler_worker(config.clone());

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/analysis", get(serve_analysis_html))
        .route("/api/catalog", get(serve_catalog))
        .route("/api/analysis", get(serve_analysis))
        .route(
            "/api/analysis/match/:match_id/latest",
            get(serve_analysis_latest_odds),
        )
        .route(
            "/api/analysis/match/:match_id/series",
            get(serve_analysis_odds_series),
        )
        .route(
            "/api/analysis/match/:match_id",
            delete(delete_analysis_match),
        )
        .route("/api/sport/:sport_slug/sections", get(serve_sport_sections))
        .route(
            "/api/group/:group_slug/tournaments",
            get(serve_group_tournaments),
        )
        .route("/api/section/:section_slug", get(serve_section))
        .route(
            "/api/scheduler",
            get(serve_scheduler).post(add_scheduler_match),
        )
        .route("/api/scheduler/force", post(force_scheduler_match))
        .route(
            "/api/scheduler/:schedule_id",
            delete(remove_scheduler_match),
        )
        .with_state(config);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Web server starting on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

fn start_scheduler_worker(config: AppConfig) {
    tokio::spawn(async move {
        loop {
            let cache = crate::scheduler::read_scheduler_cache(&config).await;
            for scheduled_match in cache.matches.into_iter().filter(|item| !item.is_finished) {
                spawn_scheduled_match_collection(config.clone(), scheduled_match).await;
            }
            sleep(Duration::from_secs(10)).await;
        }
    });
}

fn scheduler_active_set() -> Arc<Mutex<HashSet<String>>> {
    static ACTIVE: OnceLock<Arc<Mutex<HashSet<String>>>> = OnceLock::new();
    Arc::clone(ACTIVE.get_or_init(|| Arc::new(Mutex::new(HashSet::new()))))
}

async fn spawn_scheduled_match_collection(
    config: AppConfig,
    scheduled_match: crate::scheduler::ScheduledMatch,
) -> bool {
    let active = scheduler_active_set();
    let mut active_guard = active.lock().await;
    if active_guard.contains(&scheduled_match.id) {
        return false;
    }
    active_guard.insert(scheduled_match.id.clone());
    drop(active_guard);

    tokio::spawn(async move {
        let schedule_id = scheduled_match.id.clone();
        info!(
            schedule_id = %schedule_id,
            team1 = %scheduled_match.team1,
            team2 = %scheduled_match.team2,
            "web scheduler worker started collection"
        );
        if let Err(error) = crate::collector::collect_scheduled_match(config, scheduled_match).await
        {
            warn!(
                schedule_id = %schedule_id,
                error = %error,
                "web scheduler worker collection stopped with error"
            );
        }
        active.lock().await.remove(&schedule_id);
    });
    true
}

pub async fn serve_matches(data: Vec<SportMatchesData>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(serve_html))
        .route("/analysis", get(serve_analysis_html))
        .route("/api/matches", get(serve_json_with_data))
        .route("/api/catalog", get(serve_catalog_with_data))
        .route(
            "/api/sport/:sport_slug/sections",
            get(serve_sport_sections_with_data),
        )
        .route("/api/section/:section_slug", get(serve_section_with_data))
        .with_state(data);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Web server starting on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
