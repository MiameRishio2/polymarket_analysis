//! OddsPortal 数据提供商模块
//!
//! 本模块实现 [`Provider`] trait，负责从 OddsPortal（知名博彩赔率聚合网站）
//! 抓取和解析博彩公司的赔率数据。主要功能包括：
//!
//! - 发送 HTTP 请求获取 OddsPortal 网页内容
//! - 从 HTML/JSON 混合内容中提取比赛双方队伍信息（多种策略）
//! - 解析赔率数据（支持 `data-odd` 属性行和表格行两种格式）
//! - 提取博彩公司名称及对应的胜/平/负赔率
//!
//! # 比赛信息提取策略
//!
//! [`extract_oddsportal_match_identity`] 按以下优先级尝试提取比赛信息：
//!
//! 1. **结构化参与者 URL**：从 `homeParticipantUrl`/`awayParticipantUrl` 字段提取队伍名称
//! 2. **页面标题/标题元素**：从 `pageH1`、`<title>`、`<h1>` 中提取候选文本
//! 3. **eventOverviewH1Text**：从页面概览标题中提取
//! 4. **event**：从事件字段中提取
//! 5. **全文解析**：将解码后的整个页面文本交由 [`resolve_from_text`] 解析

use aes::Aes256;
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose};
use cbc::cipher::{
    BlockModeDecrypt, KeyIvInit,
    block_padding::{NoPadding, Pkcs7},
};
use chrono::Utc;
use flate2::read::GzDecoder;
use pbkdf2::pbkdf2_hmac;
use regex::Regex;
use reqwest::header::{REFERER, USER_AGENT};
use scraper::{Html, Selector};
use serde_json::Value;
use sha2::Sha256;
use std::io::Read;

use crate::http::build_http_client;
use crate::match_resolver::resolve_from_text;
use crate::model::{BookmakerOdds, MatchIdentity, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

/// 请求 OddsPortal 时使用的 User-Agent 标识字符串
const ODDSPORTAL_USER_AGENT: &str = "polymarket-analysis/0.1";
const ODDSPORTAL_FEED_PASSWORDS: &[(&[u8], &[u8])] = &[
    (
        b"J*8sQ!p$7aD_fR2yW@gHn*3bVp#sAdLd_k",
        b"5b9a8f2c3e6d1a4b7c8e9d0f1a2b3c4d",
    ),
    (
        b"%RtR8AB&nWsh=AQC+v!=pgAe@dSQG3kQ",
        b"orieC_jQQWRmhkPvR6u2kzXeTube6aYupiOddsPortal",
    ),
];

/// OddsPortal 数据提供商
///
/// 封装了 HTTP 客户端，用于向 OddsPortal 发送请求并解析返回的赔率数据。
pub struct OddsPortalProvider {
    client: reqwest::Client,
}

impl OddsPortalProvider {
    /// 创建一个新的 [`OddsPortalProvider`] 实例
    ///
    /// # 参数
    ///
    /// - `proxy_enabled`: 是否启用代理
    /// - `proxy_url`: 代理服务器 URL
    pub fn new(proxy_enabled: bool, proxy_url: &str) -> Self {
        Self {
            client: build_http_client(proxy_enabled, proxy_url)
                .expect("failed to build OddsPortal HTTP client"),
        }
    }

    /// 使用指定的 HTTP 客户端创建 [`OddsPortalProvider`] 实例
    ///
    /// 适用于需要自定义客户端配置（如代理、超时设置等）的场景。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Provider for OddsPortalProvider {
    fn source_name(&self) -> &'static str {
        "oddsportal"
    }

    /// 从 OddsPortal 获取赔率快照
    ///
    /// 处理流程：
    /// 1. 向目标 URL 发送 GET 请求，携带自定义 User-Agent 头部
    /// 2. 获取 HTTP 状态码和响应体
    /// 3. 尝试从响应体中提取比赛双方身份信息（多种策略，包括 H2H URL 解析）
    /// 4. 如果 HTTP 状态码不在 2xx 范围内，返回空赔率列表的快照
    /// 5. 如果请求成功，解析响应体中的赔率数据并返回完整快照
    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot> {
        let identity = target
            .identity
            .clone()
            .or_else(|| extract_oddsportal_match_identity_with_url("", Some(&target.url)).ok())
            .or_else(|| resolve_from_text(&target.url).ok());

        if let Some(feed_url) = oddsportal_prematch_url(&target.url) {
            let response = self
                .client
                .get(&feed_url)
                .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
                .header(REFERER, "https://www.oddsportal.com/")
                .header("x-requested-with", "XMLHttpRequest")
                .send()
                .await?;
            let status = response.status().as_u16();
            let body = response.text().await?;
            let decoded_body = decode_oddsportal_feed(&body)
                .context("failed to decode OddsPortal event data feed")?;
            let odds = if (200..300).contains(&status) {
                parse_oddsportal_odds_for_url(&decoded_body, &target.url)?
            } else {
                Vec::new()
            };

            if !odds.is_empty() || (200..300).contains(&status) {
                return Ok(ProviderSnapshot {
                    source: self.source_name(),
                    collected_at: Utc::now(),
                    http_status: Some(status),
                    identity,
                    payload: ProviderPayload::OddsPortal { odds },
                    raw_body: Some(decoded_body),
                });
            }
        }

        if let Some(ajax_user_data_url) = oddsportal_ajax_user_data_url(&target.url) {
            let response = self
                .client
                .get(&ajax_user_data_url)
                .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
                .header(REFERER, &target.url)
                .header("x-requested-with", "XMLHttpRequest")
                .send()
                .await?;
            let status = response.status().as_u16();
            let body = response.text().await?;
            let identity = extract_oddsportal_match_identity(&body)
                .ok()
                .or_else(|| identity.clone());

            if (200..300).contains(&status)
                && let Some(feed_url) =
                    oddsportal_prematch_url_from_ajax_user_data(&body)?.or_else(|| {
                        oddsportal_event_data_url_from_ajax_user_data(&body)
                            .ok()
                            .flatten()
                    })
            {
                let response = self
                    .client
                    .get(&feed_url)
                    .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
                    .header(REFERER, &target.url)
                    .header("x-requested-with", "XMLHttpRequest")
                    .send()
                    .await?;
                let status = response.status().as_u16();
                let body = response.text().await?;
                let decoded_body = decode_oddsportal_feed(&body)
                    .context("failed to decode OddsPortal event data feed")?;
                let odds = if (200..300).contains(&status) {
                    parse_oddsportal_odds_for_url(&decoded_body, &target.url)?
                } else {
                    Vec::new()
                };

                return Ok(ProviderSnapshot {
                    source: self.source_name(),
                    collected_at: Utc::now(),
                    http_status: Some(status),
                    identity,
                    payload: ProviderPayload::OddsPortal { odds },
                    raw_body: Some(decoded_body),
                });
            }

            return Ok(ProviderSnapshot {
                source: self.source_name(),
                collected_at: Utc::now(),
                http_status: Some(status),
                identity,
                payload: ProviderPayload::OddsPortal { odds: Vec::new() },
                raw_body: Some(body),
            });
        }

        if is_oddsportal_url(&target.url) {
            return Ok(ProviderSnapshot {
                source: self.source_name(),
                collected_at: Utc::now(),
                http_status: None,
                identity,
                payload: ProviderPayload::OddsPortal { odds: Vec::new() },
                raw_body: None,
            });
        }

        let response = self
            .client
            .get(&target.url)
            .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let identity = extract_oddsportal_match_identity_with_url(&body, Some(&target.url))
            .ok()
            .or_else(|| target.identity.clone())
            .or_else(|| resolve_from_text(&target.url).ok());
        if !(200..300).contains(&status) {
            return Ok(ProviderSnapshot {
                source: self.source_name(),
                collected_at: Utc::now(),
                http_status: Some(status),
                identity,
                payload: ProviderPayload::OddsPortal { odds: Vec::new() },
                raw_body: Some(body),
            });
        }

        let odds = parse_oddsportal_odds_for_url(&body, &target.url)?;

        Ok(ProviderSnapshot {
            source: self.source_name(),
            collected_at: Utc::now(),
            http_status: Some(status),
            identity,
            payload: ProviderPayload::OddsPortal { odds },
            raw_body: Some(body),
        })
    }
}

