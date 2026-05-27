use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

use crate::cli::CollectArgs;
use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, ParseStatus, ProviderPayload};
use crate::providers::oddsportal::OddsPortalProvider;
use crate::providers::polymarket::PolymarketProvider;
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};
use crate::storage::{
    connect_sqlite, insert_failed_snapshot, insert_match, insert_oddsportal_snapshot,
    insert_polymarket_snapshot,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackoffPolicy {
    Sequence {
        base: u64,
        steps: Vec<u64>,
        cap: u64,
    },
    Doubling {
        base: u64,
        cap: u64,
    },
}

impl BackoffPolicy {
    pub fn polymarket() -> Self {
        Self::Sequence {
            base: 1,
            steps: vec![2, 5, 10],
            cap: 60,
        }
    }

    pub fn oddsportal(base: u64) -> Self {
        Self::Doubling { base, cap: 300 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledProvider {
    name: String,
    base_interval_seconds: u64,
    policy: BackoffPolicy,
    failures: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectionAttempt {
    identity: MatchIdentity,
    should_backoff: bool,
}

impl ScheduledProvider {
    pub fn new(name: impl Into<String>, base_interval_seconds: u64, policy: BackoffPolicy) -> Self {
        Self {
            name: name.into(),
            base_interval_seconds,
            policy,
            failures: 0,
        }
    }

    pub fn current_delay_seconds(&self) -> u64 {
        let policy_delay = if self.failures == 0 {
            self.base_interval_seconds
        } else {
            match &self.policy {
                BackoffPolicy::Sequence { base, steps, cap } => {
                    let step_index = self.failures.saturating_sub(1) as usize;
                    steps
                        .get(step_index)
                        .copied()
                        .unwrap_or(*cap)
                        .max(*base)
                        .min(*cap)
                }
                BackoffPolicy::Doubling { base, cap } => doubling_delay(*base, *cap, self.failures),
            }
        };

        policy_delay.max(self.base_interval_seconds)
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
    }

    pub fn record_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        warn!(
            provider = %self.name,
            failures = self.failures,
            next_delay_seconds = self.current_delay_seconds(),
            "provider collection failed"
        );
    }
}

pub async fn collect(args: CollectArgs) -> Result<()> {
    let mut tasks = JoinSet::new();
    let db_url = format!("sqlite://{}", args.db.display());
    let pool = connect_sqlite(&db_url).await?;
    let canonical_identity = resolve_collect_identity(&args)?;

    if let Some(url) = args.polymarket_url {
        let schedule = ScheduledProvider::new(
            "polymarket",
            args.polymarket_interval_seconds,
            BackoffPolicy::polymarket(),
        );
        tasks.spawn(run_provider_collection_loop(
            schedule,
            ProviderTarget {
                url,
                identity: Some(canonical_identity.clone()),
            },
            PolymarketProvider::new(),
            pool.clone(),
        ));
    }

    if let Some(url) = args.odds_url {
        let schedule = ScheduledProvider::new(
            "oddsportal",
            args.odds_interval_seconds,
            BackoffPolicy::oddsportal(args.odds_interval_seconds),
        );
        tasks.spawn(run_provider_collection_loop(
            schedule,
            ProviderTarget {
                url,
                identity: Some(canonical_identity.clone()),
            },
            OddsPortalProvider::new(),
            pool.clone(),
        ));
    }

    while let Some(result) = tasks.join_next().await {
        result?;
    }

    Ok(())
}

fn resolve_collect_identity(args: &CollectArgs) -> Result<MatchIdentity> {
    for url in [&args.polymarket_url, &args.odds_url].into_iter().flatten() {
        if let Ok(identity) = resolve_from_text(url) {
            return Ok(identity);
        }
    }

    bail!("could not resolve match identity from configured provider URL before collection")
}

async fn run_provider_collection_loop<P>(
    mut schedule: ScheduledProvider,
    target: ProviderTarget,
    provider: P,
    pool: SqlitePool,
) where
    P: Provider + Send + Sync + 'static,
{
    let mut last_identity = target.identity.clone();

    loop {
        info!(
            provider = %schedule.name,
            url = %target.url,
            "collection tick"
        );

        match collect_once(&provider, &target, &pool).await {
            Ok(attempt) => {
                last_identity = Some(attempt.identity);
                if attempt.should_backoff {
                    schedule.record_failure();
                } else {
                    schedule.record_success();
                }
            }
            Err(error) => {
                warn!(
                    provider = %schedule.name,
                    url = %target.url,
                    error = %error,
                    known_match_id = last_identity.as_ref().map(|identity| identity.match_id.as_str()),
                    "provider collection attempt failed"
                );
                if let Some(identity) = &last_identity
                    && let Err(storage_error) = insert_failed_snapshot(
                        &pool,
                        &identity.match_id,
                        provider.source_name(),
                        Utc::now(),
                        None,
                        &error.to_string(),
                    )
                    .await
                {
                    warn!(
                        provider = %schedule.name,
                        url = %target.url,
                        match_id = %identity.match_id,
                        error = %storage_error,
                        "failed to store provider failure snapshot"
                    );
                }
                schedule.record_failure();
            }
        }

        sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
    }
}

async fn collect_once<P>(
    provider: &P,
    target: &ProviderTarget,
    pool: &SqlitePool,
) -> Result<CollectionAttempt>
where
    P: Provider + Send + Sync,
{
    let snapshot = provider.fetch_snapshot(target).await?;
    write_snapshot(pool, target, snapshot).await
}

async fn write_snapshot(
    pool: &SqlitePool,
    target: &ProviderTarget,
    snapshot: ProviderSnapshot,
) -> Result<CollectionAttempt> {
    let identity = snapshot.identity.ok_or_else(|| {
        anyhow!(
            "{} snapshot did not include match identity",
            snapshot.source
        )
    })?;

    insert_match(pool, &identity, snapshot.source, Some(&target.url)).await?;

    let http_status = snapshot.http_status.map(i64::from);
    let status_error_message = snapshot.http_status.and_then(http_status_error_message);
    let is_empty_payload = match snapshot.payload {
        ProviderPayload::Polymarket { prices } => {
            let is_empty_payload = prices.is_empty();
            let parse_status =
                snapshot_parse_status(status_error_message.is_some(), is_empty_payload);
            insert_polymarket_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                status_error_message.as_deref(),
                &prices,
            )
            .await?;
            is_empty_payload
        }
        ProviderPayload::OddsPortal { odds } => {
            let is_empty_payload = odds.is_empty();
            let parse_status =
                snapshot_parse_status(status_error_message.is_some(), is_empty_payload);
            insert_oddsportal_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                status_error_message.as_deref(),
                &odds,
            )
            .await?;
            is_empty_payload
        }
    };

    Ok(CollectionAttempt {
        should_backoff: should_backoff_after_snapshot(
            status_error_message.is_some(),
            is_empty_payload,
        ),
        identity,
    })
}

fn snapshot_parse_status(has_http_status_error: bool, is_empty: bool) -> ParseStatus {
    if has_http_status_error {
        ParseStatus::Failed
    } else if is_empty {
        ParseStatus::Empty
    } else {
        ParseStatus::Parsed
    }
}

fn should_backoff_after_snapshot(has_http_status_error: bool, is_empty_payload: bool) -> bool {
    has_http_status_error || is_empty_payload
}

fn http_status_error_message(status: u16) -> Option<String> {
    if !(200..300).contains(&status) {
        Some(format!("http status {status}"))
    } else {
        None
    }
}

fn doubling_delay(base: u64, cap: u64, failures: u32) -> u64 {
    let mut delay = base;

    for _ in 0..failures {
        delay = delay.saturating_mul(2).min(cap);
        if delay == cap {
            break;
        }
    }

    delay
}

#[cfg(test)]
mod tests {
    use super::{
        http_status_error_message, resolve_collect_identity, should_backoff_after_snapshot,
        snapshot_parse_status,
    };
    use crate::cli::CollectArgs;
    use crate::model::ParseStatus;
    use std::path::PathBuf;

    #[test]
    fn empty_payload_triggers_backoff_without_marking_parse_failed() {
        assert_eq!(snapshot_parse_status(false, true), ParseStatus::Empty);
        assert!(should_backoff_after_snapshot(false, true));
    }

    #[test]
    fn non_success_http_status_is_failed_and_triggers_backoff() {
        assert_eq!(
            http_status_error_message(429),
            Some("http status 429".to_string())
        );
        assert_eq!(snapshot_parse_status(true, false), ParseStatus::Failed);
        assert!(should_backoff_after_snapshot(true, false));
    }

    #[test]
    fn parsed_success_snapshot_does_not_backoff() {
        assert_eq!(http_status_error_message(200), None);
        assert_eq!(snapshot_parse_status(false, false), ParseStatus::Parsed);
        assert!(!should_backoff_after_snapshot(false, false));
    }

    #[test]
    fn collect_requires_resolvable_identity_before_looping() {
        let args = CollectArgs {
            polymarket_url: Some("https://polymarket.com/event/southampton-vs-wrexham".to_string()),
            odds_url: Some(
                "https://www.oddsportal.com/football/england/championship/wrexham-vs-southampton/"
                    .to_string(),
            ),
            polymarket_interval_seconds: 1,
            odds_interval_seconds: 60,
            db: PathBuf::from("unused.sqlite"),
        };

        let identity = resolve_collect_identity(&args).unwrap();

        assert_eq!(identity.match_id, "southampton_vs_wrexham");
    }
}
