//! # 数据采集核心模块
//!
//! 本模块实现了数据采集的核心逻辑，支持多数据源（如 Polymarket、OddsPortal）的并发采集。
//! 主要功能包括：
//!
//! - **多数据源并发采集**：使用 `tokio::task::JoinSet` 同时为多个数据提供商启动独立的采集循环
//! - **退避重试机制**：支持序列退避（Sequence）和指数退避（Doubling）两种策略，在采集失败时自动增加重试间隔
//! - **快照写入**：将每次采集的快照数据持久化到 SQLite 数据库
//! - **比赛标识解析**：从提供商 URL 中解析统一的比赛标识（MatchIdentity）

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use reqwest::Url;
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant, sleep, sleep_until};
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::http::build_http_client;
use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, ParseStatus, ProviderPayload};
use crate::providers::oddsportal::OddsPortalProvider;
use crate::providers::polymarket::PolymarketProvider;
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};
use crate::scheduler::{ScheduledMatch, read_scheduler_cache, update_scheduled_match_state};
use crate::storage::{
    connect_sqlite, insert_failed_snapshot, insert_match, insert_oddsportal_snapshot,
    insert_polymarket_snapshot,
};

/// 退避策略，定义采集失败后如何递增重试延迟。
///
/// 支持两种退避模式：
/// - **Sequence（序列退避）**：按预定义的步骤列表递增延迟。例如 `steps: [2, 5, 10]` 表示
///   第一次失败等待 2 秒，第二次失败等待 5 秒，第三次及之后等待 10 秒（受 cap 限制）。
/// - **Doubling（指数退避）**：每次失败后将延迟翻倍，直到达到上限（cap）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackoffPolicy {
    /// 序列退避：按预定义步骤递增延迟
    Sequence {
        /// 最小延迟（秒）
        base: u64,
        /// 延迟步骤列表，第 N 次失败对应第 N 个步骤值
        steps: Vec<u64>,
        /// 延迟上限（秒）
        cap: u64,
    },
    /// 指数退避：每次失败后延迟翻倍
    Doubling {
        /// 初始延迟（秒），每次翻倍
        base: u64,
        /// 延迟上限（秒）
        cap: u64,
    },
}

impl BackoffPolicy {
    /// Polymarket 数据源的默认退避策略配置。
    ///
    /// 使用序列退避，初始延迟 1 秒，步骤为 [2, 5, 10] 秒，上限 60 秒。
    pub fn polymarket() -> Self {
        Self::Sequence {
            base: 1,
            steps: vec![2, 5, 10],
            cap: 60,
        }
    }

    /// OddsPortal 数据源的默认退避策略配置。
    ///
    /// 使用指数退避，以传入的 `base` 为初始延迟（秒），上限固定为 300 秒（5 分钟）。
    pub fn oddsportal(base: u64) -> Self {
        Self::Doubling { base, cap: 300 }
    }
}

/// 封装了单个数据提供商的调度状态。
///
/// 用于在采集循环中跟踪提供商的失败次数，并根据退避策略计算下一次采集的延迟间隔。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledProvider {
    /// 数据提供商名称
    name: String,
    /// 基础采集间隔（秒），在没有任何失败时使用此值
    base_interval_seconds: u64,
    /// 退避策略
    policy: BackoffPolicy,
    /// 连续失败次数
    failures: u32,
}

/// 记录一次采集尝试的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectionAttempt {
    /// 解析得到的比赛标识
    identity: MatchIdentity,
    /// 是否需要触发退避（HTTP 错误状态或空负载时为 true）
    should_backoff: bool,
}

impl ScheduledProvider {
    /// 创建一个新的调度提供商实例。
    ///
    /// - `name`: 提供商名称标识
    /// - `base_interval_seconds`: 基础采集间隔（秒）
    /// - `policy`: 退避策略
    pub fn new(name: impl Into<String>, base_interval_seconds: u64, policy: BackoffPolicy) -> Self {
        Self {
            name: name.into(),
            base_interval_seconds,
            policy,
            failures: 0,
        }
    }

    /// 计算当前应使用的延迟间隔（秒）。
    ///
    /// 如果失败次数为 0，返回基础间隔；否则根据退避策略计算递增后的延迟。
    pub fn current_delay_seconds(&self) -> u64 {
        let policy_delay = if self.failures == 0 {
            self.base_interval_seconds
        } else {
            match &self.policy {
                BackoffPolicy::Sequence { base, steps, cap } => {
                    let step_index = self.failures.saturating_sub(1) as usize;
                    steps
                        .get(step_index)
                        .copied()
                        .unwrap_or(*cap)
                        .max(*base)
                        .min(*cap)
                }
                BackoffPolicy::Doubling { base, cap } => doubling_delay(*base, *cap, self.failures),
            }
        };

        policy_delay.max(self.base_interval_seconds)
    }

    /// 记录一次成功的采集，重置失败计数器。
    pub fn record_success(&mut self) {
        self.failures = 0;
    }

    /// 记录一次失败的采集，递增失败计数器并打印警告日志。
    pub fn record_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        warn!(
            provider = %self.name,
            failures = self.failures,
            next_delay_seconds = self.current_delay_seconds(),
            "provider collection failed"
        );
    }
}

