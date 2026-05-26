use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::model::{MatchIdentity, ProviderPayload};

pub mod oddsportal;
pub mod polymarket;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderTarget {
    pub url: String,
    pub identity: Option<MatchIdentity>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderSnapshot {
    pub source: &'static str,
    pub collected_at: DateTime<Utc>,
    pub http_status: Option<u16>,
    pub identity: Option<MatchIdentity>,
    pub payload: ProviderPayload,
    pub raw_body: Option<String>,
}

pub trait Provider {
    fn source_name(&self) -> &'static str;

    fn fetch_snapshot(
        &self,
        target: &ProviderTarget,
    ) -> impl Future<Output = Result<ProviderSnapshot>> + Send;
}
