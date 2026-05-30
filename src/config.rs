//! 配置加载模块
//!
//! 此模块负责从 `config.yaml` 文件加载应用程序配置，
//! 包括代理地址、Polymarket 和 OddsPortal 数据源配置。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// 数据导出格式枚举
#[derive(Debug, Deserialize, Clone)]
pub enum ExportFormat {
    #[serde(rename = "jsonl")]
    Jsonl,
    #[serde(rename = "csv")]
    Csv,
}

/// 应用程序主配置结构体
///
/// 包含代理地址、数据库路径和各个数据源的配置。
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// 是否启用 HTTP 代理
    pub proxy_enabled: bool,
    /// HTTP 代理地址
    pub proxy: String,
    /// SQLite 数据库文件路径
    pub db: PathBuf,
    /// Polymarket 数据源配置
    pub polymarket: PolymarketConfig,
    /// OddsPortal 数据源配置
    pub oddsportal: OddsPortalConfig,
    /// 数据导出配置
    pub export: ExportConfig,
}

/// Polymarket 数据源配置
#[derive(Debug, Deserialize)]
pub struct PolymarketConfig {
    /// Polymarket 页面 URL
    pub url: String,
    /// 数据采集间隔（秒）
    pub interval_seconds: u64,
}

/// OddsPortal 数据源配置
#[derive(Debug, Deserialize)]
pub struct OddsPortalConfig {
    /// OddsPortal 页面 URL
    pub url: String,
    /// 数据采集间隔（秒）
    pub interval_seconds: u64,
}

/// 数据导出配置
#[derive(Debug, Deserialize, Clone)]
pub struct ExportConfig {
    /// 比赛 ID
    pub match_id: String,
    /// 导出格式
    pub format: ExportFormat,
}

impl AppConfig {
    /// 从指定路径加载配置文件
    ///
    /// # 参数
    ///
    /// - `path`: 配置文件路径
    ///
    /// # 返回
    ///
    /// 成功时返回解析后的 `AppConfig`，失败时返回错误。
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file at '{}'", path.as_ref().display()))?;

        let config: AppConfig = serde_yaml::from_str(&content)
            .with_context(|| "failed to parse config.yaml")?;

        Ok(config)
    }

    /// 从当前工作目录下的 `config.yaml` 加载配置
    pub fn from_current_dir() -> Result<Self> {
        Self::load("config.yaml")
    }
}