/// 启动数据采集的主入口函数。
///
/// 主采集流程：
/// 1. **加载配置**：从 `AppConfig` 中读取代理地址、Polymarket/OddsPortal URL 和采集间隔
/// 2. **创建数据库连接**：使用 SQLite 连接池
/// 3. **解析比赛标识**：从提供商 URL 中解析出统一的 `MatchIdentity`
/// 4. **启动并发采集循环**：为每个配置的提供商（Polymarket、OddsPortal）启动一个独立的
///    `run_provider_collection_loop` 任务，使用 `JoinSet` 并发执行
/// 5. **等待所有任务完成**：循环等待直到所有采集任务结束（实际上采集循环是无限的，
///    除非发生未恢复的错误）
pub async fn collect(config: AppConfig) -> Result<()> {
    let scheduler_cache = read_scheduler_cache(&config).await;
    let scheduled_matches = scheduler_cache
        .matches
        .into_iter()
        .filter(|item| !item.is_finished)
        .collect::<Vec<_>>();
    if !scheduled_matches.is_empty() {
        return collect_scheduled_matches(config, scheduled_matches).await;
    }

    let mut tasks = JoinSet::new();
    let db_url = format!("sqlite://{}", config.db.display());
    let pool = connect_sqlite(&db_url).await?;
    let canonical_identity = resolve_collect_identity(&config)?;

    info!(
        match_id = %canonical_identity.match_id,
        home_team = %canonical_identity.home_team,
        away_team = %canonical_identity.away_team,
        "resolved canonical match identity"
    );

    if !config.polymarket.url.is_empty() {
        let schedule = ScheduledProvider::new(
            "polymarket",
            config.polymarket.interval_seconds,
            BackoffPolicy::polymarket(),
        );
        tasks.spawn(run_provider_collection_loop(
            schedule,
            ProviderTarget {
                url: config.polymarket.url.clone(),
                identity: Some(canonical_identity.clone()),
            },
            PolymarketProvider::new(config.proxy_enabled, &config.proxy),
            pool.clone(),
        ));
    }

    if !config.oddsportal.url.is_empty() {
        let schedule = ScheduledProvider::new(
            "oddsportal",
            config.oddsportal.interval_seconds,
            BackoffPolicy::oddsportal(config.oddsportal.interval_seconds),
        );
        tasks.spawn(run_provider_collection_loop(
            schedule,
            ProviderTarget {
                url: config.oddsportal.url.clone(),
                identity: Some(canonical_identity.clone()),
            },
            OddsPortalProvider::new(config.proxy_enabled, &config.proxy),
            pool.clone(),
        ));
    }

    while let Some(result) = tasks.join_next().await {
        result?;
    }

    Ok(())
}

async fn collect_scheduled_matches(
    config: AppConfig,
    scheduled_matches: Vec<ScheduledMatch>,
) -> Result<()> {
    let mut tasks = JoinSet::new();
    let db_url = format!("sqlite://{}", config.db.display());
    let pool = connect_sqlite(&db_url).await?;

    for scheduled_match in scheduled_matches {
        let config = config.clone();
        let pool = pool.clone();
        tasks.spawn(run_scheduled_match_collection_loop(
            config,
            scheduled_match,
            pool,
        ));
    }

    while let Some(result) = tasks.join_next().await {
        result??;
    }

    Ok(())
}

pub async fn collect_scheduled_match(
    config: AppConfig,
    scheduled_match: ScheduledMatch,
) -> Result<()> {
    let db_url = format!("sqlite://{}", config.db.display());
    let pool = connect_sqlite(&db_url).await?;
    run_scheduled_match_collection_loop(config, scheduled_match, pool).await
}

