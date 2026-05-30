//! 多游戏电竞数据抓取模块
//!
//! 本模块用于从多个电竞游戏（Dota 2、Counter-Strike、League of Legends）的页面抓取比赛信息。
//! 主要功能包括：
//!
//! - 遍历配置中的所有游戏
//! - 从 OddsPortal 和 Polymarket 两个平台抓取每个游戏的比赛数据
//! - 合并数据并保存到对应的目录结构

use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};

use crate::config::EsportGameConfig;
use crate::providers::esports_oddsportal;

/// 抓取所有配置的游戏数据并保存
///
/// # 参数
///
/// - `games`: 游戏配置列表
/// - `output_base`: 输出目录的根路径
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回 `Result<()>`，成功时返回 `Ok(())`，失败时返回错误信息。
pub async fn scrape_all_games(
    games: &[EsportGameConfig],
    output_base: &Path,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<()> {
    for game in games {
        info!("开始抓取 {} 游戏数据", game.name);

        match scrape_single_game(game, output_base, proxy_enabled, proxy_url).await {
            Ok(_) => info!("成功抓取 {} 游戏数据", game.name),
            Err(e) => warn!("抓取 {} 游戏数据失败: {}", game.name, e),
        }
    }

    Ok(())
}

/// 抓取单个游戏的数据并保存
///
/// # 参数
///
/// - `game`: 游戏配置
/// - `output_base`: 输出目录的根路径
/// - `proxy_enabled`: 是否启用代理
/// - `proxy_url`: 代理服务器 URL
///
/// # 返回值
///
/// 返回 `Result<()>`，成功时返回 `Ok(())`，失败时返回错误信息。
async fn scrape_single_game(
    game: &EsportGameConfig,
    output_base: &Path,
    proxy_enabled: bool,
    proxy_url: &str,
) -> Result<()> {
    // 从 OddsPortal 抓取数据
    let oddsportal_matches = esports_oddsportal::scrape_future_matches(
        &game.oddsportal_url,
        proxy_enabled,
        proxy_url,
    ).await?;
    info!("从 OddsPortal ({}) 抓取到 {} 场比赛", game.name, oddsportal_matches.len());

    // 从 Polymarket 抓取数据
    let polymarket_matches = crate::providers::polymarket_esports::scrape_matches_for_game(
        &game.polymarket_url,
        &game.name,
        proxy_enabled,
        proxy_url,
    ).await.unwrap_or_else(|e| {
        warn!("从 Polymarket ({}) 抓取失败: {}", game.name, e);
        Vec::new()
    });
    info!("从 Polymarket ({}) 抓取到 {} 场比赛", game.name, polymarket_matches.len());

    // 合并两个平台的数据
    let merged_matches = esports_oddsportal::merge_matches(oddsportal_matches, polymarket_matches);

    // 构建输出路径：esport/{game_name}/
    let game_dir = output_base.join(&game.name);
    std::fs::create_dir_all(&game_dir)?;

    // 从 URL 中提取赛事名称作为文件名
    let url_path = url::Url::parse(&game.oddsportal_url)?;
    let path_segments: Vec<&str> = url_path
        .path_segments()
        .map(|s| s.filter(|seg| !seg.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();

    let final_segment = path_segments.last().unwrap_or(&"matches").to_string();
    let file_name = format!("{}.json", final_segment);
    let file_path = game_dir.join(&file_name);

    let json_content = serde_json::to_string_pretty(&merged_matches)?;
    std::fs::write(&file_path, json_content)?;

    info!("{} 比赛数据已保存到: {} (共 {} 场)", game.name, file_path.display(), merged_matches.len());

    Ok(())
}
