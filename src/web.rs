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
    pub polymarket_url: Option<String>,
    pub oddsportal_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogSport {
    pub sport_name: String,
    pub sport_slug: String,
    pub section_count: usize,
    pub cached_match_count: usize,
    pub last_loaded_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogSection {
    pub section_name: String,
    pub section_slug: String,
    pub oddsportal_url: String,
    pub polymarket_url: String,
    pub match_count: usize,
    pub last_loaded_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct SportSectionsResponse {
    sport_name: String,
    sport_slug: String,
    last_loaded_at: Option<String>,
    sections: Vec<CatalogSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MatchCache {
    sections: HashMap<String, MatchCacheEntry>,
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

#[derive(Clone, Debug, Deserialize)]
struct SectionQuery {
    refresh: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
struct SectionResponse {
    section_name: String,
    section_slug: String,
    last_loaded_at: Option<String>,
    matches: Vec<MatchInfo>,
}

pub fn group_matches_by_category(entries: Vec<(String, Vec<MatchInfo>)>) -> Vec<SportMatchesData> {
    let mut categories: Vec<SportMatchesData> = Vec::new();

    for (raw_name, matches) in entries {
        if matches.is_empty() {
            continue;
        }

        let (category_name, section_name) = display_hierarchy_for(&raw_name);

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
        let (category_name, section_name) = configs
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

fn display_hierarchy_for_config(config: &SportConfig, raw_name: &str) -> (String, String) {
    if let Ok(url) = url::Url::parse(&config.oddsportal_url) {
        let segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();

        if segments.first() == Some(&"esports") {
            let game = segments
                .get(1)
                .copied()
                .filter(|segment| *segment != "h2h")
                .unwrap_or(raw_name);
            return ("Esports".to_string(), titleize_game(game));
        }

        if segments.first() == Some(&"football") {
            return ("Football".to_string(), titleize(raw_name));
        }

        if segments.first() == Some(&"basketball") {
            return ("Basketball".to_string(), titleize(raw_name));
        }
    }

    display_hierarchy_for(raw_name)
}

fn display_hierarchy_for(raw_name: &str) -> (String, String) {
    let normalized = raw_name.trim().to_lowercase().replace('_', "-");

    match normalized.as_str() {
        "football" => ("Football".to_string(), "Football".to_string()),
        "basketball" => ("Basketball".to_string(), "Basketball".to_string()),
        "dota-2" | "dota2" => ("Esports".to_string(), "Dota 2".to_string()),
        "league-of-legends" | "lol" => ("Esports".to_string(), "League of Legends".to_string()),
        "counter-strike" | "cs2" | "csgo" => ("Esports".to_string(), "Counter-Strike".to_string()),
        "esports" => ("Esports".to_string(), "Esports".to_string()),
        _ => (titleize(raw_name), titleize(raw_name)),
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

fn cache_key_for_config(config: &SportConfig) -> String {
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
        let (sport_name, _) = display_hierarchy_for_config(sport_config, &sport_config.name);
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

fn slug_for_sport_name(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
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
        _ => titleize(slug),
    }
}

fn catalog_section_from_config(config: &SportConfig, cache: &MatchCache) -> CatalogSection {
    let (_, section_name) = display_hierarchy_for_config(config, &config.name);
    let cache_key = cache_key_for_config(config);
    let cache_entry = cache.sections.get(&cache_key);

    CatalogSection {
        section_name,
        section_slug: cache_key,
        oddsportal_url: config.oddsportal_url.clone(),
        polymarket_url: config.polymarket_url.clone(),
        match_count: cache_entry.map(|entry| entry.matches.len()).unwrap_or(0),
        last_loaded_at: cache_entry.map(|entry| entry.last_loaded_at.clone()),
    }
}

async fn load_or_refresh_sport_sections(
    config: &AppConfig,
    sport_slug: &str,
    refresh: bool,
) -> SportSectionsResponse {
    let sport_name = sport_name_for_slug(sport_slug);
    let mut section_cache = read_section_cache(config).await;
    let match_cache = read_match_cache(config).await;

    if !refresh {
        if let Some(entry) = section_cache.sports.get(sport_slug) {
            return SportSectionsResponse {
                sport_name,
                sport_slug: sport_slug.to_string(),
                last_loaded_at: Some(entry.last_loaded_at.clone()),
                sections: entry.sections.clone(),
            };
        }
    }

    let mut sections = if sport_slug == "esports" {
        fetch_esports_sections(config, &match_cache).await
    } else {
        config
            .scrape_sports
            .sports
            .iter()
            .filter(|sport_config| {
                let (config_sport_name, _) =
                    display_hierarchy_for_config(sport_config, &sport_config.name);
                slug_for_sport_name(&config_sport_name) == sport_slug
            })
            .map(|sport_config| catalog_section_from_config(sport_config, &match_cache))
            .collect::<Vec<_>>()
    };

    if sections.is_empty() {
        sections.push(CatalogSection {
            section_name: sport_name.clone(),
            section_slug: sport_slug.to_string(),
            oddsportal_url: format!("https://www.oddsportal.com/{}/", sport_slug),
            polymarket_url: format!("https://polymarket.com/sports/{}", sport_slug),
            match_count: 0,
            last_loaded_at: None,
        });
    }

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

    SportSectionsResponse {
        sport_name,
        sport_slug: sport_slug.to_string(),
        last_loaded_at: Some(last_loaded_at),
        sections,
    }
}

async fn fetch_esports_sections(
    config: &AppConfig,
    match_cache: &MatchCache,
) -> Vec<CatalogSection> {
    let client = match build_http_client(config.proxy_enabled, &config.proxy) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(
                "failed to build HTTP client for esports sections: {}",
                error
            );
            return configured_esports_sections(config, match_cache);
        }
    };

    let html = match client
        .get("https://www.oddsportal.com/esports/")
        .send()
        .await
        .and_then(|response| response.error_for_status())
    {
        Ok(response) => response.text().await.unwrap_or_default(),
        Err(error) => {
            tracing::warn!("failed to load esports sections: {}", error);
            return configured_esports_sections(config, match_cache);
        }
    };

    let mut sections = parse_esports_sections(&html, match_cache);
    for fallback in configured_esports_sections(config, match_cache) {
        if !sections
            .iter()
            .any(|section| section.section_slug == fallback.section_slug)
        {
            sections.push(fallback);
        }
    }
    sections.sort_by(|a, b| a.section_name.cmp(&b.section_name));
    sections
}

fn configured_esports_sections(
    config: &AppConfig,
    match_cache: &MatchCache,
) -> Vec<CatalogSection> {
    config
        .scrape_sports
        .sports
        .iter()
        .filter(|sport_config| {
            display_hierarchy_for_config(sport_config, &sport_config.name).0 == "Esports"
        })
        .map(|sport_config| catalog_section_from_config(sport_config, match_cache))
        .collect()
}

fn parse_esports_sections(html: &str, match_cache: &MatchCache) -> Vec<CatalogSection> {
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

async fn load_or_refresh_section(
    config: &AppConfig,
    section_slug: &str,
    refresh: bool,
) -> SectionResponse {
    let mut cache = read_match_cache(config).await;

    if !refresh {
        if let Some(entry) = cache.sections.get(section_slug) {
            return SectionResponse {
                section_name: titleize_game(section_slug),
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
        .unwrap_or_else(|| SportConfig {
            name: section_slug.to_string(),
            oddsportal_url: format!("https://www.oddsportal.com/esports/{}/", section_slug),
            polymarket_url: format!("https://polymarket.com/esports/{}/games", section_slug),
        });

    let scraped = sports_scraper::scrape_all_sports(
        std::slice::from_ref(&sport_config),
        config.proxy_enabled,
        &config.proxy,
    )
    .await
    .unwrap_or_default();

    let matches = scraped
        .into_iter()
        .next()
        .map(|(_, matches)| {
            matches
                .into_iter()
                .map(|m| MatchInfo {
                    team1: m.team1,
                    team2: m.team2,
                    match_time: m.match_time,
                    polymarket_url: m.polymarket_url,
                    oddsportal_url: m.oddsportal_url,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

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
        section_name: titleize_game(section_slug),
        section_slug: section_slug.to_string(),
        last_loaded_at: Some(last_loaded_at),
        matches,
    }
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
            .match-table thead th:nth-child(3),
            .match-table tbody td:nth-child(3) {
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

        async function loadMatches() {
            const main = document.querySelector('main');
            try {
                const res = await fetch('/api/catalog');
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
            let html = '<div class="category-grid">';
            allMatches.forEach((sport, index) => {
                html += '<button type="button" class="category-button" data-sport-index="' + index + '">';
                html += '<span class="category-name">' + escapeHtml(sport.sport_name) + '</span>';
                html += '<span class="category-count">' + sport.cached_match_count + ' cached</span>';
                html += '</button>';
            });
            html += '</div>';
            main.innerHTML = html;
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
                sport.sections = payload.sections || [];
                sport.section_last_loaded_at = payload.last_loaded_at;

                html = renderBreadcrumb([{ label: 'All', action: 'root' }, { label: sport.sport_name }]);
                html += '<div class="sport-card">';
                html += '<div class="sport-card-header">';
                html += '<span>' + escapeHtml(sport.sport_name) + '</span>';
                html += '<button type="button" class="refresh-button" id="refresh-sections">Refresh sections</button>';
                html += '</div>';
                html += '<div class="section-meta">Sections loaded: ' + escapeHtml(formatLoadedAt(payload.last_loaded_at)) + '</div>';
                html += '</div>';
                html += '<div class="category-grid">';
                sport.sections.forEach((section, index) => {
                    html += '<button type="button" class="category-button" data-section-index="' + index + '">';
                    html += '<span class="category-name">' + escapeHtml(section.section_name) + '</span>';
                    html += '<span class="category-count">' + section.match_count + ' cached</span>';
                    html += '</button>';
                });
                html += '</div>';
                main.innerHTML = html;
                bindBreadcrumb(main);
                document.getElementById('refresh-sections').addEventListener('click', () => renderSport(sportIndex, true));
                main.querySelectorAll('[data-section-index]').forEach((button) => {
                    button.addEventListener('click', () => renderMatches(sportIndex, Number(button.dataset.sectionIndex), false));
                });
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading sections: ' + escapeHtml(err.message) + '</div>';
            }
        }

        async function renderMatches(sportIndex, sectionIndex, refresh) {
            const sport = allMatches[sportIndex];
            const section = sport && sport.sections[sectionIndex];
            if (!sport || !section) return renderRoot();

            const main = document.querySelector('main');
            let html = renderBreadcrumb([
                { label: 'All', action: 'root' },
                { label: sport.sport_name, action: 'sport', sportIndex },
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
                section.match_count = payload.matches.length;
                section.last_loaded_at = payload.last_loaded_at;

                html = renderBreadcrumb([
                    { label: 'All', action: 'root' },
                    { label: sport.sport_name, action: 'sport', sportIndex },
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
                document.getElementById('refresh-section').addEventListener('click', () => renderMatches(sportIndex, sectionIndex, true));
            } catch (err) {
                main.innerHTML = '<div class="error-msg">Error loading matches: ' + escapeHtml(err.message) + '</div>';
            }
        }

        function renderMatchTable(matches) {
            let html = '<table class="match-table">';
            html += '<thead><tr><th>Matchup</th><th>Time</th><th>Links</th></tr></thead>';
            html += '<tbody>';
            for (const m of matches) {
                html += '<tr>';
                html += '<td class="match-teams">' + escapeHtml(m.team1) + '<span class="vs">vs</span>' + escapeHtml(m.team2) + '</td>';
                html += '<td class="match-time">' + escapeHtml(m.match_time) + '</td>';
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

async fn serve_catalog(State(config): State<AppConfig>) -> Json<Vec<CatalogSport>> {
    let cache = read_match_cache(&config).await;
    Json(build_catalog(&config, &cache))
}

async fn serve_sport_sections(
    State(config): State<AppConfig>,
    AxumPath(sport_slug): AxumPath<String>,
    Query(query): Query<SectionQuery>,
) -> Json<SportSectionsResponse> {
    Json(load_or_refresh_sport_sections(&config, &sport_slug, query.refresh.unwrap_or(false)).await)
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
