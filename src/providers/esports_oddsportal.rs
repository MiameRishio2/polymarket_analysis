//! OddsPortal 电竞（Dota 2）赛事数据模块
//!
//! 本模块用于从 OddsPortal 的 Dota 2 赛事页面抓取未来比赛信息。主要功能包括：
//!
//! - 发送 HTTP 请求获取 OddsPortal 电竞页面内容
//! - 解析 HTML 提取未来比赛的队伍名称和比赛时间
//! - 提供 [`scrape_future_matches`] 函数用于外部调用

use anyhow::Result;
use regex::Regex;
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};
use serde::Serialize;
use std::path::Path;
use tracing::info;

use crate::http::build_http_client;

/// 请求 OddsPortal 时使用的 User-Agent 标识字符串
const ODDSPORTAL_USER_AGENT: &str = "polymarket-analysis/0.1";

/// 比赛信息结构体
///
/// 包含两支参赛队伍的名称、比赛时间以及各平台的 URL。
#[derive(Clone, Debug, Serialize)]
pub struct MatchInfo {
    /// 第一支队伍名称
    pub team1: String,
    /// 第二支队伍名称
    pub team2: String,
    /// 比赛时间（UTC ISO 8601 格式）
    pub match_time: String,
    /// OddsPortal 比赛状态，例如 Scheduled、Finished
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// 是否已经结束
    pub is_finished: bool,
    /// 主比分，例如 2:1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<String>,
    /// 分盘、分节等细分比分
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_score: Option<String>,
    /// Polymarket 比赛页面 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polymarket_url: Option<String>,
    /// OddsPortal 比赛页面 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oddsportal_url: Option<String>,
}

