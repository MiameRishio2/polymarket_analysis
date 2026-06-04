//! # 比赛解析模块
//!
//! 本模块负责从文本或 URL 中解析比赛信息（主要是对阵双方的队伍名称）。
//!
//! ## 功能概述
//!
//! 提供多种策略来从不同格式的输入文本中提取主客队名称：
//!
//! - 尝试从 URL slug 格式中解析（如 `lakers-vs-celtics`）
//! - 尝试从包含 "vs" 分隔符的文本中解析（如 `Lakers vs Celtics`）
//! - 尝试从包含 " - " 分隔符的文本中解析（如 `Lakers - Celtics`）
//!
//! 解析完成后，返回包含 match_id、主队名称、客队名称的 [`MatchIdentity`] 结构体。

use anyhow::{Result, bail};
use regex::Regex;
use std::sync::OnceLock;

use crate::model::MatchIdentity;

/// 从文本或 URL 中解析比赛信息。
///
/// 此函数是模块的公开入口点，负责协调多种解析策略：
///
/// 1. 首先对输入文本进行 HTML 实体解码（如 `&amp;` → `&`），并将不换行空格替换为普通空格
/// 2. 尝试从 URL slug 格式中提取候选文本，例如 URL 路径末端的 `team-a-vs-team-b` 片段
/// 3. 如果 URL slug 解析成功，则调用 [`resolve_normalized_text`] 进一步解析
/// 4. 如果 URL slug 解析失败，则直接对原始规范化文本调用 [`resolve_normalized_text`] 作为 fallback
///
/// # 参数
///
/// * `text` - 待解析的原始文本，可能包含 HTML 实体、URL 路径等
///
/// # 返回值
///
/// 成功时返回解析后的 [`MatchIdentity`]，包含 match_id、主队和客队名称；
/// 失败时返回错误，表示无法从文本中解析出有效的比赛信息。
pub fn resolve_from_text(text: &str) -> Result<MatchIdentity> {
    let normalized = html_unescape(text).replace('\u{a0}', " ");

    if let Some(slug_candidate) = url_slug_match_candidate(&normalized)
        && let Ok(identity) = resolve_normalized_text(&slug_candidate)
    {
        return Ok(identity);
    }

    resolve_normalized_text(&normalized)
}

/// 解析规范化后的文本，提取主客队名称。
///
/// 此函数按顺序尝试两种解析策略：
///
/// ## 策略一："vs" 分隔符
///
/// 使用正则表达式匹配 `vs`、`vs.`、`v`、`v.` 等分隔符（不区分大小写），
/// 将文本分为前后两部分，分别作为主队和客队。
/// 提取后通过 [`is_plausible_team`] 验证两个名称是否合理。
///
/// ## 策略二：" - " 分隔符
///
/// 从右侧查找 `" - "` 分隔符（使用 `rsplit_once`），将文本分为主队部分和客队部分。
/// 对两侧分别进行上下文清理（去除前缀、后缀等无关信息），
/// 并通过多项检查确保解析结果的合理性：
/// - 主客队名称必须是合理的队伍名（[`is_plausible_team`]）
/// - 不能包含通用页面术语（如 "odds"、"betting" 等）
/// - 不能是上下文术语（如 "football"、"league" 等）
///
/// # 参数
///
/// * `normalized` - 已规范化（HTML 实体已解码）的文本
///
/// # 返回值
///
/// 成功时返回 [`MatchIdentity`]，两种策略均失败时返回错误。
fn resolve_normalized_text(normalized: &str) -> Result<MatchIdentity> {
    let versus_re = Regex::new(r"(?i)\s+(?:vs\.?|v\.?)\s+")?;
    if let Some(separator) = versus_re.find(normalized) {
        let home = clean_team(strip_prefix_context(&normalized[..separator.start()]));
        let away = clean_team(&normalized[separator.end()..]);
        if is_plausible_team(&home) && is_plausible_team(&away) {
            return Ok(match_identity(home, away));
        }
    }

    if let Some((home_part, away_part)) = normalized.rsplit_once(" - ") {
        let home = clean_team(strip_dash_home_context(home_part));
        let away = clean_team(&strip_dash_away_suffixes(away_part)?);
        if is_plausible_team(&home)
            && is_plausible_team(&away)
            && !contains_generic_page_term(&home)
            && !contains_generic_page_term(&away)
            && !is_dash_context_term(&home)
            && !is_dash_context_term(&away)
        {
            return Ok(match_identity(home, away));
        }
    }

    bail!("could not resolve teams from text")
}

/// 根据主队和客队名称生成唯一的比赛 ID。
///
/// 生成的 ID 格式为 `{slugified_home}_vs_{slugified_away}`，
/// 其中队伍名称会通过 [`slugify`] 函数转换为小写 slug 格式（如 `Los Angeles Lakers` → `los_angeles_lakers`）。
///
/// # 参数
///
/// * `home` - 主队名称
/// * `away` - 客队名称
///
/// # 示例
///
/// ```
/// // "Lakers" vs "Celtics" → "lakers_vs_celtics"
/// ```
pub fn match_id_for(home: &str, away: &str) -> String {
    format!(
        "{}_vs_{}",
        slugify(&canonical_team_name(home)),
        slugify(&canonical_team_name(away))
    )
}

