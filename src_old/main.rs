//! 应用程序入口点
//!
//! 本文件是 polymarket_analysis 项目的可执行入口，负责启动 CLI 命令行界面。

use anyhow::Result;

/// 应用程序主函数
///
/// 该函数执行以下操作：
/// 1. 调用 `cli::run` 启动命令行交互界面
///
/// # 返回值
///
/// 返回 `Result<()>`，若执行成功则返回 `Ok(())`，否则返回错误信息。
#[tokio::main]
async fn main() -> Result<()> {
    polymarket_analysis::cli::run().await
}