async fn run_scheduled_match_collection_loop(
    config: AppConfig,
    scheduled_match: ScheduledMatch,
    pool: SqlitePool,
) -> Result<()> {
    let identity = scheduled_match_identity(&scheduled_match);
    let mut polymarket_schedule = ScheduledProvider::new(
        "polymarket",
        config.polymarket.interval_seconds,
        BackoffPolicy::polymarket(),
    );
    let mut oddsportal_schedule = ScheduledProvider::new(
        "oddsportal",
        config.oddsportal.interval_seconds,
        BackoffPolicy::oddsportal(config.oddsportal.interval_seconds),
    );
    let polymarket_provider = PolymarketProvider::new(config.proxy_enabled, &config.proxy);
    let oddsportal_provider = OddsPortalProvider::new(config.proxy_enabled, &config.proxy);
    let polymarket_target = scheduled_match
        .polymarket_url
        .as_ref()
        .map(|url| ProviderTarget {
            url: url.clone(),
            identity: Some(identity.clone()),
        });
    let oddsportal_target = scheduled_match
        .oddsportal_url
        .as_ref()
        .map(|url| ProviderTarget {
            url: url.clone(),
            identity: Some(identity.clone()),
        });

    info!(
        schedule_id = %scheduled_match.id,
        match_id = %identity.match_id,
        home_team = %identity.home_team,
        away_team = %identity.away_team,
        "scheduled match collection started"
    );

    let now = Instant::now();
    let mut next_polymarket_tick = polymarket_target.as_ref().map(|_| now);
    let mut next_oddsportal_tick = oddsportal_target.as_ref().map(|_| now);

    loop {
        let still_scheduled = read_scheduler_cache(&config)
            .await
            .matches
            .iter()
            .any(|item| item.id == scheduled_match.id);
        if !still_scheduled {
            info!(
                schedule_id = %scheduled_match.id,
                match_id = %identity.match_id,
                "scheduled match removed from cache; collection stopped"
            );
            break;
        }

        let now = Instant::now();
        let polymarket_due = next_polymarket_tick.is_some_and(|next_tick| now >= next_tick);
        let oddsportal_due = next_oddsportal_tick.is_some_and(|next_tick| now >= next_tick);

        if polymarket_due && let Some(target) = &polymarket_target {
            collect_scheduled_provider_tick(
                &mut polymarket_schedule,
                target,
                &polymarket_provider,
                &pool,
            )
            .await;

            if let Some(api_state) =
                fetch_polymarket_schedule_state(&config, &scheduled_match).await
            {
                let is_finished = api_state.is_finished;
                update_scheduled_match_state(
                    &config,
                    &scheduled_match.id,
                    api_state.status,
                    api_state.is_finished,
                    None,
                    None,
                    api_state.end_time,
                )
                .await?;
                if is_finished {
                    info!(
                        schedule_id = %scheduled_match.id,
                        match_id = %identity.match_id,
                        "scheduled match closed by Polymarket API; collection stopped"
                    );
                    break;
                }
            }

            next_polymarket_tick = Some(
                Instant::now() + Duration::from_secs(polymarket_schedule.current_delay_seconds()),
            );
        }

        if oddsportal_due && let Some(target) = &oddsportal_target {
            let match_state = collect_scheduled_provider_tick(
                &mut oddsportal_schedule,
                target,
                &oddsportal_provider,
                &pool,
            )
            .await;

            if let Some(match_state) = match_state {
                let is_finished = match_state.is_finished;
                update_scheduled_match_state(
                    &config,
                    &scheduled_match.id,
                    match_state.status,
                    match_state.is_finished,
                    match_state.score,
                    match_state.partial_score,
                    match_state.end_time,
                )
                .await?;
                if is_finished {
                    info!(
                        schedule_id = %scheduled_match.id,
                        match_id = %identity.match_id,
                        "scheduled match finished; collection stopped"
                    );
                    break;
                }
            }

            next_oddsportal_tick = Some(
                Instant::now() + Duration::from_secs(oddsportal_schedule.current_delay_seconds()),
            );
        }

        let next_tick = [next_polymarket_tick, next_oddsportal_tick]
            .into_iter()
            .flatten()
            .min();

        if let Some(next_tick) = next_tick {
            sleep_until(next_tick).await;
        } else {
            break;
        }
    }

    Ok(())
}

async fn collect_scheduled_provider_tick<P>(
    schedule: &mut ScheduledProvider,
    target: &ProviderTarget,
    provider: &P,
    pool: &SqlitePool,
) -> Option<ScheduledMatchState>
where
    P: Provider + Send + Sync,
{
    info!(
        provider = %schedule.name,
        url = %target.url,
        match_id = %schedule_match_id(&target.identity),
        "scheduled collection tick"
    );

    match provider.fetch_snapshot(target).await {
        Ok(snapshot) => {
            let finished = if provider.source_name() == "oddsportal" {
                snapshot.raw_body.as_deref().and_then(parse_match_state)
            } else {
                None
            };
            match write_snapshot(pool, target, snapshot).await {
                Ok(attempt) => {
                    if attempt.should_backoff {
                        schedule.record_failure();
                    } else {
                        schedule.record_success();
                    }
                }
                Err(error) => {
                    warn!(
                        provider = %schedule.name,
                        url = %target.url,
                        error = %error,
                        "scheduled provider snapshot write failed"
                    );
                    schedule.record_failure();
                }
            }
            finished
        }
        Err(error) => {
            warn!(
                provider = %schedule.name,
                url = %target.url,
                error = %error,
                known_match_id = target.identity.as_ref().map(|identity| identity.match_id.as_str()),
                "scheduled provider collection attempt failed"
            );
            if let Some(identity) = &target.identity {
                if let Err(storage_error) = insert_match(
                    pool,
                    identity,
                    "",
                    provider.source_name(),
                    Some(&target.url),
                )
                .await
                {
                    warn!(
                        provider = %schedule.name,
                        url = %target.url,
                        match_id = %identity.match_id,
                        error = %storage_error,
                        "failed to store scheduled match before failure snapshot"
                    );
                } else if let Err(storage_error) = insert_failed_snapshot(
                    pool,
                    &identity.match_id,
                    provider.source_name(),
                    Utc::now(),
                    None,
                    &error.to_string(),
                )
                .await
                {
                    warn!(
                        provider = %schedule.name,
                        url = %target.url,
                        match_id = %identity.match_id,
                        error = %storage_error,
                        "failed to store scheduled provider failure snapshot"
                    );
                }
            }
            schedule.record_failure();
            None
        }
    }
}

