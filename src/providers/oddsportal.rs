use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use reqwest::header::USER_AGENT;
use scraper::{Html, Selector};

use crate::match_resolver::resolve_from_text;
use crate::model::{BookmakerOdds, MatchIdentity, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

const ODDSPORTAL_USER_AGENT: &str = "polymarket-analysis/0.1";

pub struct OddsPortalProvider {
    client: reqwest::Client,
}

impl OddsPortalProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for OddsPortalProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for OddsPortalProvider {
    fn source_name(&self) -> &'static str {
        "oddsportal"
    }

    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot> {
        let response = self
            .client
            .get(&target.url)
            .header(USER_AGENT, ODDSPORTAL_USER_AGENT)
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let identity = extract_oddsportal_match_identity(&body)
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

        let odds = parse_oddsportal_odds(&body)?;

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

pub fn extract_oddsportal_match_identity(body: &str) -> Result<MatchIdentity> {
    let document = Html::parse_document(body);

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

pub fn parse_oddsportal_odds(html: &str) -> Result<Vec<BookmakerOdds>> {
    let odds = parse_data_odd_rows(html)?;
    if !odds.is_empty() {
        return Ok(odds);
    }

    parse_table_like_rows(html)
}

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

fn resolve_first_candidate(candidates: Vec<String>) -> Option<MatchIdentity> {
    candidates
        .into_iter()
        .find_map(|candidate| resolve_from_text(&decode_jsonish(&candidate)).ok())
}

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

fn parse_decimal_odd(value: &str) -> Option<f64> {
    let cleaned = value
        .trim()
        .trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '('));
    let value = cleaned.parse::<f64>().ok()?;
    (value > 1.0 && value < 100.0).then_some(value)
}

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

fn clean_label(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
