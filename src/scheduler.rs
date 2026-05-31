use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledMatch {
    pub id: String,
    pub team1: String,
    pub team2: String,
    pub match_time: String,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub is_finished: bool,
    pub score: Option<String>,
    pub partial_score: Option<String>,
    pub added_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewScheduledMatch {
    pub team1: String,
    pub team2: String,
    pub match_time: String,
    pub oddsportal_url: Option<String>,
    pub polymarket_url: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub is_finished: bool,
    pub score: Option<String>,
    pub partial_score: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SchedulerCache {
    pub matches: Vec<ScheduledMatch>,
}

pub fn scheduler_cache_path(config: &AppConfig) -> PathBuf {
    config
        .db
        .parent()
        .map(|parent| parent.join("scheduler_cache.json"))
        .unwrap_or_else(|| PathBuf::from("data/scheduler_cache.json"))
}

pub async fn read_scheduler_cache(config: &AppConfig) -> SchedulerCache {
    let path = scheduler_cache_path(config);
    match tokio::fs::read_to_string(path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => SchedulerCache::default(),
    }
}

pub async fn write_scheduler_cache(config: &AppConfig, cache: &SchedulerCache) -> Result<()> {
    let path = scheduler_cache_path(config);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = serde_json::to_string_pretty(cache)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

pub async fn add_scheduled_match(
    config: &AppConfig,
    new_match: NewScheduledMatch,
) -> Result<SchedulerCache> {
    let mut cache = read_scheduler_cache(config).await;
    let id = scheduled_match_id(&new_match);
    let now = Utc::now().to_rfc3339();

    if let Some(existing) = cache.matches.iter_mut().find(|item| item.id == id) {
        existing.team1 = new_match.team1;
        existing.team2 = new_match.team2;
        existing.match_time = new_match.match_time;
        existing.oddsportal_url = new_match.oddsportal_url;
        existing.polymarket_url = new_match.polymarket_url;
        existing.status = new_match.status;
        existing.is_finished = new_match.is_finished;
        existing.score = new_match.score;
        existing.partial_score = new_match.partial_score;
        existing.updated_at = now;
    } else {
        cache.matches.push(ScheduledMatch {
            id,
            team1: new_match.team1,
            team2: new_match.team2,
            match_time: new_match.match_time,
            oddsportal_url: new_match.oddsportal_url,
            polymarket_url: new_match.polymarket_url,
            status: new_match.status,
            is_finished: new_match.is_finished,
            score: new_match.score,
            partial_score: new_match.partial_score,
            added_at: now.clone(),
            updated_at: now,
        });
    }

    cache.matches.sort_by(|a, b| {
        a.is_finished
            .cmp(&b.is_finished)
            .then(a.match_time.cmp(&b.match_time))
            .then(a.team1.cmp(&b.team1))
            .then(a.team2.cmp(&b.team2))
    });
    write_scheduler_cache(config, &cache).await?;
    Ok(cache)
}

pub async fn remove_scheduled_match(config: &AppConfig, id: &str) -> Result<SchedulerCache> {
    let mut cache = read_scheduler_cache(config).await;
    cache.matches.retain(|item| item.id != id);
    write_scheduler_cache(config, &cache).await?;
    Ok(cache)
}

pub async fn mark_scheduled_match_finished(
    config: &AppConfig,
    id: &str,
    status: Option<String>,
    score: Option<String>,
    partial_score: Option<String>,
) -> Result<()> {
    let mut cache = read_scheduler_cache(config).await;
    if let Some(item) = cache.matches.iter_mut().find(|item| item.id == id) {
        item.status = status.or_else(|| Some("Finished".to_string()));
        item.is_finished = true;
        if score.is_some() {
            item.score = score;
        }
        if partial_score.is_some() {
            item.partial_score = partial_score;
        }
        item.updated_at = Utc::now().to_rfc3339();
        write_scheduler_cache(config, &cache).await?;
    }
    Ok(())
}

pub async fn update_scheduled_match_state(
    config: &AppConfig,
    id: &str,
    status: Option<String>,
    is_finished: bool,
    score: Option<String>,
    partial_score: Option<String>,
) -> Result<()> {
    let mut cache = read_scheduler_cache(config).await;
    if let Some(item) = cache.matches.iter_mut().find(|item| item.id == id) {
        if status.is_some() {
            item.status = status;
        }
        item.is_finished = is_finished;
        if score.is_some() {
            item.score = score;
        }
        if partial_score.is_some() {
            item.partial_score = partial_score;
        }
        item.updated_at = Utc::now().to_rfc3339();
        write_scheduler_cache(config, &cache).await?;
    }
    Ok(())
}

fn scheduled_match_id(new_match: &NewScheduledMatch) -> String {
    let source = new_match
        .oddsportal_url
        .as_deref()
        .or(new_match.polymarket_url.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{}|{}|{}",
                new_match.team1, new_match.team2, new_match.match_time
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
