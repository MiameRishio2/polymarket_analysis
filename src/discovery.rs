//! 比赛发现模块
//!
//! 此模块提供通过关键词搜索比赛的功能，
//! 支持从 Polymarket 和 OddsPortal 两个数据源发现比赛。

use anyhow::{Result, bail};
use urlencoding::encode;

use crate::config::AppConfig;
use crate::match_resolver::resolve_from_text;

/// 比赛发现结果
///
/// 包含从数据源搜索到的比赛信息。
#[derive(Debug, Clone)]
pub struct MatchDiscovery {
    /// 比赛页面 URL
    pub url: String,
    /// 主队名称
    pub home_team: String,
    /// 客队名称
    pub away_team: String,
    /// 比赛唯一 ID
    pub match_id: String,
    /// 数据来源 ("polymarket" 或 "oddsportal")
    pub source: String,
}

/// 通过关键词搜索比赛
///
/// 此函数优先从 Polymarket 搜索，如果无结果则回退到 OddsPortal。
/// 使用配置的搜索 URL 模板，将 `{query}` 替换为 URL 编码的查询字符串。
/// 搜索返回的结果会通过 `resolve_from_text` 解析主队和客队名称。
///
/// # 参数
///
/// - `config`: 应用程序配置，包含搜索 URL 模板
/// - `query`: 搜索关键词
///
/// # 返回
///
/// 成功时返回 `MatchDiscovery`，包含比赛 URL、主队、客队、match_id 和来源；
/// 如果两个数据源都无结果则返回错误。
pub async fn find_match(config: &AppConfig, query: &str) -> Result<MatchDiscovery> {
    let client = crate::http::build_http_client(config.proxy_enabled, &config.proxy)?;

    let polymarket_results = search_polymarket(&client, &config.discovery.polymarket_search_url, query).await?;
    if !polymarket_results.is_empty() {
        return Ok(polymarket_results.into_iter().next().unwrap());
    }

    let oddsportal_results = search_oddsportal(&client, &config.discovery.oddsportal_search_url, query).await?;
    if !oddsportal_results.is_empty() {
        return Ok(oddsportal_results.into_iter().next().unwrap());
    }

    bail!("no match found for query: {}", query)
}

/// 搜索 Polymarket
///
/// 将查询关键词替换到 Polymarket 搜索 URL 中，
/// 请求页面并解析搜索结果，提取比赛链接和标题。
async fn search_polymarket(
    client: &reqwest::Client,
    url_template: &str,
    query: &str,
) -> Result<Vec<MatchDiscovery>> {
    let search_url = url_template.replace("{query}", &encode(query));
    let resp = client.get(&search_url).send().await?;
    let html = resp.text().await?;

    parse_polymarket_results(&html)
}

/// 解析 Polymarket 搜索结果
///
/// 从 HTML 中提取比赛链接和标题，
/// 使用 `resolve_from_text` 解析对阵双方。
fn parse_polymarket_results(html: &str) -> Result<Vec<MatchDiscovery>> {
    let mut results = Vec::new();

    // 匹配 Polymarket 游戏列表页面中的比赛链接
    let link_re = regex::Regex::new(r#"<a[^>]*href="(/esports/dota-2/[^"]+)"[^>]*>([^<]+)</a>"#)?;
    for cap in link_re.captures_iter(html) {
        let url_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        if let Ok(identity) = resolve_from_text(title) {
            let url = format!("https://polymarket.com{}", url_path);
            results.push(MatchDiscovery {
                url,
                home_team: identity.home_team,
                away_team: identity.away_team,
                match_id: identity.match_id,
                source: "polymarket".to_string(),
            });
        }
    }

    Ok(results)
}

/// 搜索 OddsPortal
///
/// 将查询关键词替换到 OddsPortal 搜索 URL 中，
/// 请求页面并解析搜索结果，提取比赛链接和标题。
async fn search_oddsportal(
    client: &reqwest::Client,
    url_template: &str,
    query: &str,
) -> Result<Vec<MatchDiscovery>> {
    let search_url = url_template.replace("{query}", &encode(query));
    let resp = client.get(&search_url).send().await?;
    let html = resp.text().await?;

    parse_oddsportal_results(&html)
}

/// 解析 OddsPortal 搜索结果
///
/// 从 HTML 中提取比赛链接和标题，
/// 使用 `resolve_from_text` 解析对阵双方。
fn parse_oddsportal_results(html: &str) -> Result<Vec<MatchDiscovery>> {
    let mut results = Vec::new();

    let link_re = regex::Regex::new(r#"<a[^>]*href="(/[^"]+match[^"]+|/football/[^"]+)"[^>]*>([^<]+)</a>"#)?;
    for cap in link_re.captures_iter(html) {
        let url_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let title = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        if let Ok(identity) = resolve_from_text(title) {
            let url = format!("https://www.oddsportal.com{}", url_path);
            results.push(MatchDiscovery {
                url,
                home_team: identity.home_team,
                away_team: identity.away_team,
                match_id: identity.match_id,
                source: "oddsportal".to_string(),
            });
        }
    }

    Ok(results)
}
