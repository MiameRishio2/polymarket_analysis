//! Polymarket 数据提供商模块
//!
//! 本模块实现了从 Polymarket API 抓取和解析市场赔率数据的功能。
//! Polymarket 是一个去中心化的预测市场平台，本模块负责：
//!
//! - 通过 Gamma API 按 event/market slug 获取市场数据
//! - 从 API 返回的 outcomes/outcomePrices 中解析赔率
//! - API 不可用时回退到 Polymarket 页面 HTML 中的嵌入 JSON 数据
//! - 解析市场数据，包括赔率（outcomes/outcomePrices）、交易量、市场标题等信息
//! - 兼容新旧两种数据格式
//! - 从 API 或页面内容中识别比赛对阵信息

use anyhow::Result;
use chrono::Utc;
use rs_clob_client_v2::types::{Event, Market};
use serde_json::Value;
use url::Url;

use crate::http::build_http_client;
use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, PolymarketPrice, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

/// Polymarket 数据提供商
///
/// 负责通过 HTTP 客户端获取 Polymarket API 快照，并从中解析市场赔率数据。
pub struct PolymarketProvider {
    client: reqwest::Client,
}

impl PolymarketProvider {
    /// 创建一个新的 PolymarketProvider 实例
    ///
    /// # 参数
    ///
    /// - `proxy_enabled`: 是否启用代理
    /// - `proxy_url`: 代理服务器 URL
    pub fn new(proxy_enabled: bool, proxy_url: &str) -> Self {
        Self {
            client: build_http_client(proxy_enabled, proxy_url)
                .expect("failed to build Polymarket HTTP client"),
        }
    }

    /// 使用指定的 HTTP 客户端创建 PolymarketProvider 实例
    ///
    /// 适用于需要自定义客户端配置（如代理、超时等）的场景。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Provider for PolymarketProvider {
    /// 返回数据来源名称
    fn source_name(&self) -> &'static str {
        "polymarket"
    }

    /// 获取市场快照
    ///
    /// 处理流程：
    /// 1. 优先从目标 URL 解析 event/market slug，并请求 Gamma API
    /// 2. 从 API 响应中的 outcomes/outcomePrices 解析赔率
    /// 3. 如果 API 不可用，向目标 URL 发送 HTTP GET 请求并回退到页面解析
    /// 4. 尝试从响应体中提取比赛身份信息（`extract_polymarket_identity`）
    ///    - 如果提取失败，回退到使用 `target.identity`
    ///    - 如果仍不可用，尝试从 URL 文本解析
    /// 5. 如果 HTTP 状态码不在 2xx 范围内，返回空价格列表的快照
    /// 6. 如果请求成功，解析市场赔率数据（`parse_polymarket_market`）
    /// 7. 返回包含身份信息、赔率数据和原始响应体的完整快照
    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot> {
        if let Some(api_target) = PolymarketApiTarget::from_url(&target.url) {
            match fetch_polymarket_api_snapshot(&self.client, api_target).await {
                Ok((body, prices)) if !prices.is_empty() => {
                    let identity = extract_polymarket_identity(&body)
                        .ok()
                        .or_else(|| target.identity.clone())
                        .or_else(|| resolve_from_text(&target.url).ok());

                    return Ok(ProviderSnapshot {
                        source: self.source_name(),
                        collected_at: Utc::now(),
                        http_status: Some(200),
                        identity,
                        payload: ProviderPayload::Polymarket { prices },
                        raw_body: Some(body),
                    });
                }
                Ok(_) | Err(_) => {
                    // Fall through to the legacy page parser when Gamma has no usable price data
                    // or when the target URL is not a direct event/market slug.
                }
            }
        }

        let response = self.client.get(&target.url).send().await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let identity = extract_polymarket_identity(&body)
            .ok()
            .or_else(|| target.identity.clone())
            .or_else(|| resolve_from_text(&target.url).ok());
        if !(200..300).contains(&status) {
            return Ok(ProviderSnapshot {
                source: self.source_name(),
                collected_at: Utc::now(),
                http_status: Some(status),
                identity,
                payload: ProviderPayload::Polymarket { prices: Vec::new() },
                raw_body: Some(body),
            });
        }

        let prices = parse_polymarket_market(&body).unwrap_or_default();

        Ok(ProviderSnapshot {
            source: self.source_name(),
            collected_at: Utc::now(),
            http_status: Some(status),
            identity,
            payload: ProviderPayload::Polymarket { prices },
            raw_body: Some(body),
        })
    }
}

enum PolymarketApiTarget {
    EventSlug(String),
    MarketSlug(String),
}