fn scheduled_match_identity(scheduled_match: &ScheduledMatch) -> MatchIdentity {
    scheduled_match
        .oddsportal_url
        .as_deref()
        .or(scheduled_match.polymarket_url.as_deref())
        .and_then(|url| resolve_from_text(url).ok())
        .unwrap_or_else(|| MatchIdentity {
            match_id: crate::match_resolver::match_id_for(
                &scheduled_match.team1,
                &scheduled_match.team2,
            ),
            home_team: scheduled_match.team1.clone(),
            away_team: scheduled_match.team2.clone(),
            match_time: None,
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScheduledMatchState {
    status: Option<String>,
    is_finished: bool,
    score: Option<String>,
    partial_score: Option<String>,
    end_time: Option<String>,
}

fn parse_match_state(body: &str) -> Option<ScheduledMatchState> {
    let status = extract_jsonish_value(body, "eventStageName")
        .or_else(|| extract_jsonish_value(body, "event-stage-name"));
    let is_finished = body.contains(r#""isFinished":true"#)
        || body.contains(r#"&quot;isFinished&quot;:true"#)
        || status
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("finished"))
            .unwrap_or(false);
    let score = extract_jsonish_value(body, "result")
        .or_else(|| extract_jsonish_value(body, "postmatchResult"))
        .or_else(|| {
            let home = extract_jsonish_value(body, "homeResult")?;
            let away = extract_jsonish_value(body, "awayResult")?;
            if home.is_empty() || away.is_empty() {
                None
            } else {
                Some(format!("{}:{}", home, away))
            }
        });
    let partial_score = extract_jsonish_value(body, "partialresult");

    if status.is_none() && score.is_none() && partial_score.is_none() && !is_finished {
        return None;
    }

    Some(ScheduledMatchState {
        status: if is_finished && status.is_none() {
            Some("Finished".to_string())
        } else {
            status
        },
        is_finished,
        score,
        partial_score,
        end_time: None,
    })
}

async fn fetch_polymarket_schedule_state(
    config: &AppConfig,
    scheduled_match: &ScheduledMatch,
) -> Option<ScheduledMatchState> {
    let polymarket_url = scheduled_match.polymarket_url.as_deref()?;
    let client = match build_http_client(config.proxy_enabled, &config.proxy) {
        Ok(client) => client,
        Err(error) => {
            warn!(error = %error, "failed to build Polymarket API client");
            return None;
        }
    };

    for api_url in polymarket_schedule_api_urls(polymarket_url, scheduled_match) {
        let response = match client.get(&api_url).send().await {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    url = %api_url,
                    error = %error,
                    "Polymarket schedule API request failed"
                );
                continue;
            }
        };
        if !response.status().is_success() {
            warn!(
                url = %api_url,
                status = %response.status(),
                "Polymarket schedule API returned non-success status"
            );
            continue;
        }
        let value = match response.json::<Value>().await {
            Ok(value) => value,
            Err(error) => {
                warn!(
                    url = %api_url,
                    error = %error,
                    "failed to parse Polymarket schedule API JSON"
                );
                continue;
            }
        };
        if let Some(state) = parse_polymarket_schedule_state(&value, scheduled_match) {
            return Some(state);
        }
    }

    None
}

fn polymarket_schedule_api_urls(
    polymarket_url: &str,
    scheduled_match: &ScheduledMatch,
) -> Vec<String> {
    let mut urls = Vec::new();
    let parsed = Url::parse(polymarket_url).ok();
    let segments = parsed
        .as_ref()
        .and_then(|url| url.path_segments())
        .map(|segments| segments.collect::<Vec<_>>())
        .unwrap_or_default();

    if segments.first() == Some(&"event") {
        if let Some(slug) = segments.get(1) {
            let slug = urlencoding::encode(slug);
            urls.push(format!(
                "https://gamma-api.polymarket.com/events?slug={slug}"
            ));
            urls.push(format!(
                "https://gamma-api.polymarket.com/markets?slug={slug}"
            ));
        }
    }

    if segments.first() == Some(&"esports") {
        if let Some(tag_slug) = segments.last().filter(|segment| !segment.is_empty()) {
            let tag_slug = urlencoding::encode(tag_slug);
            urls.push(format!(
                "https://gamma-api.polymarket.com/events?tag_slug={tag_slug}&limit=100"
            ));
            urls.push(format!(
                "https://gamma-api.polymarket.com/markets?tag_slug={tag_slug}&limit=100"
            ));
        }
    }

    let query = format!("{} {}", scheduled_match.team1, scheduled_match.team2);
    let query = urlencoding::encode(&query);
    urls.push(format!(
        "https://gamma-api.polymarket.com/events?limit=100&search={query}"
    ));
    urls.push(format!(
        "https://gamma-api.polymarket.com/markets?limit=100&search={query}"
    ));
    urls
}

fn parse_polymarket_schedule_state(
    value: &Value,
    scheduled_match: &ScheduledMatch,
) -> Option<ScheduledMatchState> {
    let mut state = None;
    visit_polymarket_candidates(value, scheduled_match, &mut state);
    state
}

fn visit_polymarket_candidates(
    value: &Value,
    scheduled_match: &ScheduledMatch,
    state: &mut Option<ScheduledMatchState>,
) {
    if state
        .as_ref()
        .map(|state| state.is_finished)
        .unwrap_or(false)
    {
        return;
    }

    match value {
        Value::Array(items) => {
            for item in items {
                visit_polymarket_candidates(item, scheduled_match, state);
            }
        }
        Value::Object(map) => {
            if is_polymarket_market_or_event(value) {
                let text = polymarket_candidate_text(value);
                if polymarket_candidate_matches(&text, scheduled_match) {
                    let is_finished = polymarket_candidate_closed(value);
                    let next_state = ScheduledMatchState {
                        status: if is_finished {
                            Some("Finished".to_string())
                        } else {
                            None
                        },
                        is_finished,
                        score: None,
                        partial_score: None,
                        end_time: polymarket_candidate_end_time(value),
                    };
                    let should_replace = state.is_none()
                        || next_state.is_finished
                        || state
                            .as_ref()
                            .and_then(|state| state.end_time.as_ref())
                            .is_none();
                    if should_replace {
                        *state = Some(next_state);
                    }
                }
            }
            for child in map.values() {
                visit_polymarket_candidates(child, scheduled_match, state);
            }
        }
        _ => {}
    }
}

fn is_polymarket_market_or_event(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    [
        "title",
        "question",
        "slug",
        "ticker",
        "endDate",
        "closedTime",
        "umaEndDate",
        "gameStartTime",
        "markets",
        "outcomes",
    ]
    .iter()
    .any(|key| map.contains_key(*key))
}

fn polymarket_candidate_text(value: &Value) -> String {
    let mut parts = Vec::new();
    collect_polymarket_text(value, &mut parts);
    parts.join(" ")
}

fn collect_polymarket_text(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if text.len() <= 300 {
                parts.push(text.clone());
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_polymarket_text(item, parts);
            }
        }
        Value::Object(map) => {
            for key in [
                "title",
                "question",
                "marketTitle",
                "slug",
                "ticker",
                "description",
                "outcomes",
                "markets",
            ] {
                if let Some(child) = map.get(key) {
                    collect_polymarket_text(child, parts);
                }
            }
        }
        _ => {}
    }
}

