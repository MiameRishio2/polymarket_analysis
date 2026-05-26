use anyhow::{Result, anyhow};
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

use crate::cli::CollectArgs;
use crate::model::{ParseStatus, ProviderPayload};
use crate::providers::oddsportal::OddsPortalProvider;
use crate::providers::polymarket::PolymarketProvider;
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};
use crate::storage::{
    connect_sqlite, insert_match, insert_oddsportal_snapshot, insert_polymarket_snapshot,
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
                identity: None,
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
                identity: None,
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

async fn run_provider_collection_loop<P>(
    mut schedule: ScheduledProvider,
    target: ProviderTarget,
    provider: P,
    pool: SqlitePool,
) where
    P: Provider + Send + Sync + 'static,
{
    loop {
        info!(
            provider = %schedule.name,
            url = %target.url,
            "collection tick"
        );

        match collect_once(&provider, &target, &pool).await {
            Ok(()) => schedule.record_success(),
            Err(error) => {
                warn!(
                    provider = %schedule.name,
                    error = %error,
                    "provider collection attempt failed"
                );
                schedule.record_failure();
            }
        }

        sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
    }
}

async fn collect_once<P>(provider: &P, target: &ProviderTarget, pool: &SqlitePool) -> Result<()>
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
) -> Result<()> {
    let identity = snapshot.identity.ok_or_else(|| {
        anyhow!(
            "{} snapshot did not include match identity",
            snapshot.source
        )
    })?;

    insert_match(pool, &identity, snapshot.source, Some(&target.url)).await?;

    let http_status = snapshot.http_status.map(i64::from);
    match snapshot.payload {
        ProviderPayload::Polymarket { prices } => {
            let parse_status = payload_parse_status(prices.is_empty());
            insert_polymarket_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                None,
                &prices,
            )
            .await?;
        }
        ProviderPayload::OddsPortal { odds } => {
            let parse_status = payload_parse_status(odds.is_empty());
            insert_oddsportal_snapshot(
                pool,
                &identity.match_id,
                snapshot.collected_at,
                http_status,
                parse_status,
                None,
                &odds,
            )
            .await?;
        }
    }

    Ok(())
}

fn payload_parse_status(is_empty: bool) -> ParseStatus {
    if is_empty {
        ParseStatus::Empty
    } else {
        ParseStatus::Parsed
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
