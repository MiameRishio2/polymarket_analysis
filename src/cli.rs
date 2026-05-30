//! 命令行接口模块
//!
//! 此模块定义了 `polymarket-analysis` CLI 应用程序的命令行接口，
//! 包括命令解析、参数验证和命令分发。
//!
//! 支持以下子命令：
//! - `collect`: 从 Polymarket 和赔率网站采集比赛数据
//! - `export`: 将已采集的比赛数据导出为指定格式

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
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 采集 Polymarket 和赔率网站的比赛数据
    Collect,
    /// 将指定比赛的数据导出为 JSONL 或 CSV 格式
    Export,
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
    let cli = Cli::parse();
    let config = crate::config::AppConfig::from_current_dir()?;
    match cli.command {
        Command::Collect => crate::collector::collect(config).await,
        Command::Export => crate::storage::export_match(config).await,
    }
}
