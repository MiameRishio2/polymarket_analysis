//! 命令行接口模块
//!
//! 此模块定义了 `polymarket-analysis` CLI 应用程序的命令行接口，
//! 包括命令解析、参数验证和命令分发。
//!
//! 支持以下子命令：
//! - `collect`: 从 Polymarket 和赔率网站采集比赛数据
//! - `export`: 将已采集的比赛数据导出为指定格式
//! - `scrape-esport`: 从 OddsPortal 抓取电竞比赛数据并保存到文件

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// 命令行接口主结构体
///
/// 作为应用程序的入口点，负责解析命令行参数并分发到对应的子命令。
#[derive(Debug, Parser)]
#[command(name = "polymarket-analysis")]
#[command(about = "Collect Polymarket and odds-site match data")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// 支持的子命令枚举
///
/// 定义了 CLI 支持的所有子命令类型：
/// - `Collect`: 启动数据采集流程
/// - `Export`: 导出数据到指定格式
/// - `ScrapeEsport`: 从 OddsPortal 抓取电竞比赛数据并保存到文件
/// - `ScrapeEsportGames`: 从多个电竞游戏页面抓取比赛数据
/// - `FindMatch`: 通过关键词搜索匹配比赛
/// - `ScrapeSports`: 抓取多体育项目数据并启动网页可视化
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 采集 Polymarket 和赔率网站的比赛数据
    Collect,
    /// 将指定比赛的数据导出为 JSONL 或 CSV 格式
    Export {
        /// Override SQLite database path
        #[arg(long)]
        db: Option<PathBuf>,
        /// Override exported match id
        #[arg(long)]
        match_id: Option<String>,
    },
    /// 从 OddsPortal 抓取电竞比赛数据并保存到文件
    ScrapeEsport,
    /// 从多个电竞游戏页面抓取比赛数据
    ScrapeEsportGames,
    /// 通过关键词搜索匹配比赛
    FindMatch {
        /// 搜索关键词
        #[arg(long)]
        query: String,
    },
    /// 抓取多体育项目数据并启动网页可视化
    ScrapeSports,
    /// 启动网页可视化服务，使用已采集的数据
    ServeWeb,
}

/// 运行 CLI 应用程序的主入口函数
///
/// 此函数负责解析命令行参数，并根据子命令类型
/// 分发到对应的处理函数：
///
/// # 处理流程
///
/// 1. 解析命令行参数为 [`Cli`] 结构体
/// 2. 加载 `config.yaml` 配置
/// 3. 根据子命令类型进行匹配：
///    - `Collect`: 调用 `crate::collector::collect()`
///    - `Export`: 调用 `crate::storage::export_match()`
///
/// # 返回值
///
/// 返回 `Result<()>`，成功时返回 `Ok(())`，失败时返回相应的错误信息。
pub async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = crate::config::AppConfig::from_current_dir()?;
    match cli.command {
        Command::Collect => crate::collector::collect(config).await,
        Command::Export { db, match_id } => {
            let mut config = config;
            if let Some(db) = db {
                config.db = db;
            }
            if let Some(match_id) = match_id {
                config.export.match_id = match_id;
            }
            crate::storage::export_match(config).await
        }
        Command::ScrapeEsport => {
            crate::providers::esports_oddsportal::save_matches(
                &config.scrape_esport.url,
                &config.scrape_esport.output,
                config.proxy_enabled,
                &config.proxy,
            )
            .await
        }
        Command::ScrapeEsportGames => {
            crate::providers::esports_multi_game::scrape_all_games(
                &config.scrape_esport_games,
                &config.scrape_esport.output,
                config.proxy_enabled,
                &config.proxy,
            )
            .await
        }
        Command::FindMatch { query } => {
            let match_info = crate::discovery::find_match(&config, &query).await?;
            tracing::info!(
                "found match: {} vs {} ({})",
                match_info.home_team,
                match_info.away_team,
                match_info.url
            );
            Ok(())
        }
        Command::ScrapeSports => {
            let scraped = crate::providers::sports_scraper::scrape_all_sports(
                &config.scrape_sports.sports,
                config.proxy_enabled,
                &config.proxy,
            )
            .await?;

            let entries: Vec<(String, Vec<crate::web::MatchInfo>)> = scraped
                .into_iter()
                .map(|(sport_name, matches)| {
                    (
                        sport_name,
                        matches
                            .into_iter()
                            .map(|m| crate::web::MatchInfo {
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
                            .collect(),
                    )
                })
                .collect();
            let web_data =
                crate::web::group_matches_by_config(&config.scrape_sports.sports, entries);

            crate::web::serve_matches(web_data, config.web.port).await
        }
        Command::ServeWeb => {
            let port = config.web.port;
            crate::web::serve_matches_config(config, port).await
        }
    }
}
