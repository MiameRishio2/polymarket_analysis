use anyhow::Result;
use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    response::{Html, Json},
    routing::get,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::net::TcpListener;
use tracing::info;

use crate::config::{AppConfig, SportConfig};
use crate::http::build_http_client;
use crate::providers::sports_scraper;

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
            return refresh_catalog_counts(entry.sports, &cache);
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
        format!("https://polymarket.com/esports/{}/games", game_slug)
    } else {
        format!(
            "https://polymarket.com/sports/{}",
            segments.first().copied().unwrap_or("sports")
        )
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
        "volleyball" => "Volleyball".to_string(),
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

        let slug = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if slug.is_empty() || !seen.insert(slug.to_string()) {
            continue;
        }

        let section_slug = format!("esports-{}-{}", game_slug, slug);
        let cache_entry = match_cache.sections.get(&section_slug);
        sections.push(CatalogSection {
            game_name: game_name.to_string(),
            game_slug: game_slug.to_string(),
            section_name: titleize(slug),
            section_slug,
            oddsportal_url: format!("https://www.oddsportal.com/esports/{}/{}/", game_slug, slug),
            polymarket_url: format!("https://polymarket.com/esports/{}/games", game_slug),
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

        tournaments.push(TournamentSection {
            section_name: if !link_text.is_empty() {
                link_text
            } else if group_segments.first() == Some(&"esports") {
                titleize(&tournament_slug)
            } else {
                titleize(&tournament_slug)
            },
            section_slug,
            oddsportal_url: format!("https://www.oddsportal.com/{}/{}/", prefix, tournament_slug),
            polymarket_url: polymarket_url_for_path(group_segments),
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
                    polymarket_url_for_path(&path_segments_from_key(section_slug))
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

    if sport_config.oddsportal_url.contains("www.oddsportal.com")
        && matches.iter().any(|m| m.oddsportal_url.is_some())
    {
        matches.retain(|m| m.oddsportal_url.is_some());
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
            return Some((
                format!(
                    "https://www.oddsportal.com/esports/{}/{}/",
                    game_slug, tournament_slug
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
            .match-table thead th:nth-child(5),
            .match-table tbody td:nth-child(5) {
                display: none;
            }
        }
    </style>
</head>
<body>
    <header>
        <h1>Polymarket Analysis</h1>
        <p>Sports Matches Dashboard</p>
    </header>
    <main>
        <div id="loading">Loading matches...</div>
    </main>
    <script>
        let allMatches = [];

        async function loadMatches(refresh) {
            const main = document.querySelector('main');
            try {
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

        function renderRoot() {
            const main = document.querySelector('main');
            let html = '<div class="sport-card">';
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
            document.getElementById('refresh-catalog').addEventListener('click', () => loadMatches(true));
            main.querySelectorAll('[data-sport-index]').forEach((button) => {
                button.addEventListener('click', () => renderSport(Number(button.dataset.sportIndex)));
            });
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
                section.match_count = payload.matches.length;
                section.last_loaded_at = payload.last_loaded_at;

                html = renderBreadcrumb([
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
                html += renderMatchTable(payload.matches);
                html += '</div>';
                main.innerHTML = html;
                bindBreadcrumb(main);
                document.getElementById('refresh-section').addEventListener('click', () => renderMatches(sportIndex, gameSlug, sectionIndex, true));
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading matches: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function renderMatchTable(matches) {
            let html = '<table class="match-table">';
            html += '<thead><tr><th>Matchup</th><th>Time</th><th>Status</th><th>Score</th><th>Links</th></tr></thead>';
            html += '<tbody>';
            for (const m of matches) {
                const statusLabel = m.is_finished ? 'Finished' : (m.status || '');
                const statusClass = m.is_finished ? 'match-status' : 'match-status pending';
                html += '<tr>';
                html += '<td class="match-teams">' + escapeHtml(m.team1) + '<span class="vs">vs</span>' + escapeHtml(m.team2) + '</td>';
                html += '<td class="match-time">' + escapeHtml(m.match_time) + '</td>';
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
            }
            html += '</tbody></table>';
            return html;
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

async fn serve_html() -> Html<&'static str> {
    Html(HTML_TEMPLATE)
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
    let app = Router::new()
        .route("/", get(serve_html))
        .route("/api/catalog", get(serve_catalog))
        .route("/api/sport/:sport_slug/sections", get(serve_sport_sections))
        .route(
            "/api/group/:group_slug/tournaments",
            get(serve_group_tournaments),
        )
        .route("/api/section/:section_slug", get(serve_section))
        .with_state(config);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!("Web server starting on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

pub async fn serve_matches(data: Vec<SportMatchesData>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/", get(serve_html))
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