/// 从指定的 OddsPortal 页面抓取未来比赛信息
///
/// # 参数
///
/// - `url`: OddsPortal 赛事页面的完整 URL
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回一个 `Result<Vec<MatchInfo>>`，包含所有未来比赛的队伍名称和时间。
///
/// # 解析策略
///
/// 1. 发送带自定义 User-Agent 的 HTTP GET 请求
/// 2. 解析 HTML 文档，查找包含比赛信息的元素
/// 3. 提取队伍名称（从链接文本或属性）
/// 4. 提取比赛时间（从时间属性或文本）
/// 5. 过滤掉已开始或已结束的比赛
pub async fn scrape_future_matches(
    url: &str,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<Vec<MatchInfo>> {
    let client = build_http_client(proxy_enabled, proxy_url).expect("failed to build HTTP client");

    let response = client
        .get(url)
        .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
        .send()
        .await?;

    let body = response.text().await?;

    parse_future_matches(&body)
}

/// 从 HTML 内容中解析未来比赛信息
///
/// # 参数
///
/// - `html`: OddsPortal 页面的完整 HTML 内容
///
/// # 返回值
///
/// 返回解析到的未来比赛列表。
fn parse_future_matches(html: &str) -> Result<Vec<MatchInfo>> {
    let document = Html::parse_document(html);
    let mut matches = Vec::new();

    if let Some(parsed) = parse_from_json_data(html)? {
        matches.extend(parsed);
    }

    if matches.is_empty() {
        if let Ok(selector) = Selector::parse("table.t-table tr, div.table-container tr, tr[id]") {
            for row in document.select(&selector) {
                if let Some(info) = extract_match_from_row(&row) {
                    matches.push(info);
                }
            }
        }
    }

    if matches.is_empty() {
        if let Ok(selector) = Selector::parse(".table-header-link, a[href*=\"/matches/\"]") {
            for link in document.select(&selector) {
                if let Some(info) = extract_match_from_link(&link, &document) {
                    matches.push(info);
                }
            }
        }
    }

    if matches.is_empty() {
        if let Ok(selector) = Selector::parse("[data-start-time], [class*='event']") {
            for element in document.select(&selector) {
                if let Some(info) = extract_match_from_event(&element) {
                    matches.push(info);
                }
            }
        }
    }

    Ok(matches)
}

fn parse_from_json_data(html: &str) -> Result<Option<Vec<MatchInfo>>> {
    let mut matches = Vec::new();

    // 查找包含比赛数据的 JSON 块（嵌入在页面中）
    // 数据格式包含 "home-name", "away-name", "url" (h2h 链接), "date-start-timestamp" 等
    // 注意：HTML 中使用 &quot; 表示引号
    let json_pattern = Regex::new(r#"home-name&quot;:&quot;([^&]+)&quot;"#)?;

    for cap in json_pattern.captures_iter(html) {
        let home_name = cap
            .get(1)
            .map(|m| clean_label_string(&decode_jsonish(m.as_str())))
            .unwrap_or_default();

        // 查找相邻的 away-name
        let start_pos = cap.get(0).map(|m| m.end()).unwrap_or(0);
        let remaining = &html[start_pos..];

        let away_re = Regex::new(r#"away-name&quot;:&quot;([^&]+)&quot;"#)?;
        if let Some(away_cap) = away_re.captures(remaining) {
            let away_name = away_cap
                .get(1)
                .map(|m| clean_label_string(&decode_jsonish(m.as_str())))
                .unwrap_or_default();

            // 查找 url 字段
            let url_re = Regex::new(r#"url&quot;:&quot;([^&]+)&quot;"#)?;
            let oddsportal_url = url_re
                .captures(remaining)
                .and_then(|c| c.get(1).map(|m| m.as_str()))
                .map(|url| decode_jsonish(url).replace('"', ""))
                .map(|url| format!("https://www.oddsportal.com{}", url.trim_end_matches('/')));

            // 查找时间戳（格式：date-start-timestamp":1780142400，数字没有引号）
            let timestamp_re = Regex::new(r#"date-start-timestamp&quot;:(\d+)"#)?;
            let match_time = timestamp_re
                .captures(remaining)
                .and_then(|c| c.get(1).map(|m| m.as_str()))
                .and_then(|ts| ts.parse::<i64>().ok())
                .map(|ts| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(ts, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let status = capture_jsonish_string(remaining, "event-stage-name")?;
            let status_id = capture_jsonish_number(remaining, "status-id")?;
            let is_finished = status
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("finished"))
                .unwrap_or(false)
                || status_id.as_deref() == Some("3");
            let score = capture_jsonish_string(remaining, "result")?
                .filter(|value| !value.is_empty())
                .or_else(|| {
                    let home = capture_jsonish_string(remaining, "homeResult")
                        .ok()
                        .flatten();
                    let away = capture_jsonish_string(remaining, "awayResult")
                        .ok()
                        .flatten();
                    match (home, away) {
                        (Some(home), Some(away)) if !home.is_empty() && !away.is_empty() => {
                            Some(format!("{}:{}", home, away))
                        }
                        _ => None,
                    }
                });
            let partial_score = capture_jsonish_string(remaining, "partialresult")?
                .filter(|value| !value.is_empty());

            if !home_name.is_empty() && !away_name.is_empty() {
                matches.push(MatchInfo {
                    team1: home_name,
                    team2: away_name,
                    match_time,
                    status,
                    is_finished,
                    score,
                    partial_score,
                    polymarket_url: None,
                    oddsportal_url,
                });
            }
        }
    }

    if matches.is_empty() {
        Ok(None)
    } else {
        // 去重
        matches.sort_by(|a, b| a.team1.cmp(&b.team1).then(a.team2.cmp(&b.team2)));
        matches.dedup_by(|a, b| a.team1 == b.team1 && a.team2 == b.team2);
        Ok(Some(matches))
    }
}

fn capture_jsonish_string(value: &str, key: &str) -> Result<Option<String>> {
    let pattern = format!(r#"{}&quot;:&quot;([^&]*)&quot;"#, regex::escape(key));
    let re = Regex::new(&pattern)?;
    Ok(re.captures(value).and_then(|cap| {
        cap.get(1)
            .map(|m| clean_label_string(&decode_jsonish(m.as_str())))
    }))
}

fn capture_jsonish_number(value: &str, key: &str) -> Result<Option<String>> {
    let pattern = format!(r#"{}&quot;:(\d+)"#, regex::escape(key));
    let re = Regex::new(&pattern)?;
    Ok(re
        .captures(value)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string())))
}

fn decode_jsonish(value: &str) -> String {
    value
        .replace(r#"\/"#, "/")
        .replace(r#"\""#, "\"")
        .replace(r#"\n"#, " ")
        .replace(r#"\t"#, " ")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&rsquo;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn clean_label_string(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 从表格行元素中提取比赛信息
fn extract_match_from_row(row: &scraper::ElementRef) -> Option<MatchInfo> {
    let links: Vec<_> = row
        .select(&Selector::parse("a").ok()?)
        .filter(|a| {
            let href = a.value().attr("href").unwrap_or("");
            href.contains("/team/") || href.contains("/match/")
        })
        .collect();

    if links.len() < 2 {
        return None;
    }

    let team1 = clean_label(links[0].text().collect::<Vec<_>>().join(" "));
    let team2 = clean_label(links[1].text().collect::<Vec<_>>().join(" "));

    if team1.is_empty() || team2.is_empty() {
        return None;
    }

    let match_time = extract_time_from_row(row);

    Some(MatchInfo {
        team1,
        team2,
        match_time,
        status: None,
        is_finished: false,
        score: None,
        partial_score: None,
        polymarket_url: None,
        oddsportal_url: None,
    })
}

/// 从链接元素中提取比赛信息（配合父元素查找）
fn extract_match_from_link(link: &scraper::ElementRef, _document: &Html) -> Option<MatchInfo> {
    let team1 = clean_label(link.text().collect::<Vec<_>>().join(" "));
    if team1.is_empty() {
        return None;
    }

    // 查找相邻的链接作为第二支队伍
    let parent = link.parent()?;
    let parent_ref = scraper::ElementRef::wrap(parent)?;
    let sibling_links: Vec<_> = parent_ref
        .select(&Selector::parse("a").ok()?)
        .filter(|a| {
            let href = a.value().attr("href").unwrap_or("");
            href.contains("/team/") || href.contains("/match/")
        })
        .collect();

    if sibling_links.len() < 2 {
        return None;
    }

    let team2 = clean_label(sibling_links[1].text().collect::<Vec<_>>().join(" "));
    if team2.is_empty() {
        return None;
    }

    let match_time = extract_time_from_element(&parent_ref);

    Some(MatchInfo {
        team1,
        team2,
        match_time,
        status: None,
        is_finished: false,
        score: None,
        partial_score: None,
        polymarket_url: None,
        oddsportal_url: None,
    })
}

/// 从事件元素中提取比赛信息
fn extract_match_from_event(element: &scraper::ElementRef) -> Option<MatchInfo> {
    let links: Vec<_> = element
        .select(&Selector::parse("a").ok()?)
        .filter(|a| {
            let href = a.value().attr("href").unwrap_or("");
            href.contains("/team/") || href.contains("/match/")
        })
        .collect();

    if links.len() < 2 {
        return None;
    }

    let team1 = clean_label(links[0].text().collect::<Vec<_>>().join(" "));
    let team2 = clean_label(links[1].text().collect::<Vec<_>>().join(" "));

    if team1.is_empty() || team2.is_empty() {
        return None;
    }

    let match_time = extract_time_from_element(element);

    Some(MatchInfo {
        team1,
        team2,
        match_time,
        status: None,
        is_finished: false,
        score: None,
        partial_score: None,
        polymarket_url: None,
        oddsportal_url: None,
    })
}

/// 从表格行中提取时间信息
fn extract_time_from_row(row: &scraper::ElementRef) -> String {
    // 尝试从时间元素提取
    if let Ok(selector) = Selector::parse("time, [class*='time'], [class*='date'], span.t-time") {
        if let Some(time_elem) = row.select(&selector).next() {
            if let Some(datetime) = time_elem.value().attr("datetime") {
                if !datetime.is_empty() {
                    return datetime.to_string();
                }
            }
            let text = clean_label(time_elem.text().collect::<Vec<_>>().join(" "));
            if !text.is_empty() {
                return text;
            }
        }
    }

    // 尝试从 data 属性提取
    if let Some(start_time) = row.value().attr("data-start-time") {
        if !start_time.is_empty() {
            return start_time.to_string();
        }
    }

    // 尝试从任何 td 元素中查找时间格式
    if let Ok(selector) = Selector::parse("td") {
        for cell in row.select(&selector) {
            let text = clean_label(cell.text().collect::<Vec<_>>().join(" "));
            if is_time_like(&text) {
                return text;
            }
        }
    }

    String::new()
}

/// 从通用元素中提取时间信息
fn extract_time_from_element(element: &scraper::ElementRef) -> String {
    // 尝试从 data 属性提取
    if let Some(start_time) = element.value().attr("data-start-time") {
        if !start_time.is_empty() {
            return start_time.to_string();
        }
    }

    // 尝试从时间元素提取
    if let Ok(selector) = Selector::parse("time, [class*='time'], [class*='date']") {
        if let Some(time_elem) = element.select(&selector).next() {
            if let Some(datetime) = time_elem.value().attr("datetime") {
                if !datetime.is_empty() {
                    return datetime.to_string();
                }
            }
            let text = clean_label(time_elem.text().collect::<Vec<_>>().join(" "));
            if !text.is_empty() {
                return text;
            }
        }
    }

    String::new()
}

/// 判断文本是否像时间格式
fn is_time_like(text: &str) -> bool {
    // 匹配常见时间格式：HH:MM、YYYY-MM-DD HH:MM、月 日, 年 等
    let patterns = [
        r"^\d{1,2}:\d{2}$",
        r"^\d{4}-\d{2}-\d{2}",
        r"^[A-Z][a-z]{2}\s+\d{1,2}",
        r"^\d{1,2}\s+[A-Z][a-z]{2}",
    ];

    patterns.iter().any(|pattern| {
        regex::Regex::new(pattern)
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    })
}

/// 清理文本标签，将连续空白字符规范化为单个空格
fn clean_label(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 抓取比赛数据并保存到 JSON 文件
///
/// # 参数
///
/// - `url`: OddsPortal 赛事页面的完整 URL
/// - `output_base`: 输出目录的根路径
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回 `Result<()>`，成功时返回 `Ok(())`，失败时返回错误信息。
///
/// # 功能
///
/// 1. 从 OddsPortal 和 Polymarket 两个平台抓取比赛数据
/// 2. 合并两个平台的数据，匹配相同的比赛
/// 3. 从 URL 路径中提取目录结构
/// 4. 创建对应的目录结构
/// 5. 将比赛数据以 JSON 格式保存到文件
/// 6. 打印日志消息说明保存位置
pub async fn save_matches(
    url: &str,
    output_base: &Path,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<()> {
    // 从 OddsPortal 抓取数据
    let oddsportal_matches = scrape_future_matches(url, proxy_enabled, proxy_url).await?;
    info!("从 OddsPortal 抓取到 {} 场比赛", oddsportal_matches.len());

    // 从 Polymarket 抓取数据
    let polymarket_url = "https://polymarket.com/esports/dota-2/games";
    let polymarket_matches = crate::providers::polymarket_esports::scrape_matches(
        polymarket_url,
        proxy_enabled,
        proxy_url,
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("从 Polymarket 抓取失败: {}", e);
        Vec::new()
    });
    info!("从 Polymarket 抓取到 {} 场比赛", polymarket_matches.len());

    // 合并两个平台的数据
    let merged_matches = merge_matches(oddsportal_matches, polymarket_matches);

    let url_path = url::Url::parse(url)?;
    let path_segments: Vec<&str> = url_path
        .path_segments()
        .map(|s| s.filter(|seg| !seg.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();

    // 构建目录结构：esport/ + URL路径中除第一个外的所有段
    // 例如：/esports/dota-2/dota-2-blast-slam-vii/ -> esport/dota-2/dota-2-blast-slam-vii/
    let dir_segments: Vec<String> = {
        let mut parts = Vec::new();
        parts.push("esport".to_string());
        // 跳过第一个段（如 "esports"），保留后面的所有段
        for seg in &path_segments[1..] {
            parts.push(seg.to_string());
        }
        parts
    };

    let final_segment = path_segments.last().unwrap_or(&"matches").to_string();

    let mut dir_path = output_base.to_path_buf();
    for segment in &dir_segments {
        dir_path = dir_path.join(segment);
    }

    std::fs::create_dir_all(&dir_path)?;

    let file_name = format!("{}.json", final_segment);
    let file_path = dir_path.join(&file_name);

    let json_content = serde_json::to_string_pretty(&merged_matches)?;
    std::fs::write(&file_path, json_content)?;

    info!(
        "比赛数据已保存到: {} (共 {} 场)",
        file_path.display(),
        merged_matches.len()
    );

    Ok(())
}

/// 合并两个平台的比赛数据
///
/// 根据队伍名称匹配相同的比赛，将两个平台的 URL 合并到同一个 MatchInfo 中。
pub fn merge_matches(
    mut oddsportal_matches: Vec<MatchInfo>,
    polymarket_matches: Vec<MatchInfo>,
) -> Vec<MatchInfo> {
    let mut merged = Vec::new();

    // 遍历 OddsPortal 的比赛
    for op_match in &mut oddsportal_matches {
        // 尝试在 Polymarket 中找到匹配的比赛
        let pm_match = polymarket_matches
            .iter()
            .find(|pm| teams_match(&op_match.team1, &op_match.team2, &pm.team1, &pm.team2));

        let mut merged_match = op_match.clone();
        if let Some(pm) = pm_match {
            merged_match.polymarket_url = pm.polymarket_url.clone();
        }

        merged.push(merged_match);
    }

    // 添加 Polymarket 中独有的比赛
    for pm_match in &polymarket_matches {
        let found = oddsportal_matches
            .iter()
            .any(|op| teams_match(&op.team1, &op.team2, &pm_match.team1, &pm_match.team2));

        if !found {
            merged.push(pm_match.clone());
        }
    }

    merged
}

/// 判断两支队伍名称是否匹配
///
/// 忽略大小写和空白字符差异，比较两支队伍是否相同。
fn teams_match(op_team1: &str, op_team2: &str, pm_team1: &str, pm_team2: &str) -> bool {
    (single_team_match(op_team1, pm_team1) && single_team_match(op_team2, pm_team2))
        || (single_team_match(op_team1, pm_team2) && single_team_match(op_team2, pm_team1))
}

fn single_team_match(left: &str, right: &str) -> bool {
    let left = normalize_team_name(left);
    let right = normalize_team_name(right);
    if left == right {
        return true;
    }

    let left_compact = compact_team_name(&left);
    let right_compact = compact_team_name(&right);
    if !left_compact.is_empty() && left_compact == right_compact {
        return true;
    }

    let left_tokens = significant_team_tokens(&left);
    let right_tokens = significant_team_tokens(&right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }

    left_tokens.iter().any(|token| right_tokens.contains(token))
}

fn normalize_team_name(value: &str) -> String {
    let lowered = value.to_lowercase().replace('&', " and ");
    let mut normalized = String::with_capacity(lowered.len());
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
        } else {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact_team_name(value: &str) -> String {
    significant_team_tokens(value).join("")
}

fn significant_team_tokens(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|token| !matches!(*token, "team" | "esports" | "gaming" | "club" | "lol"))
        .map(|token| token.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_future_matches, teams_match};

    #[test]
    fn parses_finished_status_and_score_from_tournament_data() {
        let html = r#"
            <tournament-component :sport-data="{&quot;d&quot;:{&quot;rows&quot;:[{
                &quot;home-name&quot;:&quot;Zhang T.&quot;,
                &quot;away-name&quot;:&quot;Chen Y. C.&quot;,
                &quot;status-id&quot;:3,
                &quot;event-stage-name&quot;:&quot;Finished&quot;,
                &quot;url&quot;:&quot;\/tennis\/h2h\/chen-yan-cheng-txqrVwV8\/zhang-tianhui-EuCdP7ft\/#nJCHr9bd&quot;,
                &quot;date-start-timestamp&quot;:1780192800,
                &quot;homeResult&quot;:&quot;2&quot;,
                &quot;awayResult&quot;:&quot;1&quot;,
                &quot;result&quot;:&quot;2:1&quot;,
                &quot;partialresult&quot;:&quot;4:6, 7:5, 6:4&quot;
            }]}}"></tournament-component>
        "#;

        let matches = parse_future_matches(html).expect("parse tournament data");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].team1, "Zhang T.");
        assert_eq!(matches[0].team2, "Chen Y. C.");
        assert!(matches[0].is_finished);
        assert_eq!(matches[0].status.as_deref(), Some("Finished"));
        assert_eq!(matches[0].score.as_deref(), Some("2:1"));
        assert_eq!(matches[0].partial_score.as_deref(), Some("4:6, 7:5, 6:4"));
    }

    #[test]
    fn fuzzy_team_match_handles_league_of_legends_names() {
        assert!(teams_match("Vitality", "GIANTX", "Team Vitality", "Giantx"));
        assert!(teams_match("GIANTX", "Vitality", "Team Vitality", "Giantx"));
    }
}
