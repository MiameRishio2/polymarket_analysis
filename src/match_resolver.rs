use anyhow::{Result, bail};
use regex::Regex;

use crate::model::MatchIdentity;

pub fn resolve_from_text(text: &str) -> Result<MatchIdentity> {
    let normalized = html_unescape(text).replace('\u{a0}', " ");
    let patterns = [
        r"(?i)\b(.+?)\s+vs\.?\s+(.+?)\b",
        r"(?i)\b(.+?)\s+v\.?\s+(.+?)\b",
        r"\b(.+?)\s+-\s+(.+?)\b",
    ];

    for pattern in patterns {
        let re = Regex::new(pattern)?;
        if let Some(captures) = re.captures(&normalized) {
            let home = clean_team(captures.get(1).unwrap().as_str());
            let away = clean_team(captures.get(2).unwrap().as_str());
            if is_plausible_team(&home) && is_plausible_team(&away) {
                return Ok(MatchIdentity {
                    match_id: match_id_for(&home, &away),
                    home_team: home,
                    away_team: away,
                    match_time: None,
                });
            }
        }
    }

    bail!("could not resolve teams from text")
}

pub fn match_id_for(home: &str, away: &str) -> String {
    format!("{}_vs_{}", slugify(home), slugify(away))
}

fn clean_team(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c: char| matches!(c, '"' | '\'' | ':' | ',' | '-' | '|'))
        .split(" - ")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn is_plausible_team(value: &str) -> bool {
    let len = value.chars().count();
    (2..=60).contains(&len) && value.chars().any(|c| c.is_alphabetic())
}

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

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
}