/// 将队伍名称规范化为用于展示和匹配的稳定名称。
///
/// 目前主要移除 Polymarket/esports 标题中常见的赛制后缀，例如 `(BO3)`、
/// `BO5` 和 `Best of 3`，避免同一场比赛被存成两个 match_id。
pub fn canonical_team_name(value: &str) -> String {
    strip_match_format_suffix(value)
}

/// 清理队伍名称，去除首尾空白及常见的装饰性字符。
///
/// 具体处理步骤：
/// 1. 去除首尾空白
/// 2. 去除首尾的引号、冒号、逗号、连字符、竖线等装饰性字符
/// 3. 如果遇到 `" - "` 分隔符，只保留分隔符之前的部分
/// 4. 再次去除首尾空白
///
/// # 参数
///
/// * `value` - 待清理的队伍名称
fn clean_team(value: &str) -> String {
    let cleaned = value
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | ':' | ',' | '-' | '|'))
        .split(" - ")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string();
    canonical_team_name(&cleaned)
}

fn strip_match_format_suffix(value: &str) -> String {
    static PAREN_FORMAT_RE: OnceLock<Regex> = OnceLock::new();
    static TRAILING_FORMAT_RE: OnceLock<Regex> = OnceLock::new();

    let paren_re = PAREN_FORMAT_RE.get_or_init(|| {
        Regex::new(r"(?i)\s*[\(\[]\s*(?:bo|best\s+of)\s*\d+\s*[\)\]]\s*$")
            .expect("valid match format suffix regex")
    });
    let trailing_re = TRAILING_FORMAT_RE.get_or_init(|| {
        Regex::new(r"(?i)\s+(?:bo|best\s+of)\s*\d+\s*$").expect("valid trailing match format regex")
    });

    let without_paren = paren_re.replace(value.trim(), "");
    trailing_re
        .replace(without_paren.trim(), "")
        .trim()
        .to_string()
}

/// 去除文本中冒号前的上下文前缀。
///
/// 例如：`"NBA: Lakers vs Celtics"` → `" Lakers vs Celtics"`
///
/// # 参数
///
/// * `value` - 待处理的文本
///
/// # 返回值
///
/// 如果包含冒号，返回冒号之后的部分；否则返回原始文本。
fn strip_prefix_context(value: &str) -> &str {
    value
        .rsplit_once(':')
        .map(|(_, team)| team)
        .unwrap_or(value)
}

/// 清理主队部分的上下文信息。
///
/// 依次尝试去除以下前缀：
/// 1. 冒号（`:`）之前的内容
/// 2. 逗号（`,`）之前的内容
/// 3. 最后一个 `" - "` 之前的内容（取 `rsplit_once` 的后半部分）
///
/// # 参数
///
/// * `value` - 主队部分的原始文本
fn strip_dash_home_context(value: &str) -> &str {
    let after_colon = strip_prefix_context(value);
    let after_comma = after_colon
        .rsplit_once(',')
        .map(|(_, team)| team)
        .unwrap_or(after_colon);
    after_comma
        .rsplit_once(" - ")
        .map(|(_, team)| team)
        .unwrap_or(after_comma)
}

/// 清理客队部分的后缀信息。
///
/// 处理步骤：
/// 1. 去除竖线（`|`）之后的内容
/// 2. 去除常见的后缀关键词，如 `odds`、`predictions`、`h2h` 等
///
/// # 参数
///
/// * `value` - 客队部分的原始文本
fn strip_dash_away_suffixes(value: &str) -> Result<String> {
    let before_pipe = value.split('|').next().unwrap_or(value);
    let suffix_re = Regex::new(r"(?i)\s+(?:odds|predictions|h2h)\b")?;
    Ok(suffix_re
        .find(before_pipe)
        .map(|suffix| &before_pipe[..suffix.start()])
        .unwrap_or(before_pipe)
        .to_string())
}

/// 构建并返回 [`MatchIdentity`] 对象。
///
/// 内部调用 [`match_id_for`] 生成 match_id，并将主队、客队名称封装到结构体中。
///
/// # 参数
///
/// * `home` - 主队名称
/// * `away` - 客队名称
fn match_identity(home: String, away: String) -> MatchIdentity {
    MatchIdentity {
        match_id: match_id_for(&home, &away),
        home_team: home,
        away_team: away,
        match_time: None,
    }
}

/// 判断一个字符串是否是合理的队伍名称。
///
/// 验证条件：
/// - 字符长度在 2 到 60 之间
/// - 至少包含一个字母字符
///
/// # 参数
///
/// * `value` - 待验证的字符串
fn is_plausible_team(value: &str) -> bool {
    let len = value.chars().count();
    (2..=60).contains(&len) && value.chars().any(|c| c.is_alphabetic())
}

