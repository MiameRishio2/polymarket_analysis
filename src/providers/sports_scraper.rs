//! 多体育项目赛事数据抓取模块
//!
//! 本模块用于从多个体育项目的 OddsPortal 和 Polymarket 页面抓取未来比赛信息。
//! 主要功能包括：
//!
//! - 支持多种体育项目的并发抓取
//! - 复用 esports_oddsportal 的 scrape_future_matches 和 merge_matches 逻辑
//! - 复用 polymarket_esports 的 scrape_matches 逻辑
//! - 返回按体育项目分组的比赛数据

use anyhow::Result;
use serde_json::Value;
use tracing::{info, warn};

use crate::config::SportConfig;
use crate::http::build_http_client;
use crate::providers::esports_oddsportal::{self, MatchInfo};
use crate::providers::polymarket_esports;

/// 抓取所有体育项目的比赛数据
///
/// # 参数
///
/// - `sports`: 体育项目配置列表，每个配置包含项目名称、OddsPortal URL 和 Polymarket URL
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回一个 `Result<Vec<(String, Vec<MatchInfo>)>>`，包含每个体育项目的名称和对应的比赛列表。
/// 每个体育项目的比赛数据已从 OddsPortal 和 Polymarket 两个来源合并。
///
/// # 功能
///
/// 1. 遍历每个体育项目配置
/// 2. 对每个项目，分别从 OddsPortal 和 Polymarket 抓取比赛数据
/// 3. 使用 merge_matches 合并两个来源的数据
/// 4. 返回按体育项目分组的结果
pub async fn scrape_all_sports(
    sports: &[SportConfig],
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<Vec<(String, Vec<MatchInfo>)>> {
    let mut results = Vec::new();

    for sport in sports {
        info!("开始抓取体育项目: {} (OddsPortal + Polymarket)", sport.name);

        let oddsportal_matches =
            scrape_oddsportal_for_sport(&sport.oddsportal_url, proxy_enabled, proxy_url).await;
        info!(
            "从 OddsPortal 抓取到 {} 场比赛 [{}]",
            oddsportal_matches.len(),
            sport.name
        );

        let polymarket_matches =
            scrape_polymarket_for_sport(&sport.polymarket_url, proxy_enabled, proxy_url).await;
        info!(
            "从 Polymarket 抓取到 {} 场比赛 [{}]",
            polymarket_matches.len(),
            sport.name
        );

        let mut merged_matches =
            esports_oddsportal::merge_matches(oddsportal_matches, polymarket_matches);
        enrich_end_times_from_polymarket(
            &mut merged_matches,
            &sport.polymarket_url,
            proxy_enabled,
            proxy_url,
        )
        .await;
        info!("合并后共 {} 场比赛 [{}]", merged_matches.len(), sport.name);

        results.push((sport.name.clone(), merged_matches));
    }

    Ok(results)
}

async fn enrich_end_times_from_polymarket(
    matches: &mut [MatchInfo],
    polymarket_url: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) {
    let client = match build_http_client(proxy_enabled, proxy_url) {
        Ok(client) => client,
        Err(error) => {
            warn!("创建 Polymarket API 客户端失败: {}", error);
            return;
        }
    };

    let api_urls = polymarket_schedule_api_urls(polymarket_url);
    for api_url in api_urls {
        let value = match client.get(&api_url).send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<Value>().await {
                    Ok(value) => value,
                    Err(error) => {
                        warn!("解析 Polymarket API 响应失败 [{}]: {}", api_url, error);
                        continue;
                    }
                }
            }
            Ok(response) => {
                warn!(
                    "Polymarket API 返回非成功状态 [{}]: {}",
                    api_url,
                    response.status()
                );
                continue;
            }
            Err(error) => {
                warn!("请求 Polymarket API 失败 [{}]: {}", api_url, error);
                continue;
            }
        };

        for match_info in matches.iter_mut().filter(|item| item.end_time.is_none()) {
            if let Some((end_time, is_finished)) = find_polymarket_end_time(&value, match_info) {
                match_info.end_time = Some(end_time);
                if is_finished {
                    match_info.status = Some("Finished".to_string());
                    match_info.is_finished = true;
                }
            }
        }

        if matches.iter().all(|item| item.end_time.is_some()) {
            break;
        }
    }
}

fn polymarket_schedule_api_urls(polymarket_url: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let segments = url::Url::parse(polymarket_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .map(|segments| segments.map(str::to_string).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    if segments.first().map(String::as_str) == Some("event") {
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

    if segments.first().map(String::as_str) == Some("esports") {
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

    urls
}

fn find_polymarket_end_time(value: &Value, match_info: &MatchInfo) -> Option<(String, bool)> {
    let mut found = None;
    visit_polymarket_candidates(value, match_info, &mut found);
    found
}

fn visit_polymarket_candidates(
    value: &Value,
    match_info: &MatchInfo,
    found: &mut Option<(String, bool)>,
) {
    if found.as_ref().map(|(_, closed)| *closed).unwrap_or(false) {
        return;
    }

    match value {
        Value::Array(items) => {
            for item in items {
                visit_polymarket_candidates(item, match_info, found);
            }
        }
        Value::Object(map) => {
            if is_polymarket_candidate(value) {
                let text = polymarket_candidate_text(value);
                if team_name_in_text(&match_info.team1, &text)
                    && team_name_in_text(&match_info.team2, &text)
                    && let Some(end_time) = polymarket_candidate_end_time(value)
                {
                    let closed = polymarket_candidate_closed(value);
                    if found.is_none() || closed {
                        *found = Some((end_time, closed));
                    }
                }
            }
            for child in map.values() {
                visit_polymarket_candidates(child, match_info, found);
            }
        }
        _ => {}
    }
}

fn is_polymarket_candidate(value: &Value) -> bool {
    value.as_object().is_some_and(|map| {
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
    })
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

/// 从 OddsPortal 抓取单个体育项目的比赛数据
///
/// 复用 esports_oddsportal::scrape_future_matches 的抓取逻辑。
async fn scrape_oddsportal_for_sport(
    url: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Vec<MatchInfo> {
    match esports_oddsportal::scrape_future_matches(url, proxy_enabled, proxy_url).await {
        Ok(matches) => matches,
        Err(e) => {
            warn!("从 OddsPortal 抓取失败: {}", e);
            Vec::new()
        }
    }
}

/// 从 Polymarket 抓取单个体育项目的比赛数据
///
/// 复用 polymarket_esports::scrape_matches 的抓取逻辑。
async fn scrape_polymarket_for_sport(
    url: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Vec<MatchInfo> {
    let result = if let Some(game_name) = esports_game_name_from_url(url) {
        polymarket_esports::scrape_matches_for_game(url, game_name, proxy_enabled, proxy_url).await
    } else {
        polymarket_esports::scrape_matches(url, proxy_enabled, proxy_url).await
    };

    match result {
        Ok(matches) => matches,
        Err(e) => {
            warn!("从 Polymarket 抓取失败: {}", e);
            Vec::new()
        }
    }
}

fn esports_game_name_from_url(url: &str) -> Option<&str> {
    let parsed = url::Url::parse(url).ok()?;
    let mut segments = parsed.path_segments()?;
    if segments.next()? != "esports" {
        return None;
    }
    match segments.next()? {
        "dota-2" => Some("dota-2"),
        "league-of-legends" => Some("league-of-legends"),
        "counter-strike" => Some("counter-strike"),
        _ => None,
    }
}