pub fn oddsportal_event_data_url(match_url: &str) -> Option<String> {
    let parsed = url::Url::parse(match_url).ok()?;
    if !is_oddsportal_host(parsed.host_str()?) {
        return None;
    }
    let event_id = parsed.fragment()?.split(':').next()?.trim();
    if event_id.is_empty()
        || event_id.len() < 6
        || !event_id.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(format!(
        "https://www.oddsportal.com/ajax-event-data/{}/0/",
        event_id
    ))
}

pub fn oddsportal_prematch_url(match_url: &str) -> Option<String> {
    let parsed = url::Url::parse(match_url).ok()?;
    if !is_oddsportal_host(parsed.host_str()?) {
        return None;
    }
    let event_id = parsed.fragment()?.split(':').next()?.trim();
    if event_id.is_empty()
        || event_id.len() < 6
        || !event_id.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return None;
    }

    let mut sport_id = None;
    let mut default_bet_id = None;
    let mut default_scope_id = None;
    if let Some(page_data) = oddsportal_embedded_event_data_url(match_url) {
        return Some(page_data);
    }

    if let Some(bet_id) = oddsportal_fragment_bet_id(&parsed) {
        default_bet_id = Some(bet_id);
    }

    // Fallback keeps older H2H URLs working when only the fragment is known.
    if parsed.path().contains("/esports/") {
        sport_id = Some("36");
        default_bet_id.get_or_insert("3");
        default_scope_id = Some("2");
    }

    Some(format!(
        "https://www.oddsportal.com/match-event/1-{}-{}-{}-{}-yj1dd.dat?_={}",
        sport_id.unwrap_or("1"),
        event_id,
        default_bet_id.unwrap_or("1"),
        default_scope_id.unwrap_or("2"),
        Utc::now().timestamp_millis()
    ))
}

fn oddsportal_embedded_event_data_url(_match_url: &str) -> Option<String> {
    None
}