/// 检查字符串是否包含通用页面术语（非队伍名称的关键词）。
///
/// 这些术语通常出现在网页标题或 URL 中，但不代表具体的队伍名称，
/// 例如 `"oddsportal"`、`"odds"`、`"betting"`、`"live scores"` 等。
///
/// # 参数
///
/// * `value` - 待检查的字符串
fn contains_generic_page_term(value: &str) -> bool {
    let lower = value.to_lowercase();
    [
        "oddsportal",
        "odds",
        "betting",
        "live scores",
        "football betting odds",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

/// 检查字符串是否是常见的上下文术语（联赛、运动类型等）。
///
/// 这些术语通常是比赛的分类信息而非具体队伍名称，
/// 例如 `"football"`、`"premier league"`、`"championship"` 等。
///
/// # 参数
///
/// * `value` - 待检查的字符串
fn is_dash_context_term(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "football"
            | "england"
            | "championship"
            | "league"
            | "premier league"
            | "scores"
            | "standings"
    )
}

/// 尝试从文本中提取 URL slug 格式的候选比赛信息。
///
/// 此函数从 URL 路径的最后一个片段中查找包含 `-vs-` 或 `-v-` 的 slug，
/// 并将其转换为 `"Home vs Away"` 的格式。
///
/// 处理流程：
/// 1. 去除末尾的斜杠，按 `/`、`?`、`#` 分割，找到包含 `vs` 或 `v` 的片段
/// 2. 按 `-` 分割为 token 列表
/// 3. 找到 `vs` 或 `v` 分隔符的位置
/// 4. 对分隔符前的 token 进行首字母大写处理，作为主队名称
/// 5. 对分隔符后的 token 去除可能的数字后缀后，进行首字母大写处理，作为客队名称
/// 6. 拼接为 `"Home vs Away"` 格式
///
/// # 参数
///
/// * `value` - 待处理的文本
fn url_slug_match_candidate(value: &str) -> Option<String> {
    let segment = value
        .trim_end_matches('/')
        .rsplit(['/', '?', '#'])
        .find(|segment| {
            let lower = segment.to_lowercase();
            lower.contains("-vs-") || lower.contains("-v-")
        })?;
    let tokens: Vec<&str> = segment
        .split('-')
        .filter(|token| !token.is_empty())
        .collect();
    let separator_index = tokens
        .iter()
        .position(|token| matches!(token.to_ascii_lowercase().as_str(), "vs" | "v"))?;
    if separator_index == 0 || separator_index + 1 >= tokens.len() {
        return None;
    }

    let home = title_case_slug_tokens(&tokens[..separator_index]);
    let away_tokens = strip_slug_suffix_tokens(&tokens[separator_index + 1..]);
    let away = title_case_slug_tokens(away_tokens);
    if home.is_empty() || away.is_empty() {
        return None;
    }

    Some(format!(
        "{} {} {}",
        home,
        tokens[separator_index].to_ascii_lowercase(),
        away
    ))
}

/// 去除 slug token 列表中可能存在的数字后缀 token。
///
/// 在 URL slug 中，客队名称后有时会跟随数字（如赛季年份或比分），
/// 此函数检查最后一个 token 是否包含数字，如果是则将其去除。
///
/// # 参数
///
/// * `tokens` - slug 的 token 切片
///
/// # 返回值
///
/// 如果最后一个 token 包含数字，返回去除该 token 后的切片；否则返回原始切片。
fn strip_slug_suffix_tokens<'a>(tokens: &'a [&'a str]) -> &'a [&'a str] {
    let Some((last, rest)) = tokens.split_last() else {
        return tokens;
    };

    if last.chars().any(|ch| ch.is_ascii_digit()) {
        rest
    } else {
        tokens
    }
}

/// 将 slug token 列表转换为首字母大写的字符串，并用空格连接。
///
/// 例如：`["lakers", "vs"]` → `"Lakers Vs"`
///
/// # 参数
///
/// * `tokens` - 待转换的 token 切片
fn title_case_slug_tokens(tokens: &[&str]) -> String {
    tokens
        .iter()
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
        .join(" ")
}

/// 将字符串转换为 slug 格式（小写、下划线分隔）。
///
/// 处理规则：
/// - 将所有字符转换为小写
/// - 保留字母数字字符
/// - 非字母数字字符转换为下划线，且连续的非字母数字字符只生成一个下划线
/// - 去除首尾的下划线
///
/// # 参数
///
/// * `value` - 待转换的字符串
fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in value.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// 将 HTML 实体解码为对应的字符。
///
/// 支持的实体包括：
/// - `&amp;` → `&`
/// - `&nbsp;` → 空格
/// - `&#160;` → 空格（不换行空格的数字实体引用）
///
/// # 参数
///
/// * `value` - 包含 HTML 实体的原始字符串
fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
}
