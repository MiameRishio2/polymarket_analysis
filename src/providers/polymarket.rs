use anyhow::Result;
use chrono::Utc;
use serde_json::Value;

use crate::http::build_http_client;
use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, PolymarketPrice, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

pub struct PolymarketProvider {
    client: reqwest::Client,
}

impl PolymarketProvider {
    pub fn new() -> Self {
        Self {
            client: build_http_client().expect("failed to build Polymarket HTTP client"),
        }
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for PolymarketProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for PolymarketProvider {
    fn source_name(&self) -> &'static str {
        "polymarket"
    }

    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot> {
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

        let prices = parse_polymarket_market(&body)?;

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

fn polymarket_title_candidates(value: &Value) -> Vec<String> {
    let mut titles = Vec::new();
    collect_title_candidates(value, &mut titles);
    titles
}

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

fn read_string_array(value: Option<&Value>) -> Vec<String> {
    read_array(value, |value| value.as_str().map(ToOwned::to_owned))
}

fn read_f64_array(value: Option<&Value>) -> Vec<f64> {
    read_array(value, read_optional_f64)
}

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

fn read_optional_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse::<f64>().ok()))
}