pub fn oddsportal_ajax_user_data_url(match_url: &str) -> Option<String> {
    let parsed = url::Url::parse(match_url).ok()?;
    if !is_oddsportal_host(parsed.host_str()?) {
        return None;
    }

    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    let h2h_index = segments
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case("h2h"))?;
    let sport = segments.get(h2h_index.checked_sub(1)?)?;
    let home = segments.get(h2h_index + 1)?;
    let away = segments.get(h2h_index + 2)?;

    Some(format!(
        "https://www.oddsportal.com/ajax-user-data/h2h/{}/{}/{}/",
        sport, home, away
    ))
}

pub fn oddsportal_prematch_url_from_ajax_user_data(body: &str) -> Result<Option<String>> {
    let values = extract_jsonish_string_values(body, "url")?;
    Ok(values.into_iter().find_map(|value| {
        if !value.contains("/match-event/") {
            return None;
        }
        let mut url = absolutize_oddsportal_ajax_url(&value)?;
        if url.ends_with("_=") {
            url.push_str(&Utc::now().timestamp_millis().to_string());
        }
        Some(url)
    }))
}

pub fn oddsportal_event_data_url_from_ajax_user_data(body: &str) -> Result<Option<String>> {
    let Some(url) = extract_jsonish_string_values(body, "requestEventData")?
        .into_iter()
        .find_map(|value| absolutize_oddsportal_ajax_url(&value))
    else {
        return Ok(None);
    };
    Ok(Some(url))
}

fn absolutize_oddsportal_ajax_url(value: &str) -> Option<String> {
    let decoded = decode_jsonish(value);
    let url = if decoded.starts_with('/') {
        url::Url::parse("https://www.oddsportal.com")
            .ok()?
            .join(&decoded)
            .ok()?
    } else {
        url::Url::parse(&decoded).ok()?
    };

    if !is_oddsportal_host(url.host_str()?) {
        return None;
    }

    Some(url.to_string())
}

fn is_oddsportal_url(value: &str) -> bool {
    url::Url::parse(value)
        .ok()
        .and_then(|parsed| parsed.host_str().map(is_oddsportal_host))
        .unwrap_or(false)
}

fn is_oddsportal_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("www.oddsportal.com") || host.eq_ignore_ascii_case("oddsportal.com")
}

pub fn decode_oddsportal_feed(body: &str) -> Result<String> {
    let decoded = if body.contains(':') {
        body.trim().to_string()
    } else {
        let decoded = general_purpose::STANDARD
            .decode(body.trim())
            .context("failed to base64-decode oddsportal feed envelope")?;
        String::from_utf8(decoded).context("oddsportal feed envelope is not utf-8")?
    };
    let (encrypted, iv_hex) = decoded
        .split_once(':')
        .context("oddsportal feed envelope missing iv separator")?;
    let encrypted = general_purpose::STANDARD
        .decode(encrypted)
        .context("failed to base64-decode oddsportal feed payload")?;
    let iv = decode_hex(iv_hex).context("failed to decode oddsportal feed iv")?;
    if iv.len() != 16 {
        anyhow::bail!("oddsportal feed iv length is {}", iv.len());
    }

    for (password, salt) in ODDSPORTAL_FEED_PASSWORDS {
        let mut key = [0_u8; 32];
        pbkdf2_hmac::<Sha256>(password, salt, 1000, &mut key);

        let mut buffer = encrypted.clone();
        let decrypted = cbc::Decryptor::<Aes256>::new_from_slices(&key, &iv)
            .context("failed to initialize oddsportal feed decryptor")?
            .decrypt_padded::<Pkcs7>(&mut buffer)
            .map(|bytes| bytes.to_vec())
            .or_else(|_| {
                let mut buffer = encrypted.clone();
                cbc::Decryptor::<Aes256>::new_from_slices(&key, &iv)
                    .map_err(|err| anyhow::anyhow!(err))?
                    .decrypt_padded::<NoPadding>(&mut buffer)
                    .map(|bytes| bytes.to_vec())
                    .map_err(|err| anyhow::anyhow!(err))
            });

        let Ok(decrypted) = decrypted else {
            continue;
        };
        let decoded = if decrypted.starts_with(&[0x1f, 0x8b]) {
            let mut decoder = GzDecoder::new(decrypted.as_slice());
            let mut text = String::new();
            decoder.read_to_string(&mut text)?;
            text
        } else {
            match String::from_utf8(decrypted) {
                Ok(text) => text,
                Err(_) => continue,
            }
        };

        return Ok(decoded
            .rfind('}')
            .map(|end| decoded[..=end].to_string())
            .unwrap_or(decoded));
    }

    anyhow::bail!("failed to decrypt oddsportal feed with known keys")
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let value = value.trim();
    if !value.len().is_multiple_of(2) {
        anyhow::bail!("hex string has odd length");
    }
    (0..value.len())
        .step_by(2)
        .map(|idx| {
            u8::from_str_radix(&value[idx..idx + 2], 16)
                .with_context(|| format!("invalid hex byte at {}", idx))
        })
        .collect()
}

