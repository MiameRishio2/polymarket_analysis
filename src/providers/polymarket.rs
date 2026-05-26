use anyhow::Result;
use chrono::Utc;
use serde_json::Value;

use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, PolymarketPrice, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

pub struct PolymarketProvider {
    client: reqwest::Client,
}

impl PolymarketProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
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
            .or_else(|| target.identity.clone());
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
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        for key in ["question", "title", "marketTitle"] {
            if let Some(text) = value.get(key).and_then(Value::as_str)
                && let Ok(identity) = resolve_from_text(text)
            {
                return Ok(identity);
            }
        }
    }

    resolve_from_text(body)
}

pub fn parse_polymarket_market(body: &str) -> Result<Vec<PolymarketPrice>> {
    let value: Value = serde_json::from_str(body)?;
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

    Ok(outcomes
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
        .collect())
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