impl PolymarketApiTarget {
    fn from_url(url: &str) -> Option<Self> {
        let parsed = Url::parse(url).ok()?;
        let segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();

        if let Some(sports_index) = segments.iter().position(|segment| *segment == "sports")
            && let Some(slug) = segments.get(sports_index + 2)
        {
            return Some(Self::MarketSlug((*slug).to_string()));
        }

        let path = if segments
            .first()
            .is_some_and(|segment| looks_like_locale_segment(segment))
        {
            &segments[1..]
        } else {
            &segments[..]
        };

        match path {
            ["event", slug] => Some(Self::EventSlug((*slug).to_string())),
            ["event", event_slug, _market_slug] => Some(Self::EventSlug((*event_slug).to_string())),
            ["market", slug] | ["markets", slug] => Some(Self::MarketSlug((*slug).to_string())),
            [slug] if !slug.is_empty() => Some(Self::MarketSlug((*slug).to_string())),
            _ => None,
        }
    }
}

fn looks_like_locale_segment(segment: &str) -> bool {
    let mut parts = segment.split('-');
    let Some(language) = parts.next() else {
        return false;
    };

    language.len() == 2
        && language.chars().all(|ch| ch.is_ascii_lowercase())
        && parts.all(|part| {
            (part.len() == 2 || part.len() == 4) && part.chars().all(|ch| ch.is_ascii_alphabetic())
        })
}

async fn fetch_polymarket_api_snapshot(
    client: &reqwest::Client,
    target: PolymarketApiTarget,
) -> Result<(String, Vec<PolymarketPrice>)> {
    match target {
        PolymarketApiTarget::EventSlug(slug) => {
            let url = format!(
                "https://gamma-api.polymarket.com/events/slug/{}",
                urlencoding::encode(&slug)
            );
            let event = client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json::<Event>()
                .await?;
            let body = serde_json::to_string(&event)?;
            let prices = event
                .markets
                .as_deref()
                .unwrap_or_default()
                .iter()
                .flat_map(parse_polymarket_value)
                .collect();

            Ok((body, prices))
        }
        PolymarketApiTarget::MarketSlug(slug) => {
            let url = format!(
                "https://gamma-api.polymarket.com/markets/slug/{}",
                urlencoding::encode(&slug)
            );
            let market = client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .json::<Market>()
                .await?;
            let body = serde_json::to_string(&market)?;
            let prices = parse_polymarket_api_market(&market);

            Ok((body, prices))
        }
    }
}

fn parse_polymarket_api_market(market: &Market) -> Vec<PolymarketPrice> {
    let value = serde_json::to_value(market).unwrap_or(Value::Null);
    parse_polymarket_value(&value)
}

/// 从 HTML 响应体中提取 Polymarket 比赛身份信息
///
/// 处理流程：
/// 1. 遍历页面中所有提取出的 JSON 候选对象（`polymarket_json_candidates`）
/// 2. 对每个 JSON 对象，收集所有可能的标题候选字符串
///    （通过 `polymarket_title_candidates`，从 question/title/marketTitle/Title 等字段获取）
/// 3. 对每个标题候选，尝试解析为 `MatchIdentity`（`resolve_from_text`）
/// 4. 如果所有 JSON 候选都失败，最后尝试直接从整个 HTML 体中解析
pub fn extract_polymarket_identity(body: &str) -> Result<MatchIdentity> {
    for value in polymarket_json_candidates(body) {
        for text in polymarket_title_candidates(&value) {
            if let Ok(identity) = resolve_from_text(&text) {
                return Ok(identity);
            }
        }
    }

    resolve_from_text(body)
}

/// 解析 Polymarket 市场赔率数据
///
/// 处理流程：
/// 1. 遍历页面中所有 JSON 候选对象（`polymarket_json_candidates`）
/// 2. 对每个 JSON 对象调用 `parse_polymarket_value` 尝试解析
/// 3. 如果某个候选解析出了非空的价格列表，立即返回
/// 4. 如果所有嵌入 JSON 都失败，尝试将整个响应体作为 JSON 解析
pub fn parse_polymarket_market(body: &str) -> Result<Vec<PolymarketPrice>> {
    for value in polymarket_json_candidates(body) {
        let prices = parse_polymarket_value(&value);
        if !prices.is_empty() {
            return Ok(prices);
        }
    }

    let value: Value = serde_json::from_str(body)?;
    Ok(parse_polymarket_value(&value))
}