/// 从 OddsPortal 页面 HTML 中提取比赛双方身份信息
///
/// 采用多种策略按优先级尝试提取，确保在不同页面结构下都能获取比赛信息：
///
/// 1. **H2H URL 直接提取**：如果提供了 URL 且为 H2H 格式，从 URL 直接解析队伍名称
/// 2. **结构化参与者 URL 提取**：优先尝试从 `homeParticipantUrl` 和 `awayParticipantUrl`
///    字段中提取队伍名称，这是最可靠的方式
/// 3. **页面候选文本收集**：
///    - 从 JSON-like 结构中提取 `pageH1` 字段值
///    - 从 `<title>` 和 `<h1>` 元素中提取文本内容
/// 4. **eventOverviewH1Text**：尝试从页面概览标题字段提取
/// 5. **event 字段**：尝试从事件字段提取
/// 6. **全文回退解析**：将解码后的整个页面文本交由 [`resolve_from_text`] 进行通用解析
pub fn extract_oddsportal_match_identity(body: &str) -> Result<MatchIdentity> {
    extract_oddsportal_match_identity_with_url(body, None)
}

/// 从 OddsPortal 页面 HTML 中提取比赛双方身份信息（可选提供 URL）
///
/// 如果提供了 URL 且为 H2H 格式，会优先从 URL 解析队伍名称。
pub fn extract_oddsportal_match_identity_with_url(
    body: &str,
    url: Option<&str>,
) -> Result<MatchIdentity> {
    let document = Html::parse_document(body);

    if let Some(h2h_url) = url {
        if is_h2h_url(h2h_url) {
            if let Some((home_team, away_team)) = parse_h2h_url(h2h_url) {
                return Ok(MatchIdentity {
                    match_id: crate::match_resolver::match_id_for(&home_team, &away_team),
                    home_team,
                    away_team,
                    match_time: None,
                });
            }
        }
    }

    if let Some(identity) = extract_structured_participant_identity(body)? {
        return Ok(identity);
    }

    let mut page_candidates = extract_jsonish_string_values(body, "pageH1")?;
    for selector in ["title", "h1"] {
        if let Ok(selector) = Selector::parse(selector) {
            page_candidates.extend(
                document
                    .select(&selector)
                    .map(|node| clean_label(&node.text().collect::<Vec<_>>().join(" ")))
                    .filter(|text| !text.is_empty()),
            );
        }
    }

    if let Some(identity) = resolve_first_candidate(page_candidates) {
        return Ok(identity);
    }

    if let Some(identity) =
        resolve_first_candidate(extract_jsonish_string_values(body, "eventOverviewH1Text")?)
    {
        return Ok(identity);
    }

    if let Some(identity) = resolve_first_candidate(extract_jsonish_string_values(body, "event")?) {
        return Ok(identity);
    }

    resolve_from_text(&decode_jsonish(body))
}

/// 从结构化的参与者 URL 中提取比赛身份信息
///
/// 尝试从页面中解析 `homeParticipantUrl` 和 `awayParticipantUrl` 字段的值，
/// 这些字段通常包含指向队伍页面的 URL（如 `https://www.oddsportal.com/team/arsenal-.../`）。
/// 通过 [`team_name_from_participant_url`] 从 URL 中提取队伍名称，并构建 [`MatchIdentity`]。
///
/// 如果任一队伍的 URL 缺失或无法解析，返回 `None`。
fn extract_structured_participant_identity(body: &str) -> Result<Option<MatchIdentity>> {
    let home_values = extract_jsonish_string_values(body, "homeParticipantUrl")?;
    let away_values = extract_jsonish_string_values(body, "awayParticipantUrl")?;
    let Some(home_url) = home_values.first() else {
        return Ok(None);
    };
    let Some(away_url) = away_values.first() else {
        return Ok(None);
    };

    let Some(home_team) = team_name_from_participant_url(home_url) else {
        return Ok(None);
    };
    let Some(away_team) = team_name_from_participant_url(away_url) else {
        return Ok(None);
    };

    Ok(Some(MatchIdentity {
        match_id: crate::match_resolver::match_id_for(&home_team, &away_team),
        home_team,
        away_team,
        match_time: None,
    }))
}

