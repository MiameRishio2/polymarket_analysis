//! 本模块定义了项目的核心数据模型。
//!
//! 包含比赛标识、博彩公司赔率、Polymarket市场价格、快照记录等
//! 关键数据结构，以及解析状态和数据提供商载荷的枚举类型。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 比赛标识，用于唯一标识一场比赛。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchIdentity {
    /// 比赛的唯一编号。
    pub match_id: String,
    /// 主队名称。
    pub home_team: String,
    /// 客队名称。
    pub away_team: String,
    /// 比赛时间（可选）。
    pub match_time: Option<DateTime<Utc>>,
}

/// 博彩公司赔率，记录单一博彩公司对某场比赛的胜平负赔率。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BookmakerOdds {
    /// 博彩公司名称。
    pub bookmaker: String,
    /// 主队获胜赔率。
    pub home: f64,
    /// 平局赔率。
    pub draw: f64,
    /// 客队获胜赔率。
    pub away: f64,
}

/// Polymarket市场价格，记录预测市场上某一结果的价格信息。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolymarketPrice {
    /// 市场唯一编号（可选）。
    pub market_id: Option<String>,
    /// 市场标题。
    pub market_title: String,
    /// 结果选项（如"是"/"否"或具体结果名称）。
    pub outcome: String,
    /// 当前价格，范围为 0.0 到 1.0。
    pub price: f64,
    /// 交易量（可选）。
    pub volume: Option<f64>,
    /// 市场是否处于活跃状态（可选）。
    pub active: Option<bool>,
}

/// 解析状态，表示数据解析的结果。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParseStatus {
    /// 解析成功。
    Parsed,
    /// 数据为空，无有效内容可解析。
    Empty,
    /// 解析失败。
    Failed,
}

/// 快照记录，存储每次数据采集的元数据和结果状态。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// 记录的唯一编号。
    pub id: i64,
    /// 比赛编号。
    pub match_id: String,
    /// 数据来源（如"polymarket"、"oddsportal"）。
    pub source: String,
    /// 数据采集时间。
    pub collected_at: DateTime<Utc>,
    /// HTTP 响应状态码（可选）。
    pub http_status: Option<i64>,
    /// 数据解析状态。
    pub parse_status: ParseStatus,
    /// 原始数据内容的哈希值（可选）。
    pub raw_hash: Option<String>,
    /// 原始数据文件的存储路径（可选）。
    pub raw_artifact_path: Option<String>,
    /// 错误信息（可选，解析或采集失败时记录）。
    pub error_message: Option<String>,
}

/// 数据提供商载荷，封装来自不同数据源的数据。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProviderPayload {
    /// Polymarket 数据载荷，包含一组市场价格。
    Polymarket { prices: Vec<PolymarketPrice> },
    /// OddsPortal 数据载荷，包含一组博彩公司赔率。
    OddsPortal { odds: Vec<BookmakerOdds> },
}
