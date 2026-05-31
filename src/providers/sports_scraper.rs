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
use tracing::{info, warn};

use crate::config::SportConfig;
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

        let merged_matches =
            esports_oddsportal::merge_matches(oddsportal_matches, polymarket_matches);
        info!("合并后共 {} 场比赛 [{}]", merged_matches.len(), sport.name);

        results.push((sport.name.clone(), merged_matches));
    }

    Ok(results)
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