fn polymarket_candidate_matches(text: &str, scheduled_match: &ScheduledMatch) -> bool {
    team_name_in_text(&scheduled_match.team1, text)
        && team_name_in_text(&scheduled_match.team2, text)
}

fn team_name_in_text(team: &str, text: &str) -> bool {
    let team = normalize_polymarket_match_text(team);
    let text = normalize_polymarket_match_text(text);
    if team.is_empty() || text.is_empty() {
        return false;
    }
    if text.contains(&team) {
        return true;
    }
    if let Some(stripped) = team.strip_prefix("team") {
        return !stripped.is_empty() && text.contains(stripped);
    }
    text.contains(&format!("team{team}"))
}

fn normalize_polymarket_match_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn polymarket_candidate_closed(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.get("closed").and_then(Value::as_bool).unwrap_or(false)
                || map
                    .get("archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                || map
                    .get("markets")
                    .map(polymarket_candidate_closed)
                    .unwrap_or(false)
        }
        Value::Array(items) => items.iter().any(polymarket_candidate_closed),
        _ => false,
    }
}

fn polymarket_candidate_end_time(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["closedTime", "umaEndDate", "endDate"] {
                if let Some(value) = map.get(key).and_then(Value::as_str)
                    && !value.is_empty()
                {
                    return Some(value.to_string());
                }
            }
            map.get("markets").and_then(polymarket_candidate_end_time)
        }
        Value::Array(items) => items.iter().find_map(polymarket_candidate_end_time),
        _ => None,
    }
}

