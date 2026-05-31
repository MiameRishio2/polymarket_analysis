//! Polymarket 电竞（Dota 2）赛事数据模块
//!
//! 本模块用于从 Polymarket 的 Dota 2 赛事页面抓取比赛信息。
//! 由于 Polymarket 使用 React 动态渲染，我们通过 API 和页面结构来提取数据。

use anyhow::Result;
use regex::Regex;
use reqwest::header::USER_AGENT;

use crate::http::build_http_client;
use crate::providers::esports_oddsportal::MatchInfo;

/// 请求 Polymarket 时使用的 User-Agent 标识字符串
const POLYMARKET_USER_AGENT: &str = "polymarket-analysis/0.1";

/// 从指定的 Polymarket 页面抓取比赛信息
///
/// # 参数
///
/// - `url`: Polymarket 赛事页面的完整 URL
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回一个 `Result<Vec<MatchInfo>>`，包含所有比赛的队伍名称和 Polymarket URL。
pub async fn scrape_matches(
    url: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<Vec<MatchInfo>> {
    let client = build_http_client(proxy_enabled, proxy_url).expect("failed to build HTTP client");

    let response = client
        .get(url)
        .header(USER_AGENT, POLYMARKET_USER_AGENT)
        .send()
        .await?;

    let body = response.text().await?;

    parse_matches(&body)
}

/// 从指定的 Polymarket 页面抓取特定游戏的比赛信息
///
/// # 参数
///
/// - `url`: Polymarket 赛事页面的完整 URL
/// - `game_name`: 游戏名称（如 "dota-2", "counter-strike", "league-of-legends"）
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回一个 `Result<Vec<MatchInfo>>`，包含所有比赛的队伍名称和 Polymarket URL。
pub async fn scrape_matches_for_game(
    url: &str,
    game_name: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<Vec<MatchInfo>> {
    let client = build_http_client(proxy_enabled, proxy_url).expect("failed to build HTTP client");

    let response = client
        .get(url)
        .header(USER_AGENT, POLYMARKET_USER_AGENT)
        .send()
        .await?;

    let body = response.text().await?;

    parse_matches_for_game(&body, game_name)
}

/// 从 HTML 内容中解析比赛信息
///
/// 由于 Polymarket 使用 React 动态渲染，我们从页面中的团队 logo 和链接中提取信息。
fn parse_matches(html: &str) -> Result<Vec<MatchInfo>> {
    parse_matches_for_game(html, "dota-2")
}

/// 从 HTML 内容中解析特定游戏的比赛信息
fn parse_matches_for_game(html: &str, game_name: &str) -> Result<Vec<MatchInfo>> {
    // 只使用页面中真实出现过的链接。旧逻辑会从队伍 logo 推断组合并拼接 URL，
    // 这会生成不可访问的 Polymarket 地址，不适合作为网页最后一层的外链。
    parse_from_links(html, game_name)
}

/// 从页面链接中解析比赛信息
fn parse_from_links(html: &str, game_name: &str) -> Result<Vec<MatchInfo>> {
    let mut matches = Vec::new();

    // 查找 /esports/{game_name}/ 开头的链接
    let link_re = Regex::new(&format!(
        r#"href="(/esports/{}/[^/]+/[^"]+)"#,
        regex::escape(game_name)
    ))?;

    for cap in link_re.captures_iter(html) {
        let url_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");

        // 从 URL 中提取队伍信息
        if let Some(parsed) = parse_polymarket_url(url_path, game_name) {
            matches.push(parsed);
        }
    }

    // 去重
    matches.sort_by(|a, b| a.team1.cmp(&b.team1).then(a.team2.cmp(&b.team2)));
    matches.dedup_by(|a, b| a.team1 == b.team1 && a.team2 == b.team2);

    Ok(matches)
}

/// 解析 Polymarket URL，提取队伍名称和 URL
fn parse_polymarket_url(url_path: &str, game_name: &str) -> Option<MatchInfo> {
    // URL 格式：/esports/{game_name}/tournament/slug
    // 例如：/esports/dota-2/blast-slam/dota2-aur1-tundra-2026-05-30

    let parts: Vec<&str> = url_path.split('/').collect();
    if parts.len() < 5 {
        return None;
    }

    // 最后一部分是比赛 slug
    let slug = parts.last()?;

    // 支持多种游戏前缀格式
    let possible_prefixes = vec![
        format!("{}-", game_name.replace('-', "")),
        format!("{}-", game_name),
        // 对于 Counter Strike，还支持 cs2- 前缀
        if game_name == "counter-strike" {
            "cs2-".to_string()
        } else if game_name == "league-of-legends" {
            "lol-".to_string()
        } else {
            String::new()
        },
    ];

    let mut slug_without_prefix = None;
    for prefix in &possible_prefixes {
        if !prefix.is_empty() && slug.starts_with(prefix) {
            slug_without_prefix = Some(&slug[prefix.len()..]);
            break;
        }
    }

    // 如果没有找到匹配的前缀，尝试不检查前缀直接处理
    let slug_without_prefix = match slug_without_prefix {
        Some(s) => s,
        None => &slug,
    };

    // 移除日期部分（最后的 YYYY-MM-DD 或类似格式）
    let date_re = Regex::new(r"-\d{4}-\d{2}-\d{2}$").unwrap();
    let slug_without_date = date_re.replace(slug_without_prefix, "").to_string();

    // 分割队伍名称（用 - 分隔）
    let teams: Vec<&str> = slug_without_date.split('-').collect();
    if teams.len() < 2 {
        return None;
    }

    // 队伍名称可能是多部分，需要智能分割
    let (team1_slug, team2_slug) = split_teams(&teams)?;

    let team1 = decode_team_slug(&team1_slug, game_name);
    let team2 = decode_team_slug(&team2_slug, game_name);

    let polymarket_url = format!("https://polymarket.com{}", url_path);

    Some(MatchInfo {
        team1,
        team2,
        match_time: String::new(),
        status: None,
        is_finished: false,
        score: None,
        partial_score: None,
        polymarket_url: Some(polymarket_url),
        oddsportal_url: None,
    })
}

/// 将 slug 数组分割为两个队伍
fn split_teams(teams: &[&str]) -> Option<(String, String)> {
    if teams.len() == 2 {
        Some((teams[0].to_string(), teams[1].to_string()))
    } else if teams.len() > 2 {
        // 尝试找到分割点
        // 常见模式：team1-team2-date，其中 team 可能是多部分
        // 我们假设最后一个是 team2，其余是 team1
        let mid = teams.len() - 1;
        let team1 = teams[..mid].join("-");
        let team2 = teams[mid].to_string();
        Some((team1, team2))
    } else {
        None
    }
}

/// 将队伍 slug 解码为可读名称
fn decode_team_slug(slug: &str, _game_name: &str) -> String {
    match slug {
        "aur1" | "aur" | "aurora" => "Aurora".to_string(),
        "bb4" | "bb" | "betboom" => "BetBoom Team".to_string(),
        "flc" | "falcons" => "Falcons".to_string(),
        "ty" | "yandex" | "team-yandex" => "Team Yandex".to_string(),
        "lgd" => "LGD Gaming".to_string(),
        "pipsqu" | "ex-pipsqueak" => "ex-Pipsqueak+4".to_string(),
        "fla" | "flame" => "Flame Team".to_string(),
        "tundra" | "tun" => "Tundra Esports".to_string(),
        "ts8" | "ts" | "spirit" => "Team Spirit".to_string(),
        "og" => "OG".to_string(),
        "nemiga" | "g" => "Nemiga Gaming".to_string(),
        "ramzes" => "Ramzes Club".to_string(),
        "tpabom" | "tpabomah" => "Tpabomah Club".to_string(),
        "nem" | "nemesis" => "Team Nemesis".to_string(),
        "yangon" | "yg" => "Yangon Galacticos".to_string(),
        "yellow" => "Yellow Submarine".to_string(),
        "maru" | "amaru" => "Amaru Gaming".to_string(),
        "krd" => "KRD".to_string(),
        "balu" => "Balu".to_string(),
        "giordo" => "Giordo".to_string(),
        "tdk" => "TDK".to_string(),
        "newgro" | "new" => "New Generation".to_string(),
        "unknow" | "unk" => "Unknown".to_string(),
        "f9team" | "f9" => "F9 Team".to_string(),
        "yes" | "yes-tm6" => "Yes Team".to_string(),
        "mental" => "Mental".to_string(),
        _ => {
            // 默认：将 slug 转换为可读格式
            slug.replace('-', " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}
