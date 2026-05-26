use anyhow::Result;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

use crate::cli::CollectArgs;

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

    if let Some(url) = args.polymarket_url {
        let schedule = ScheduledProvider::new(
            "polymarket",
            args.polymarket_interval_seconds,
            BackoffPolicy::polymarket(),
        );
        tasks.spawn(run_placeholder_collection_loop(schedule, url));
    }

    if let Some(url) = args.odds_url {
        let schedule = ScheduledProvider::new(
            "oddsportal",
            args.odds_interval_seconds,
            BackoffPolicy::oddsportal(args.odds_interval_seconds),
        );
        tasks.spawn(run_placeholder_collection_loop(schedule, url));
    }

    while let Some(result) = tasks.join_next().await {
        result?;
    }

    Ok(())
}

async fn run_placeholder_collection_loop(mut schedule: ScheduledProvider, url: String) {
    loop {
        info!(
            provider = %schedule.name,
            url = %url,
            "collection tick"
        );
        schedule.record_success();
        sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
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