fn extract_jsonish_value(body: &str, key: &str) -> Option<String> {
    let patterns = [
        format!(r#""{}":"([^"]*)""#, regex::escape(key)),
        format!(r#"&quot;{}&quot;:&quot;([^&]*)&quot;"#, regex::escape(key)),
    ];
    patterns.iter().find_map(|pattern| {
        regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(body))
            .and_then(|cap| cap.get(1).map(|m| decode_jsonish_label(m.as_str())))
            .filter(|value| !value.is_empty())
    })
}

fn decode_jsonish_label(value: &str) -> String {
    value
        .replace(r#"\/"#, "/")
        .replace(r#"\""#, "\"")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 从配置的提供商 URL 中解析比赛标识（MatchIdentity）。
///
/// 依次尝试从 Polymarket URL 和 OddsPortal URL 中解析，返回第一个成功解析的结果。
/// 如果所有 URL 都无法解析，则返回错误。
fn resolve_collect_identity(config: &AppConfig) -> Result<MatchIdentity> {
    let urls = [&config.polymarket.url, &config.oddsportal.url]
        .into_iter()
        .filter(|url| !url.is_empty());

    for url in urls {
        if let Ok(identity) = resolve_from_text(url) {
            return Ok(identity);
        }
    }

    bail!("could not resolve match identity from configured provider URL before collection")
}

/// 单个数据提供商的采集循环核心逻辑。
///
/// 此函数在一个无限循环中持续执行以下操作：
/// 1. **记录采集日志**：打印当前提供商名称、URL 和比赛 ID
/// 2. **执行单次采集（collect_once）**：调用提供商接口获取快照并写入数据库
/// 3. **错误处理**：
///    - 成功时：根据 `should_backoff` 标记记录成功或失败
///    - 失败时：记录警告日志，尝试将失败快照存入数据库，然后记录失败以触发退避
/// 4. **退避睡眠**：根据 `current_delay_seconds()` 计算出的延迟进行 sleep
async fn run_provider_collection_loop<P>(
    mut schedule: ScheduledProvider,
    target: ProviderTarget,
    provider: P,
    pool: SqlitePool,
) where
    P: Provider + Send + Sync + 'static,
{
    let mut last_identity = target.identity.clone();

    loop {
        info!(
            provider = %schedule.name,
            url = %target.url,
            match_id = %schedule_match_id(&last_identity),
            "collection tick"
        );

        match collect_once(&provider, &target, &pool).await {
            Ok(attempt) => {
                last_identity = Some(attempt.identity);
                if attempt.should_backoff {
                    schedule.record_failure();
                } else {
                    schedule.record_success();
                }
            }
            Err(error) => {
                warn!(
                    provider = %schedule.name,
                    url = %target.url,
                    error = %error,
                    known_match_id = last_identity.as_ref().map(|identity| identity.match_id.as_str()),
                    "provider collection attempt failed"
                );
                if let Some(identity) = &last_identity
                    && let Err(storage_error) = insert_failed_snapshot(
                        &pool,
                        &identity.match_id,
                        provider.source_name(),
                        Utc::now(),
                        None,
                        &error.to_string(),
                    )
                    .await
                {
                    warn!(
                        provider = %schedule.name,
                        url = %target.url,
                        match_id = %identity.match_id,
                        error = %storage_error,
                        "failed to store provider failure snapshot"
                    );
                }
                schedule.record_failure();
            }
        }

        sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
    }
}

/// 从可选的 MatchIdentity 中提取 match_id 字符串。
///
/// 如果 identity 为 None，返回 "unknown"。
fn schedule_match_id(identity: &Option<MatchIdentity>) -> &str {
    identity
        .as_ref()
        .map(|identity| identity.match_id.as_str())
        .unwrap_or("unknown")
}

/// 执行单次采集操作。
///
/// 调用提供商的 `fetch_snapshot` 方法获取快照数据，然后通过 `write_snapshot` 写入数据库。
async fn collect_once<P>(
    provider: &P,
    target: &ProviderTarget,
    pool: &SqlitePool,
) -> Result<CollectionAttempt>
where
    P: Provider + Send + Sync,
{
    let snapshot = provider.fetch_snapshot(target).await?;
    write_snapshot(pool, target, snapshot).await
}

/// 将采集到的快照数据写入数据库。
///
/// 流程：
/// 1. 从快照中提取比赛标识，如果缺失则返回错误
/// 2. 插入/更新比赛记录（insert_match）
/// 3. 根据快照负载类型（Polymarket 价格数据 或 OddsPortal 赔率数据）插入对应的快照记录
/// 4. 判断是否需要触发退避（HTTP 错误或空负载）
/// 5. 返回 CollectionAttempt 包含比赛标识和退避标记
async fn write_snapshot(
    pool: &SqlitePool,
    target: &ProviderTarget,
    snapshot: ProviderSnapshot,
) -> Result<CollectionAttempt> {
    let identity = snapshot.identity.ok_or_else(|| {
        anyhow!(
            "{} snapshot did not include match identity",
            snapshot.source
        )
    })?;

    insert_match(pool, &identity, "", snapshot.source, Some(&target.url)).await?;

    let http_status = snapshot.http_status.map(i64::from);
    let status_error_message = snapshot.http_status.and_then(http_status_error_message);
    let is_empty_payload = match snapshot.payload {
        ProviderPayload::Polymarket { prices } => {
            let is_empty_payload = prices.is_empty();
            let parse_status =
                snapshot_parse_status(status_error_message.is_some(), is_empty_payload);
            insert_polymarket_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                status_error_message.as_deref(),
                &prices,
            )
            .await?;
            is_empty_payload
        }
        ProviderPayload::OddsPortal { odds } => {
            let is_empty_payload = odds.is_empty();
            let parse_status =
                snapshot_parse_status(status_error_message.is_some(), is_empty_payload);
            insert_oddsportal_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                status_error_message.as_deref(),
                &odds,
            )
            .await?;
            is_empty_payload
        }
    };

    Ok(CollectionAttempt {
        should_backoff: should_backoff_after_snapshot(
            status_error_message.is_some(),
            is_empty_payload,
        ),
        identity,
    })
}

/// 根据快照结果判断解析状态。
///
/// 优先级：HTTP 错误 > 空负载 > 解析成功
fn snapshot_parse_status(has_http_status_error: bool, is_empty: bool) -> ParseStatus {
    if has_http_status_error {
        ParseStatus::Failed
    } else if is_empty {
        ParseStatus::Empty
    } else {
        ParseStatus::Parsed
    }
}

/// 判断在写入快照后是否需要触发退避。
///
/// 当存在 HTTP 状态错误或负载为空时返回 true，表示需要增加重试间隔。
fn should_backoff_after_snapshot(has_http_status_error: bool, is_empty_payload: bool) -> bool {
    has_http_status_error || is_empty_payload
}

/// 根据 HTTP 状态码生成错误消息。
///
/// 如果状态码不在 200-299 范围内，返回错误描述字符串；否则返回 None。
fn http_status_error_message(status: u16) -> Option<String> {
    if !(200..300).contains(&status) {
        Some(format!("http status {status}"))
    } else {
        None
    }
}

/// 计算指数退避延迟。
///
/// 从 base 开始，每次失败翻倍延迟，直到达到 cap 上限。
fn doubling_delay(base: u64, cap: u64, failures: u32) -> u64 {
    let mut delay = base;

    for _ in 0..failures {
        delay = delay.saturating_mul(2).min(cap);
        if delay == cap {
            break;
        }
    }

    delay
}

#[cfg(test)]
mod tests {
    use super::{
        http_status_error_message, parse_polymarket_schedule_state, resolve_collect_identity,
        should_backoff_after_snapshot, snapshot_parse_status,
    };
    use crate::config::AppConfig;
    use crate::model::ParseStatus;
    use crate::scheduler::ScheduledMatch;

    /// 测试：空负载应触发退避，但解析状态标记为 Empty 而非 Failed
    #[test]
    fn empty_payload_triggers_backoff_without_marking_parse_failed() {
        assert_eq!(snapshot_parse_status(false, true), ParseStatus::Empty);
        assert!(should_backoff_after_snapshot(false, true));
    }

    /// 测试：非成功 HTTP 状态码应标记为 Failed 并触发退避
    #[test]
    fn non_success_http_status_is_failed_and_triggers_backoff() {
        assert_eq!(
            http_status_error_message(429),
            Some("http status 429".to_string())
        );
        assert_eq!(snapshot_parse_status(true, false), ParseStatus::Failed);
        assert!(should_backoff_after_snapshot(true, false));
    }

    /// 测试：成功解析的快照不应触发退避
    #[test]
    fn parsed_success_snapshot_does_not_backoff() {
        assert_eq!(http_status_error_message(200), None);
        assert_eq!(snapshot_parse_status(false, false), ParseStatus::Parsed);
        assert!(!should_backoff_after_snapshot(false, false));
    }

    /// 测试：在启动采集循环前需要成功解析比赛标识
    #[test]
    fn collect_requires_resolvable_identity_before_looping() {
        let config = AppConfig {
            proxy_enabled: true,
            proxy: "http://10.32.110.233:7890".to_string(),
            db: std::path::PathBuf::from("data/polymarket_analysis.sqlite"),
            polymarket: crate::config::PolymarketConfig {
                url: "https://polymarket.com/event/southampton-vs-wrexham".to_string(),
                interval_seconds: 1,
            },
            oddsportal: crate::config::OddsPortalConfig {
                url: "https://www.oddsportal.com/football/england/championship/wrexham-vs-southampton/".to_string(),
                interval_seconds: 1,
            },
            scrape_esport: crate::config::ScrapeEsportConfig {
                url: "https://www.oddsportal.com/esports/dota-2/dota-2-blast-slam-vii/".to_string(),
                output: std::path::PathBuf::from("data/esport"),
            },
            scrape_esport_games: Vec::new(),
            scrape_sports: crate::config::ScrapeSportsConfig { sports: Vec::new() },
            web: crate::config::WebConfig { port: 23333 },
            discovery: crate::config::DiscoveryConfig {
                polymarket_search_url: "https://polymarket.com/markets?q={query}".to_string(),
                oddsportal_search_url: "https://www.oddsportal.com/search/{query}/".to_string(),
            },
            export: crate::config::ExportConfig {
                match_id: "southampton_vs_wrexham".to_string(),
                format: crate::config::ExportFormat::Jsonl,
            },
        };

        let identity = resolve_collect_identity(&config).unwrap();

        assert_eq!(identity.match_id, "southampton_vs_wrexham");
    }

    /// 测试：空 URL 应被跳过
    #[test]
    fn empty_url_is_skipped_when_resolving_identity() {
        let config = AppConfig {
            proxy_enabled: true,
            proxy: "http://10.32.110.233:7890".to_string(),
            db: std::path::PathBuf::from("data/polymarket_analysis.sqlite"),
            polymarket: crate::config::PolymarketConfig {
                url: "https://polymarket.com/event/southampton-vs-wrexham".to_string(),
                interval_seconds: 1,
            },
            oddsportal: crate::config::OddsPortalConfig {
                url: String::new(),
                interval_seconds: 1,
            },
            scrape_esport: crate::config::ScrapeEsportConfig {
                url: "https://www.oddsportal.com/esports/dota-2/dota-2-blast-slam-vii/".to_string(),
                output: std::path::PathBuf::from("data/esport"),
            },
            scrape_esport_games: Vec::new(),
            scrape_sports: crate::config::ScrapeSportsConfig { sports: Vec::new() },
            web: crate::config::WebConfig { port: 23333 },
            discovery: crate::config::DiscoveryConfig {
                polymarket_search_url: "https://polymarket.com/markets?q={query}".to_string(),
                oddsportal_search_url: "https://www.oddsportal.com/search/{query}/".to_string(),
            },
            export: crate::config::ExportConfig {
                match_id: "southampton_vs_wrexham".to_string(),
                format: crate::config::ExportFormat::Jsonl,
            },
        };

        let identity = resolve_collect_identity(&config).unwrap();

        assert_eq!(identity.match_id, "southampton_vs_wrexham");
    }

    /// 测试：两个 URL 都为空时应报错
    #[test]
    fn empty_urls_fail_identity_resolution() {
        let config = AppConfig {
            proxy_enabled: true,
            proxy: "http://10.32.110.233:7890".to_string(),
            db: std::path::PathBuf::from("data/polymarket_analysis.sqlite"),
            polymarket: crate::config::PolymarketConfig {
                url: String::new(),
                interval_seconds: 1,
            },
            oddsportal: crate::config::OddsPortalConfig {
                url: String::new(),
                interval_seconds: 1,
            },
            scrape_esport: crate::config::ScrapeEsportConfig {
                url: "https://www.oddsportal.com/esports/dota-2/dota-2-blast-slam-vii/".to_string(),
                output: std::path::PathBuf::from("data/esport"),
            },
            scrape_esport_games: Vec::new(),
            scrape_sports: crate::config::ScrapeSportsConfig { sports: Vec::new() },
            web: crate::config::WebConfig { port: 23333 },
            discovery: crate::config::DiscoveryConfig {
                polymarket_search_url: "https://polymarket.com/markets?q={query}".to_string(),
                oddsportal_search_url: "https://www.oddsportal.com/search/{query}/".to_string(),
            },
            export: crate::config::ExportConfig {
                match_id: "southampton_vs_wrexham".to_string(),
                format: crate::config::ExportFormat::Jsonl,
            },
        };

        assert!(resolve_collect_identity(&config).is_err());
    }

    #[test]
    fn parses_live_match_state_from_oddsportal_body() {
        let body = r#"
            {"eventStageName":"2nd Set","homeResult":"1","awayResult":"0","partialresult":"6:4, 2:1","isFinished":false}
        "#;

        let state = super::parse_match_state(body).expect("match state");

        assert_eq!(state.status.as_deref(), Some("2nd Set"));
        assert!(!state.is_finished);
        assert_eq!(state.score.as_deref(), Some("1:0"));
        assert_eq!(state.partial_score.as_deref(), Some("6:4, 2:1"));
    }

    #[test]
    fn parses_finished_match_state_from_oddsportal_body() {
        let body = r#"
            {"eventStageName":"Finished","result":"2:1","partialresult":"4:6, 7:5, 6:4","isFinished":true}
        "#;

        let state = super::parse_match_state(body).expect("match state");

        assert_eq!(state.status.as_deref(), Some("Finished"));
        assert!(state.is_finished);
        assert_eq!(state.score.as_deref(), Some("2:1"));
        assert_eq!(state.partial_score.as_deref(), Some("4:6, 7:5, 6:4"));
    }

    #[test]
    fn parses_polymarket_gamma_end_time_for_fuzzy_team_match() {
        let scheduled_match = scheduled_match("Vitality", "GIANTX");
        let value = serde_json::json!([
            {
                "title": "LEC: Team Vitality vs. GIANTX",
                "closed": false,
                "endDate": "2026-05-31T17:00:00Z",
                "markets": [
                    {
                        "question": "LEC: Team Vitality vs. GIANTX",
                        "closed": false,
                        "endDate": "2026-05-31T17:30:00Z"
                    }
                ]
            }
        ]);

        let state = parse_polymarket_schedule_state(&value, &scheduled_match).expect("state");

        assert!(!state.is_finished);
        assert_eq!(state.status, None);
        assert_eq!(state.end_time.as_deref(), Some("2026-05-31T17:00:00Z"));
    }

    #[test]
    fn parses_polymarket_gamma_closed_match() {
        let scheduled_match = scheduled_match("Falcons", "Team Yandex");
        let value = serde_json::json!({
            "title": "Dota 2: Falcons vs. Team Yandex",
            "closed": true,
            "closedTime": "2026-05-31 18:12:00+00"
        });

        let state = parse_polymarket_schedule_state(&value, &scheduled_match).expect("state");

        assert!(state.is_finished);
        assert_eq!(state.status.as_deref(), Some("Finished"));
        assert_eq!(state.end_time.as_deref(), Some("2026-05-31 18:12:00+00"));
    }

    fn scheduled_match(team1: &str, team2: &str) -> ScheduledMatch {
        ScheduledMatch {
            id: "schedule-test".to_string(),
            team1: team1.to_string(),
            team2: team2.to_string(),
            match_time: "2026-05-31T15:00:00+00:00".to_string(),
            end_time: None,
            oddsportal_url: None,
            polymarket_url: Some(
                "https://polymarket.com/esports/league-of-legends/lec".to_string(),
            ),
            status: Some("Scheduled".to_string()),
            is_finished: false,
            score: None,
            partial_score: None,
            added_at: "2026-05-31T00:00:00Z".to_string(),
            updated_at: "2026-05-31T00:00:00Z".to_string(),
        }
    }
}