/// 从单个 JSON 值中解析 Polymarket 价格数据
///
/// 处理流程：
/// 1. 首先尝试兼容旧格式（`parse_legacy_match_polymarket`），检查是否存在顶层 "Polymarket" 键
/// 2. 如果不是旧格式，递归查找包含 "outcomes" 和 "outcomePrices" 的市场对象（`find_market_value`）
/// 3. 从找到的市场对象中提取：
///    - market_id: 从 "id" 字段
///    - market_title: 从 "question"/"title"/"marketTitle" 字段（按优先级）
///    - volume: 从 "volume" 字段
///    - active: 从 "active" 字段
///    - outcomes: 从 "outcomes" 字段读取字符串数组
///    - prices: 从 "outcomePrices" 字段读取浮点数数组
/// 4. 将 outcomes 和 prices 按索引一一配对，生成 `PolymarketPrice` 列表
fn parse_polymarket_value(value: &Value) -> Vec<PolymarketPrice> {
    if let Some(prices) = parse_legacy_match_polymarket(value) {
        return prices;
    }

    let value = find_market_value(value).unwrap_or(value);
    let market_id = value
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let market_title = value
        .get("question")
        .or_else(|| value.get("title"))
        .or_else(|| value.get("marketTitle"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let volume = value.get("volume").and_then(read_optional_f64);
    let active = value.get("active").and_then(Value::as_bool);
    let outcomes = read_string_array(value.get("outcomes"));
    let prices = read_f64_array(value.get("outcomePrices"));

    outcomes
        .into_iter()
        .zip(prices)
        .map(|(outcome, price)| PolymarketPrice {
            market_id: market_id.clone(),
            market_title: market_title.clone(),
            outcome,
            price,
            volume,
            active,
        })
        .collect()
}

/// 兼容旧版 Polymarket 数据格式
///
/// 旧格式的数据结构为：`{"Polymarket": {"Title": "...", "YesPrice": "...", "NoPrice": "...", ...}}`
///
/// 如果找到顶层 "Polymarket" 键，则从中提取 Yes/No 两种结果的价格，
/// 返回包含两条 `PolymarketPrice` 记录的列表。
/// 如果不存在 "Polymarket" 键，返回 None。
fn parse_legacy_match_polymarket(value: &Value) -> Option<Vec<PolymarketPrice>> {
    let market = value.get("Polymarket")?;
    let title = market.get("Title")?.as_str()?.to_string();
    let market_id = market
        .get("MarketID")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let volume = market.get("Volume").and_then(read_optional_f64);
    let active = market.get("Active").and_then(Value::as_bool);
    let yes = market.get("YesPrice").and_then(read_optional_f64)?;
    let no = market.get("NoPrice").and_then(read_optional_f64)?;

    Some(vec![
        PolymarketPrice {
            market_id: market_id.clone(),
            market_title: title.clone(),
            outcome: "Yes".to_string(),
            price: yes,
            volume,
            active,
        },
        PolymarketPrice {
            market_id,
            market_title: title,
            outcome: "No".to_string(),
            price: no,
            volume,
            active,
        },
    ])
}

/// 从 JSON 值中收集所有可能的标题候选字符串
///
/// 返回的标题来自以下字段：question, title, marketTitle, Title
/// 会递归遍历整个 JSON 树，收集所有匹配字段的字符串值。
fn polymarket_title_candidates(value: &Value) -> Vec<String> {
    let mut titles = Vec::new();
    collect_title_candidates(value, &mut titles);
    titles
}

/// 递归遍历 JSON 值，收集所有标题候选字段
///
/// - 对于 Object 类型：检查 question/title/marketTitle/Title 四个键，
///   然后递归遍历所有子值
/// - 对于 Array 类型：递归遍历每个元素
/// - 其他类型：忽略
fn collect_title_candidates(value: &Value, titles: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for key in ["question", "title", "marketTitle", "Title"] {
                if let Some(text) = object.get(key).and_then(Value::as_str) {
                    titles.push(text.to_string());
                }
            }
            for value in object.values() {
                collect_title_candidates(value, titles);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_title_candidates(value, titles);
            }
        }
        _ => {}
    }
}

/// 递归查找包含 "outcomes" 和 "outcomePrices" 字段的市场对象
///
/// 搜索策略：
/// - 如果当前对象同时包含 "outcomes" 和 "outcomePrices" 键，直接返回当前对象
/// - 否则递归搜索所有子对象（优先遍历 Object 的子值，然后遍历 Array 的元素）
/// - 返回第一个匹配的市场对象，如果找不到则返回 None
fn find_market_value(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(object) => {
            if object.contains_key("outcomes") && object.contains_key("outcomePrices") {
                return Some(value);
            }
            object.values().find_map(find_market_value)
        }
        Value::Array(values) => values.iter().find_map(find_market_value),
        _ => None,
    }
}

/// 从 HTML 响应体中提取所有可能的 JSON 候选值
///
/// 处理流程：
/// 1. 首先尝试将整个响应体作为 JSON 解析，如果成功则加入候选列表
/// 2. 然后通过 `extract_balanced_json_objects` 从 HTML 中提取所有顶层 JSON 对象字符串
///    （处理字符串转义、括号匹配等），尝试逐个解析为 JSON 值
/// 3. 返回所有成功解析的 JSON 值
fn polymarket_json_candidates(body: &str) -> Vec<Value> {
    let mut candidates = Vec::new();
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        candidates.push(value);
    }

    for json in extract_balanced_json_objects(body) {
        if let Ok(value) = serde_json::from_str::<Value>(&json) {
            candidates.push(value);
        }
    }

    candidates
}