/// 从参与者 URL 中提取队伍名称
///
/// 解析 URL 字符串，提取其中的队伍 slug 并转换为人类可读的队伍名称。
///
/// # URL 解析逻辑
///
/// 1. **解码**：先通过 [`decode_jsonish`] 处理 JSON 风格的转义字符和 HTML 实体
/// 2. **提取 slug**：
///    - 按 `/` 分割 URL，优先查找 `/team/xxx` 模式中的 `xxx` 部分
///    - 如果未找到 `team` 模式，则取倒数第二个路径段作为 slug
/// 3. **去除后缀**：如果 slug 的最后一个 `-` 后面的部分包含大写字母或数字
///   （如 `arsenal-123`），则认为 `-` 前面的部分是纯队伍名称（`arsenal`）
/// 4. **转换为名称**：将 slug 按 `-` 分割，每个部分首字母大写后用空格拼接
///    （如 `manchester-united` → `Manchester United`）
fn team_name_from_participant_url(value: &str) -> Option<String> {
    let decoded = decode_jsonish(value);
    let segments: Vec<&str> = decoded
        .trim_end_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let slug = segments
        .windows(2)
        .find_map(|window| (window[0] == "team").then_some(window[1]))
        .or_else(|| segments.iter().rev().nth(1).copied())?;
    let slug = slug
        .rsplit_once('-')
        .map(|(name, suffix)| {
            if suffix
                .chars()
                .any(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
            {
                name
            } else {
                slug
            }
        })
        .unwrap_or(slug);

    Some(
        slug.split('-')
            .filter(|token| !token.is_empty())
            .map(|token| {
                let mut chars = token.chars();
                let Some(first) = chars.next() else {
                    return String::new();
                };
                format!(
                    "{}{}",
                    first.to_uppercase().collect::<String>(),
                    chars.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// 检测 URL 是否为 OddsPortal H2H（对战）页面格式
///
/// H2H URL 格式示例：
/// `https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/`
///
/// # 检测逻辑
///
/// 1. 检查 URL 是否包含 `/esports/h2h/` 路径段
/// 2. 检查路径是否以两个非空段结尾（H2H 页面的两支队伍）
pub fn is_h2h_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    if !url_lower.contains("/h2h/") {
        return false;
    }

    if let Ok(parsed) = url::Url::parse(url) {
        let segments: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.filter(|seg| !seg.is_empty()).collect())
            .unwrap_or_default();

        let h2h_index = segments.iter().position(|&s| s.eq_ignore_ascii_case("h2h"));

        if let Some(idx) = h2h_index {
            let remaining = &segments[idx + 1..];
            return remaining.len() >= 2 && !remaining[0].is_empty() && !remaining[1].is_empty();
        }
    }

    false
}

/// 从 H2H URL 中提取两支对战队伍的名称
///
/// H2H URL 格式示例：
/// `https://www.oddsportal.com/esports/h2h/keyd-stars-league-of-legends-KbFmk5wg/loud-league-of-legends-8xpjeD0R/`
///
/// # 解析逻辑
///
/// 每个队伍段格式为：`{team-name}-{game-name}-{unique-id}`
/// - Team name 出现在 game name 之前
/// - Game name 是已知的游戏名称（如 `league-of-legends`, `dota-2`, `counter-strike`）
/// - Unique ID 是跟在游戏名称后面的字母数字组合
///
/// 函数会：
/// 1. 检测队伍段中包含的游戏名称
/// 2. 提取游戏名称之前的部分作为队伍名称
/// 3. 将队伍名称从 slug 格式转换为首字母大写格式
pub fn parse_h2h_url(url: &str) -> Option<(String, String)> {
    if !is_h2h_url(url) {
        return None;
    }

    let known_games = [
        "league-of-legends",
        "leagueoflegends",
        "lol",
        "dota-2",
        "dota2",
        "counter-strike",
        "counterstrike",
        "cs2",
        "csgo",
        "valorant",
        "overwatch",
        "rainbow-six",
        "rainbowsix",
        "r6",
    ];

    if let Ok(parsed) = url::Url::parse(url) {
        let segments: Vec<&str> = parsed
            .path_segments()
            .map(|s| s.filter(|seg| !seg.is_empty()).collect())
            .unwrap_or_default();

        let h2h_index = segments
            .iter()
            .position(|&s| s.eq_ignore_ascii_case("h2h"))?;

        let team_segments = &segments[h2h_index + 1..];
        if team_segments.len() < 2 {
            return None;
        }

        let home_raw = team_segments[0];
        let away_raw = team_segments[1];

        let home_team = extract_team_name_from_h2h_segment(home_raw, &known_games)?;
        let away_team = extract_team_name_from_h2h_segment(away_raw, &known_games)?;

        Some((home_team, away_team))
    } else {
        None
    }
}

/// 从 H2H URL 段中提取队伍名称
///
/// # 参数
///
/// - `segment`: H2H URL 中的队伍段（如 `keyd-stars-league-of-legends-KbFmk5wg`）
/// - `known_games`: 已知的游戏名称列表
///
/// # 返回
///
/// 队伍名称（首字母大写格式），如 `Keyd Stars`
fn extract_team_name_from_h2h_segment(segment: &str, known_games: &[&str]) -> Option<String> {
    let segment_lower = segment.to_lowercase();

    for game in known_games {
        if segment_lower.contains(&game.to_lowercase()) {
            let game_start = segment_lower.find(&game.to_lowercase())?;
            let team_slug = &segment[..game_start].trim_end_matches('-');

            if !team_slug.is_empty() {
                return Some(titleize_slug(team_slug));
            }
        }
    }

    if let Some((before_hash, _)) = find_hash_suffix(segment) {
        let team_slug = before_hash.trim_end_matches('-');
        if !team_slug.is_empty() {
            return Some(titleize_slug(&team_slug));
        }
    }

    Some(titleize_slug(segment))
}

/// 查找并返回 hash 后缀的分割点
///
/// Hash 后缀通常是跟在游戏名称后面的字母数字组合，
/// 格式类似于 `KbFmk5wg`（大小写混合的短字符串）
fn find_hash_suffix(segment: &str) -> Option<(String, String)> {
    let chars: Vec<char> = segment.chars().collect();

    for i in (0..chars.len()).rev() {
        if chars[i].is_ascii_alphanumeric() && !chars[i].is_ascii_alphabetic() {
            let before = chars[..i].iter().collect::<String>();
            let hash = chars[i..].iter().collect::<String>();
            return Some((before, hash));
        }
    }

    None
}

/// 将 slug 格式的队伍名称转换为首字母大写格式
///
/// 例如：`keyd-stars` → `Keyd Stars`, `manchester-united` → `Manchester United`
fn titleize_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => format!(
                    "{}{}",
                    first.to_uppercase().collect::<String>(),
                    chars.as_str()
                ),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 解析 OddsPortal 页面中的赔率数据
///
/// 支持两种解析方式，按优先级尝试：
///
/// 1. **data-odd 属性行解析**（通过 [`parse_data_odd_rows`]）：
///    使用正则表达式匹配包含 `data-odd` 属性的 `<tr>` 或 `<div>` 元素，
///    这是 OddsPortal 较新版页面使用的格式。
///
/// 2. **表格行解析**（通过 [`parse_table_like_rows`]）：
///    使用 `scraper` 库解析传统表格结构，选择 `<tr>` 或包含 `odds`/`bookmaker`
///    类名的元素，提取其中的文本赔率值。
///
/// 如果第一种方式未找到任何赔率数据，则回退到第二种方式。
pub fn parse_oddsportal_odds(html: &str) -> Result<Vec<BookmakerOdds>> {
    let odds = parse_oddsportal_json_oddsdata(html, None)?;
    if !odds.is_empty() {
        return Ok(odds);
    }

    let odds = parse_data_odd_rows(html)?;
    if !odds.is_empty() {
        return Ok(odds);
    }

    parse_table_like_rows(html)
}

pub fn parse_oddsportal_odds_for_url(body: &str, match_url: &str) -> Result<Vec<BookmakerOdds>> {
    let preferred_bet_id = url::Url::parse(match_url)
        .ok()
        .as_ref()
        .and_then(oddsportal_fragment_bet_id);
    let odds = parse_oddsportal_json_oddsdata(body, preferred_bet_id)?;
    if !odds.is_empty() {
        return Ok(odds);
    }

    parse_oddsportal_odds(body)
}

fn parse_oddsportal_json_oddsdata(
    body: &str,
    preferred_bet_id: Option<&str>,
) -> Result<Vec<BookmakerOdds>> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Ok(Vec::new());
    };
    let Some(back) = value
        .pointer("/d/oddsdata/back")
        .and_then(|value| value.as_object())
    else {
        return Ok(Vec::new());
    };

    let market_has_odds = |market: &Value| {
        market
            .get("odds")
            .and_then(|odds| odds.as_object())
            .map(|odds| !odds.is_empty())
            .unwrap_or(false)
    };
    let preferred_market = preferred_bet_id.and_then(|bet_id| {
        back.iter()
            .find(|(market_key, market)| {
                oddsportal_market_key_matches_bet_id(market_key, bet_id) && market_has_odds(market)
            })
            .map(|(_, market)| market)
    });
    let Some(market) =
        preferred_market.or_else(|| back.values().find(|market| market_has_odds(market)))
    else {
        return Ok(Vec::new());
    };
    let Some(odds_by_bookmaker) = market.get("odds").and_then(|odds| odds.as_object()) else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    for (bookmaker_id, values) in odds_by_bookmaker {
        let Some(values) = values.as_array() else {
            continue;
        };
        let decimal_values = values
            .iter()
            .filter_map(|value| value.as_f64())
            .collect::<Vec<_>>();
        if decimal_values.len() == 2 && decimal_values.iter().all(|value| *value > 1.0) {
            rows.push(BookmakerOdds {
                bookmaker: bookmaker_name_from_json_market(market, bookmaker_id),
                home: decimal_values[0],
                draw: 0.0,
                away: decimal_values[1],
            });
        } else if decimal_values.len() >= 3 && decimal_values[..3].iter().all(|value| *value > 1.0)
        {
            rows.push(BookmakerOdds {
                bookmaker: bookmaker_name_from_json_market(market, bookmaker_id),
                home: decimal_values[0],
                draw: decimal_values[1],
                away: decimal_values[2],
            });
        }
    }

    rows.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
    Ok(rows)
}

fn oddsportal_fragment_bet_id(parsed: &url::Url) -> Option<&'static str> {
    let market = parsed.fragment()?.split(':').nth(1)?.split(';').next()?;
    match market.trim().to_ascii_lowercase().as_str() {
        "1x2" => Some("1"),
        "home-away" | "home_away" | "homeaway" => Some("3"),
        _ => None,
    }
}

fn oddsportal_market_key_matches_bet_id(market_key: &str, bet_id: &str) -> bool {
    let numbers = market_key
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match numbers.as_slice() {
        [first, ..] if *first == bet_id => true,
        [first, second, ..] if first.len() > 3 && *second == bet_id => true,
        _ => false,
    }
}

fn bookmaker_name_from_json_market(market: &Value, bookmaker_id: &str) -> String {
    market
        .get("bs")
        .and_then(|bs| bs.get(bookmaker_id))
        .and_then(|value| value.as_array())
        .and_then(|values| values.first())
        .and_then(|value| value.as_str())
        .and_then(bookmaker_name_from_betslip)
        .unwrap_or_else(|| format!("Bookmaker {bookmaker_id}"))
}

fn bookmaker_name_from_betslip(value: &str) -> Option<String> {
    let slug = value
        .split("/bookmakers/")
        .nth(1)?
        .split('/')
        .next()?
        .trim();
    if slug.is_empty() {
        return None;
    }

    Some(
        slug.split('-')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        format!(
                            "{}{}",
                            first.to_uppercase().collect::<String>(),
                            chars.as_str()
                        )
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// 从 HTML/JSON 混合内容中提取指定键的字符串值
///
/// OddsPortal 页面通常在 `<script>` 标签中嵌入 JSON-like 数据，
/// 同时也可能在 HTML 属性中使用 HTML 实体编码（如 `&quot;`）。
///
/// 此函数尝试从多个来源和格式中提取值：
///
/// 1. 使用双引号模式 `"<key>" : "<value>"` 匹配原始 HTML 中的 JSON 键值对
/// 2. 使用 `&quot;` 编码模式匹配 HTML 实体编码的 JSON 数据
/// 3. 如果 HTML 解码后的内容与原始内容不同，再对解码后的内容重复上述两种匹配
///
/// 提取的值会通过 [`decode_jsonish`] 进行转义处理，并通过 [`clean_label`] 清理空白。
fn extract_jsonish_string_values(body: &str, key: &str) -> Result<Vec<String>> {
    let plain_pattern = format!(r#"(?s)"{}"\s*:\s*"((?:\\.|[^"\\])*)""#, regex::escape(key));
    let entity_pattern = format!(
        r#"(?s)&quot;{}&quot;\s*:\s*&quot;(.*?)&quot;"#,
        regex::escape(key)
    );
    let mut values = Vec::new();
    let decoded_body = decode_jsonish(body);

    for (source, pattern) in [
        (body, plain_pattern.as_str()),
        (body, entity_pattern.as_str()),
    ] {
        let re = Regex::new(pattern)?;
        values.extend(
            re.captures_iter(source)
                .filter_map(|captures| captures.get(1))
                .map(|value| clean_label(&decode_jsonish(value.as_str())))
                .filter(|value| !value.is_empty()),
        );
    }

    if decoded_body != body {
        for pattern in [plain_pattern.as_str(), entity_pattern.as_str()] {
            let re = Regex::new(pattern)?;
            values.extend(
                re.captures_iter(&decoded_body)
                    .filter_map(|captures| captures.get(1))
                    .map(|value| clean_label(&decode_jsonish(value.as_str())))
                    .filter(|value| !value.is_empty()),
            );
        }
    }

    Ok(values)
}

/// 从候选字符串列表中尝试解析第一个有效的比赛身份信息
///
/// 遍历候选列表，对每个候选字符串通过 [`decode_jsonish`] 解码后，
/// 交由 [`resolve_from_text`] 进行通用文本解析，返回第一个成功解析的结果。
fn resolve_first_candidate(candidates: Vec<String>) -> Option<MatchIdentity> {
    candidates
        .into_iter()
        .find_map(|candidate| resolve_from_text(&decode_jsonish(&candidate)).ok())
}

/// 使用正则表达式解析包含 `data-odd` 属性的赔率行
///
/// 通过两个正则表达式协同工作：
///
/// 1. **行匹配** `<(?:tr|div)\b[^>]*>.*?</(?:tr|div)>`：
///    匹配 `<tr>` 或 `<div>` 标签及其内容，捕获完整的行元素
///
/// 2. **赔率匹配** `data-odd\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))`：
///    从行元素中提取 `data-odd` 属性的值，支持双引号、单引号和无引号三种格式
///
/// 对于每行，如果提取到至少 3 个有效的十进制赔率值，
/// 则调用 [`bookmaker_odds_from_values`] 构建 [`BookmakerOdds`] 记录。
fn parse_data_odd_rows(html: &str) -> Result<Vec<BookmakerOdds>> {
    let row_re = Regex::new(r#"(?is)<(?:tr|div)\b[^>]*>.*?</(?:tr|div)>"#)?;
    let odd_re = Regex::new(r#"(?i)data-odd\s*=\s*(?:"([^"]+)"|'([^']+)'|([^\s>]+))"#)?;
    let mut rows = Vec::new();

    for row in row_re.find_iter(html).map(|found| found.as_str()) {
        let odds: Vec<f64> = odd_re
            .captures_iter(row)
            .filter_map(|captures| {
                captures
                    .get(1)
                    .or_else(|| captures.get(2))
                    .or_else(|| captures.get(3))
            })
            .filter_map(|value| parse_decimal_odd(&decode_jsonish(value.as_str())))
            .collect();

        if odds.len() >= 3
            && let Some(bookmaker_odds) = bookmaker_odds_from_values(row, &odds[..3])
        {
            rows.push(bookmaker_odds);
        }
    }

    Ok(rows)
}

/// 使用 scraper 库解析类表格结构的赔率行
///
/// 依次尝试三种 CSS 选择器（`tr`、`[class*=odds]`、`[class*=bookmaker]`），
/// 匹配表格行或包含赔率/博彩公司类名的元素。
///
/// 对每个匹配的元素，提取其文本内容并按空白分割，
/// 尝试将每个分词解析为十进制赔率值。如果找到至少 3 个有效赔率值，
/// 则调用 [`bookmaker_odds_from_values`] 构建 [`BookmakerOdds`] 记录。
///
/// 一旦某个选择器成功匹配到赔率数据，立即停止后续选择器的尝试。
fn parse_table_like_rows(html: &str) -> Result<Vec<BookmakerOdds>> {
    let document = Html::parse_document(html);
    let mut rows = Vec::new();

    for selector in ["tr", "[class*=odds]", "[class*=bookmaker]"] {
        let Ok(selector) = Selector::parse(selector) else {
            continue;
        };

        for node in document.select(&selector) {
            let label = clean_label(&node.text().collect::<Vec<_>>().join(" "));
            let values: Vec<f64> = label
                .split_whitespace()
                .filter_map(parse_decimal_odd)
                .collect();

            if values.len() >= 3
                && let Some(bookmaker_odds) = bookmaker_odds_from_values(&label, &values[..3])
            {
                rows.push(bookmaker_odds);
            }
        }

        if !rows.is_empty() {
            break;
        }
    }

    Ok(rows)
}

/// 从赔率值数组构建 [`BookmakerOdds`] 记录
///
/// 验证赔率数组至少包含 3 个值，且所有值均大于 1.0。
/// 取前 3 个值分别作为主胜（home）、平局（draw）、客胜（away）赔率，
/// 并通过 [`extract_bookmaker`] 从行文本中提取博彩公司名称。
fn bookmaker_odds_from_values(row: &str, values: &[f64]) -> Option<BookmakerOdds> {
    if values.len() < 3 || values.iter().any(|value| *value <= 1.0) {
        return None;
    }

    Some(BookmakerOdds {
        bookmaker: extract_bookmaker(row),
        home: values[0],
        draw: values[1],
        away: values[2],
    })
}

/// 从行 HTML 文本中提取博彩公司名称
///
/// 按以下优先级尝试提取：
///
/// 1. **HTML 属性匹配**：依次尝试匹配 `data-bookmaker`、`title`、`alt` 属性值
/// 2. **文本回退**：如果属性匹配失败，则解析 HTML 片段，
///    提取文本中位于第一个赔率数值之前的部分作为博彩公司名称
fn extract_bookmaker(row: &str) -> String {
    for attr in ["data-bookmaker", "title", "alt"] {
        let Ok(re) = Regex::new(&format!(
            r#"(?i)\b{}\s*=\s*(?:"([^"]+)"|'([^']+)')"#,
            regex::escape(attr)
        )) else {
            continue;
        };

        let Some(value) = re.captures(row).and_then(|captures| {
            captures
                .get(1)
                .or_else(|| captures.get(2))
                .map(|value| clean_label(&decode_jsonish(value.as_str())))
        }) else {
            continue;
        };

        if !value.is_empty() {
            return value;
        }
    }

    let document = Html::parse_fragment(row);
    let text = clean_label(&document.root_element().text().collect::<Vec<_>>().join(" "));
    text.split_whitespace()
        .take_while(|part| parse_decimal_odd(part).is_none())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// 解析十进制格式的赔率字符串为 `f64` 值
///
/// 处理流程：
/// 1. 去除首尾空白
/// 2. 去除包裹的逗号、分号、括号等标点符号
/// 3. 尝试解析为 `f64` 浮点数
/// 4. **验证**：赔率必须满足 `1.0 < odd < 100.0` 才视为有效
///
/// 验证逻辑说明：
/// - 大于 1.0：十进制赔率必须大于 1.0（1.0 表示无收益，不构成有效赔率）
/// - 小于 100.0：过滤掉明显异常的极大值（可能是数据错误或非赔率数值）
fn parse_decimal_odd(value: &str) -> Option<f64> {
    let cleaned = value
        .trim()
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '('));
    let value = cleaned.parse::<f64>().ok()?;
    (value > 1.0 && value < 100.0).then_some(value)
}

/// 解码 JSON/HTML 风格的转义字符和实体
///
/// 处理以下转义和编码：
/// - JSON 斜杠转义 `\/` → `/`
/// - JSON 引号转义 `\"` → `"`
/// - JSON 换行/制表符 `\n`/`\t` → 空格
/// - HTML 实体 `&quot;`、`&#34;` → `"`
/// - HTML 实体 `&amp;` → `&`
/// - HTML 实体 `&nbsp;`、`&#160;` → 空格
/// - HTML 实体 `&rsquo;` → `'`
/// - HTML 实体 `&ldquo;`/`&rdquo;` → `"`
/// - HTML 实体 `&lt;`/`&gt;` → `<`/`>`
///
/// 最后通过 [`clean_label`] 清理多余空白。
fn decode_jsonish(value: &str) -> String {
    clean_label(
        &value
            .replace(r#"\/"#, "/")
            .replace(r#"\""#, "\"")
            .replace(r#"\n"#, " ")
            .replace(r#"\t"#, " ")
            .replace("&quot;", "\"")
            .replace("&#34;", "\"")
            .replace("&amp;", "&")
            .replace("&nbsp;", " ")
            .replace("&#160;", " ")
            .replace("&rsquo;", "'")
            .replace("&ldquo;", "\"")
            .replace("&rdquo;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">"),
    )
}

/// 清理文本标签，将连续空白字符规范化为单个空格
///
/// 按空白分割文本后重新以单个空格拼接，去除首尾空白
/// 并将中间的所有连续空白（包括换行、制表符）压缩为单个空格。
fn clean_label(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
