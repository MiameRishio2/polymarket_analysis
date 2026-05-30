//! 数据提供商模块
//!
//! 本模块定义了数据提供商的通用接口和数据结构，用于从不同数据源（如 Polymarket、OddsPortal 等）
//! 抓取和统一处理数据快照。通过抽象出 `Provider` trait，可以方便地扩展新的数据源。

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::future::Future;

use crate::model::{MatchIdentity, ProviderPayload};

pub mod oddsportal;
pub mod polymarket;

/// 数据抓取目标
///
/// 表示一个需要抓取的数据目标，包含目标 URL 和可选的比赛标识信息。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderTarget {
    /// 目标 URL 地址
    pub url: String,
    /// 可选的比赛标识信息，用于关联具体的比赛或事件
    pub identity: Option<MatchIdentity>,
}

/// 数据快照
///
/// 表示从数据源抓取到的一次性数据快照，包含原始数据及抓取时的元信息。
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderSnapshot {
    /// 数据源名称，标识数据来源（如 "polymarket"、"oddsportal"）
    pub source: &'static str,
    /// 数据抓取的时间戳（UTC）
    pub collected_at: DateTime<Utc>,
    /// HTTP 响应状态码，抓取失败时可能为 None
    pub http_status: Option<u16>,
    /// 比赛标识信息，用于关联具体的比赛或事件
    pub identity: Option<MatchIdentity>,
    /// 解析后的结构化数据负载
    pub payload: ProviderPayload,
    /// 原始响应体内容，便于调试或二次解析
    pub raw_body: Option<String>,
}

/// 数据提供商接口
///
/// 定义了从数据源抓取数据的通用行为。实现此 trait 可以为系统添加新的数据源。
pub trait Provider {
    /// 返回数据源名称
    ///
    /// 此名称用于标识数据来源，应返回一个静态字符串，如 `"polymarket"` 或 `"oddsportal"`。
    fn source_name(&self) -> &'static str;

    /// 抓取数据快照
    ///
    /// 根据给定的抓取目标，从数据源获取一次数据快照。
    ///
    /// # 参数
    ///
    /// * `target` - 包含目标 URL 和可选比赛标识的抓取目标
    ///
    /// # 返回值
    ///
    /// 返回一个 `Future`，其输出为 `Result<ProviderSnapshot>`。成功时包含抓取到的数据快照，
    /// 失败时返回错误信息。
    fn fetch_snapshot(
        &self,
        target: &ProviderTarget,
    ) -> impl Future<Output = Result<ProviderSnapshot>> + Send;
}