/// 从 HTML 文本中提取所有顶层平衡的 JSON 对象字符串
///
/// 提取算法：
/// 使用状态机逐字符扫描 HTML 文本，维护以下状态：
/// - `in_string`: 是否处于 JSON 字符串值内部（双引号之间）
/// - `escaped`: 是否遇到了转义字符（反斜杠后紧跟下一个字符）
/// - `depth`: 当前大括号嵌套深度
///
/// 处理规则：
/// 1. 在字符串内部时：
///    - 遇到 `\` 标记转义状态，跳过下一个字符的特殊含义
///    - 遇到 `"` 且未转义时，退出字符串状态
///    - 其他字符一律忽略（不触发括号计数）
/// 2. 在字符串外部时：
///    - 遇到 `"` 进入字符串状态
///    - 遇到 `{` 增加嵌套深度，如果是第一个 `{`（depth 从 0 变为 1），记录起始位置
///    - 遇到 `}` 减少嵌套深度，如果 depth 回到 0，说明一个完整的 JSON 对象扫描完毕
/// 3. 当一个完整的 JSON 对象提取完成后，检查其是否包含 "outcomePrices" 或 "Polymarket" 关键字，
///    只有包含关键字的对象才会被加入结果列表（过滤无关的 JSON 片段）
fn extract_balanced_json_objects(body: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in body.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0
                    && let Some(start_index) = start.take()
                {
                    let candidate = &body[start_index..=index];
                    if candidate.contains("outcomePrices") || candidate.contains("Polymarket") {
                        objects.push(candidate.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    objects
}

/// 从 JSON 值中读取字符串数组
///
/// 支持两种格式：
/// - 直接的 JSON 数组：`["a", "b", "c"]`
/// - 字符串形式的 JSON 数组：`"[\"a\", \"b\", \"c\"]"`（会被二次解析）
fn read_string_array(value: Option<&Value>) -> Vec<String> {
    read_array(value, |value| value.as_str().map(ToOwned::to_owned))
}

/// 从 JSON 值中读取 f64 浮点数数组
///
/// 支持两种格式：
/// - 直接的 JSON 数组：`[0.5, 0.3]`
/// - 字符串形式的 JSON 数组：`"[0.5, 0.3]"`（会被二次解析）
/// 每个元素会尝试作为数字解析，失败时尝试作为字符串解析（如 "0.5"）
fn read_f64_array(value: Option<&Value>) -> Vec<f64> {
    read_array(value, read_optional_f64)
}

/// 通用的数组读取函数
///
/// 支持两种输入格式：
/// 1. 直接的 JSON 数组：对每个元素应用 `read_item` 函数提取值
/// 2. JSON 字符串（编码后的数组）：先解析字符串为 JSON，再对内部数组元素应用 `read_item`
///
/// 只保留成功提取的元素，忽略无法解析的元素。
fn read_array<T>(value: Option<&Value>, read_item: impl Fn(&Value) -> Option<T>) -> Vec<T> {
    match value {
        Some(Value::Array(values)) => values.iter().filter_map(read_item).collect(),
        Some(Value::String(encoded)) => serde_json::from_str::<Value>(encoded)
            .ok()
            .and_then(|decoded| match decoded {
                Value::Array(values) => Some(
                    values
                        .into_iter()
                        .filter_map(|value| read_item(&value))
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// 从 JSON 值中读取可选的 f64 浮点数
///
/// 支持两种格式：
/// - 直接的数字值：`0.5`
/// - 字符串形式的数字：`"0.5"`
/// 如果两者都解析失败，返回 None。
fn read_optional_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
}

#[cfg(test)]
mod tests {
    use super::PolymarketApiTarget;

    #[test]
    fn parses_localized_sports_market_url_as_market_slug() {
        let target = PolymarketApiTarget::from_url(
            "https://polymarket.com/ja/sports/nba/nba-nyk-sas-2026-06-03",
        );

        assert!(
            matches!(target, Some(PolymarketApiTarget::MarketSlug(slug)) if slug == "nba-nyk-sas-2026-06-03")
        );
    }

    #[test]
    fn parses_default_sports_market_url_as_market_slug() {
        let target = PolymarketApiTarget::from_url(
            "https://polymarket.com/sports/nba/nba-nyk-sas-2026-06-03",
        );

        assert!(
            matches!(target, Some(PolymarketApiTarget::MarketSlug(slug)) if slug == "nba-nyk-sas-2026-06-03")
        );
    }
}
