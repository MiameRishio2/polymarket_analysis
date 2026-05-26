use anyhow::{Result, bail};
use regex::Regex;

use crate::model::MatchIdentity;

pub fn resolve_from_text(text: &str) -> Result<MatchIdentity> {
    let normalized = html_unescape(text).replace('\u{a0}', " ");

    let versus_re = Regex::new(r"(?i)\s+(?:vs\.?|v\.?)\s+")?;
    if let Some(separator) = versus_re.find(&normalized) {
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
        {
            return Ok(match_identity(home, away));
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

fn strip_prefix_context(value: &str) -> &str {
    value
        .rsplit_once(':')
        .map(|(_, team)| team)
        .unwrap_or(value)
}

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

fn strip_dash_away_suffixes(value: &str) -> Result<String> {
    let before_pipe = value.split('|').next().unwrap_or(value);
    let suffix_re = Regex::new(r"(?i)\s+(?:odds|predictions|h2h)\b")?;
    Ok(suffix_re
        .find(before_pipe)
        .map(|suffix| &before_pipe[..suffix.start()])
        .unwrap_or(before_pipe)
        .to_string())
}

fn match_identity(home: String, away: String) -> MatchIdentity {
    MatchIdentity {
        match_id: match_id_for(&home, &away),
        home_team: home,
        away_team: away,
        match_time: None,
    }
}

fn is_plausible_team(value: &str) -> bool {
    let len = value.chars().count();
    (2..=60).contains(&len) && value.chars().any(|c| c.is_alphabetic())
}

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
