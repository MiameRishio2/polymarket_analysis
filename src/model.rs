use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchIdentity {
    pub match_id: String,
    pub home_team: String,
    pub away_team: String,
    pub match_time: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BookmakerOdds {
    pub bookmaker: String,
    pub home: f64,
    pub draw: f64,
    pub away: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolymarketPrice {
    pub market_id: Option<String>,
    pub market_title: String,
    pub outcome: String,
    pub price: f64,
    pub volume: Option<f64>,
    pub active: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParseStatus {
    Parsed,
    Empty,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub id: i64,
    pub match_id: String,
    pub source: String,
    pub collected_at: DateTime<Utc>,
    pub http_status: Option<i64>,
    pub parse_status: ParseStatus,
    pub raw_hash: Option<String>,
    pub raw_artifact_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProviderPayload {
    Polymarket { prices: Vec<PolymarketPrice> },
    OddsPortal { odds: Vec<BookmakerOdds> },
}
