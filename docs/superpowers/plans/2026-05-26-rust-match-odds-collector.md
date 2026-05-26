# Rust Match Odds Collector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Go prototype with a Rust CLI that resolves match teams from web data, collects Polymarket every 1 second, collects OddsPortal on a configurable slower interval, stores every attempt in SQLite, and exports retained match data.

**Architecture:** Build a Rust binary plus library crate. Keep CLI, provider parsing, match resolution, storage, collection scheduling, and export logic in separate modules so the first CLI can later grow into a service without rewriting the core.

**Tech Stack:** Rust 1.95, `clap`, `tokio`, `reqwest`, `serde`, `serde_json`, `sqlx` with SQLite, `chrono`, `anyhow`, `thiserror`, `regex`, `scraper`, `csv`, `tempfile`.

---

## File Structure

- Create `Cargo.toml`: Rust package metadata and dependencies.
- Create `src/main.rs`: Tokio entrypoint that delegates to CLI handling.
- Create `src/lib.rs`: Exposes modules for integration tests.
- Create `src/cli.rs`: Command definitions and command dispatch.
- Create `src/model.rs`: Shared domain structs and enums.
- Create `src/match_resolver.rs`: Team-name parsing, normalization, and `match_id` generation.
- Create `src/providers/mod.rs`: Provider trait and shared provider types.
- Create `src/providers/oddsportal.rs`: OddsPortal parsing and HTTP fetch implementation.
- Create `src/providers/polymarket.rs`: Polymarket parsing and HTTP fetch implementation.
- Create `src/storage.rs`: SQLite schema creation, inserts, queries, and exports.
- Create `src/collector.rs`: Independent provider scheduling and backoff logic.
- Create `tests/fixtures/`: Fixture copies of current captured HTML, text, and JSON files.
- Create `tests/resolver_tests.rs`: Match resolver tests.
- Create `tests/oddsportal_tests.rs`: OddsPortal fixture parsing tests.
- Create `tests/storage_tests.rs`: SQLite append and export tests.
- Create `tests/collector_tests.rs`: Scheduler/backoff tests using deterministic time inputs.
- Modify `README.md`: Rust usage instructions.
- Remove or stop using `main.go`, `go.mod`, and `polymarket-analysis` only after Rust tests pass.

### Task 1: Rust Crate Skeleton And Fixture Preservation

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`
- Create: `src/model.rs`
- Create: `tests/fixtures/`
- Copy fixtures from root into `tests/fixtures/`

- [ ] **Step 1: Create crate manifest**

Create `Cargo.toml`:

```toml
[package]
name = "polymarket-analysis"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive"] }
csv = "1"
regex = "1"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
scraper = "0.24"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite", "chrono"] }
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
url = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create library module declarations**

Create `src/lib.rs`:

```rust
pub mod cli;
pub mod collector;
pub mod match_resolver;
pub mod model;
pub mod providers;
pub mod storage;
```

- [ ] **Step 3: Create binary entrypoint**

Create `src/main.rs`:

```rust
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    polymarket_analysis::cli::run().await
}
```

- [ ] **Step 4: Create initial CLI module**

Create `src/cli.rs`:

```rust
use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "polymarket-analysis")]
#[command(about = "Collect Polymarket and odds-site match data")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Collect(CollectArgs),
    Export(ExportArgs),
}

#[derive(Debug, Parser)]
pub struct CollectArgs {
    #[arg(long)]
    pub polymarket_url: Option<String>,
    #[arg(long)]
    pub odds_url: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub polymarket_interval_seconds: u64,
    #[arg(long, default_value_t = 60)]
    pub odds_interval_seconds: u64,
    #[arg(long, default_value = "data/polymarket_analysis.sqlite")]
    pub db: PathBuf,
}

#[derive(Debug, Parser)]
pub struct ExportArgs {
    #[arg(long)]
    pub db: PathBuf,
    #[arg(long)]
    pub match_id: String,
    #[arg(long)]
    pub format: ExportFormat,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    Jsonl,
    Csv,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Collect(args) => {
            if args.polymarket_url.is_none() && args.odds_url.is_none() {
                bail!("at least one of --polymarket-url or --odds-url is required");
            }
            if args.polymarket_interval_seconds == 0 {
                bail!("--polymarket-interval-seconds must be greater than 0");
            }
            if args.odds_interval_seconds == 0 {
                bail!("--odds-interval-seconds must be greater than 0");
            }
            crate::collector::collect(args).await
        }
        Command::Export(args) => crate::storage::export_match(args).await,
    }
}
```

- [ ] **Step 5: Create initial model module**

Create `src/model.rs`:

```rust
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
pub enum ProviderPayload {
    Polymarket { prices: Vec<PolymarketPrice> },
    OddsPortal { odds: Vec<BookmakerOdds> },
}
```

- [ ] **Step 6: Create empty implementation stubs required by imports**

Create `src/collector.rs`:

```rust
use anyhow::Result;

use crate::cli::CollectArgs;

pub async fn collect(_args: CollectArgs) -> Result<()> {
    anyhow::bail!("collection is not implemented yet")
}
```

Create `src/storage.rs`:

```rust
use anyhow::Result;

use crate::cli::ExportArgs;

pub async fn export_match(_args: ExportArgs) -> Result<()> {
    anyhow::bail!("export is not implemented yet")
}
```

Create `src/match_resolver.rs`:

```rust
use anyhow::Result;

use crate::model::MatchIdentity;

pub fn resolve_from_text(_text: &str) -> Result<MatchIdentity> {
    anyhow::bail!("match resolution is not implemented yet")
}
```

Create `src/providers/mod.rs`:

```rust
pub mod oddsportal;
pub mod polymarket;
```

Create `src/providers/oddsportal.rs`:

```rust
use anyhow::Result;

use crate::model::BookmakerOdds;

pub fn parse_oddsportal_odds(_html: &str) -> Result<Vec<BookmakerOdds>> {
    anyhow::bail!("oddsportal parsing is not implemented yet")
}
```

Create `src/providers/polymarket.rs`:

```rust
use anyhow::Result;

use crate::model::PolymarketPrice;

pub fn parse_polymarket_market(_body: &str) -> Result<Vec<PolymarketPrice>> {
    anyhow::bail!("polymarket parsing is not implemented yet")
}
```

- [ ] **Step 7: Preserve current captured data as fixtures**

Run:

```bash
mkdir -p tests/fixtures
cp oddsportal_debug.html tests/fixtures/oddsportal_debug.html
cp odds_portal.html tests/fixtures/odds_portal.html
cp odds_portal_ajax.html tests/fixtures/odds_portal_ajax.html
cp odds_portal_eventdata.html tests/fixtures/odds_portal_eventdata.html
cp odds_portal_eventdata_decoded.txt tests/fixtures/odds_portal_eventdata_decoded.txt
cp odds_portal_decoded.txt tests/fixtures/odds_portal_decoded.txt
cp match_data_southampton_vs_wrexham_20260408_224015.json tests/fixtures/match_data_southampton_vs_wrexham_20260408_224015.json
cp match_data_millwall_vs_west_brom_20260408_225019.json tests/fixtures/match_data_millwall_vs_west_brom_20260408_225019.json
```

- [ ] **Step 8: Run check**

Run:

```bash
cargo check
```

Expected: PASS after dependencies download and compilation.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml src tests/fixtures
git commit -m "chore: scaffold rust collector crate"
```

### Task 2: Match Resolution

**Files:**
- Modify: `src/match_resolver.rs`
- Test: `tests/resolver_tests.rs`

- [ ] **Step 1: Write failing resolver tests**

Create `tests/resolver_tests.rs`:

```rust
use polymarket_analysis::match_resolver::{match_id_for, resolve_from_text};

#[test]
fn resolves_vs_separator() {
    let identity = resolve_from_text("Southampton vs Wrexham").unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
    assert_eq!(identity.match_id, "southampton_vs_wrexham");
}

#[test]
fn resolves_dash_separator() {
    let identity = resolve_from_text("Wrexham - Southampton").unwrap();
    assert_eq!(identity.home_team, "Wrexham");
    assert_eq!(identity.away_team, "Southampton");
    assert_eq!(identity.match_id, "wrexham_vs_southampton");
}

#[test]
fn creates_ascii_like_match_id() {
    assert_eq!(match_id_for("West Brom", "Millwall"), "west_brom_vs_millwall");
}

#[test]
fn rejects_unresolvable_text() {
    let err = resolve_from_text("Championship odds and live scores").unwrap_err();
    assert!(err.to_string().contains("could not resolve teams"));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test resolver_tests
```

Expected: FAIL because `match_id_for` does not exist and resolver is still a stub.

- [ ] **Step 3: Implement resolver**

Replace `src/match_resolver.rs`:

```rust
use anyhow::{bail, Result};
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
```

- [ ] **Step 4: Run resolver tests**

Run:

```bash
cargo test --test resolver_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/match_resolver.rs tests/resolver_tests.rs
git commit -m "feat: resolve match teams from text"
```

### Task 3: OddsPortal Fixture Parsing

**Files:**
- Modify: `src/providers/oddsportal.rs`
- Test: `tests/oddsportal_tests.rs`

- [ ] **Step 1: Write failing OddsPortal parser tests**

Create `tests/oddsportal_tests.rs`:

```rust
use polymarket_analysis::match_resolver::resolve_from_text;
use polymarket_analysis::providers::oddsportal::{
    extract_oddsportal_match_identity,
    parse_oddsportal_odds,
};

#[test]
fn extracts_teams_from_decoded_event_data() {
    let body = include_str!("fixtures/odds_portal_eventdata_decoded.txt");
    let identity = extract_oddsportal_match_identity(body).unwrap();
    assert!(
        identity.home_team.contains("Southampton") || identity.home_team.contains("Wrexham")
    );
    assert!(
        identity.away_team.contains("Southampton") || identity.away_team.contains("Wrexham")
    );
    assert_ne!(identity.home_team, identity.away_team);
}

#[test]
fn fallback_resolver_handles_event_title() {
    let identity = resolve_from_text("Southampton vs Wrexham - Odds, Predictions and H2H Results")
        .unwrap();
    assert_eq!(identity.home_team, "Southampton");
    assert_eq!(identity.away_team, "Wrexham");
}

#[test]
fn parses_bookmaker_odds_from_fixture_or_returns_empty_cleanly() {
    let html = include_str!("fixtures/oddsportal_debug.html");
    let odds = parse_oddsportal_odds(html).unwrap();
    assert!(
        odds.is_empty() || odds.iter().all(|row| row.home > 1.0 && row.draw > 1.0 && row.away > 1.0)
    );
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test oddsportal_tests
```

Expected: FAIL because `extract_oddsportal_match_identity` does not exist and parser is a stub.

- [ ] **Step 3: Implement fixture parsing**

Replace `src/providers/oddsportal.rs`:

```rust
use anyhow::Result;
use regex::Regex;
use scraper::{Html, Selector};

use crate::match_resolver::resolve_from_text;
use crate::model::{BookmakerOdds, MatchIdentity};

pub fn extract_oddsportal_match_identity(body: &str) -> Result<MatchIdentity> {
    let event_patterns = [
        r#""eventOverviewH1Text"\s*:\s*"([^"]+)""#,
        r#""pageH1"\s*:\s*"([^"]+)""#,
        r#""event"\s*:\s*"([^"]+)""#,
        r#"<title>([^<]+)</title>"#,
        r#"<h1[^>]*>([^<]+)</h1>"#,
    ];

    for pattern in event_patterns {
        let re = Regex::new(pattern)?;
        for captures in re.captures_iter(body) {
            let candidate = decode_jsonish(captures.get(1).unwrap().as_str());
            if let Ok(identity) = resolve_from_text(&candidate) {
                return Ok(identity);
            }
        }
    }

    resolve_from_text(body)
}

pub fn parse_oddsportal_odds(html: &str) -> Result<Vec<BookmakerOdds>> {
    let mut parsed = parse_data_odd_rows(html)?;
    if parsed.is_empty() {
        parsed = parse_table_like_rows(html)?;
    }
    Ok(parsed)
}

fn parse_data_odd_rows(html: &str) -> Result<Vec<BookmakerOdds>> {
    let bookmaker_re = Regex::new(r#"class="[^"]*bookmaker-name[^"]*"[^>]*>([^<]+)"#)?;
    let odds_re = Regex::new(r#"data-odd="([0-9]+(?:\.[0-9]+)?)""#)?;
    let bookmakers: Vec<String> = bookmaker_re
        .captures_iter(html)
        .map(|cap| clean_label(cap.get(1).unwrap().as_str()))
        .collect();
    let odds: Vec<f64> = odds_re
        .captures_iter(html)
        .filter_map(|cap| cap.get(1).unwrap().as_str().parse::<f64>().ok())
        .collect();

    let mut rows = Vec::new();
    for (idx, bookmaker) in bookmakers.iter().enumerate() {
        let offset = idx * 3;
        if offset + 2 >= odds.len() {
            break;
        }
        rows.push(BookmakerOdds {
            bookmaker: bookmaker.clone(),
            home: odds[offset],
            draw: odds[offset + 1],
            away: odds[offset + 2],
        });
    }
    Ok(rows)
}

fn parse_table_like_rows(html: &str) -> Result<Vec<BookmakerOdds>> {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("div, tr").unwrap();
    let number_re = Regex::new(r"\b[1-9][0-9]?\.[0-9]{1,3}\b")?;
    let mut rows = Vec::new();

    for row in document.select(&row_selector) {
        let text = row.text().collect::<Vec<_>>().join(" ");
        let nums: Vec<f64> = number_re
            .find_iter(&text)
            .filter_map(|m| m.as_str().parse::<f64>().ok())
            .collect();
        if nums.len() >= 3 {
            let bookmaker = text
                .split_whitespace()
                .next()
                .map(clean_label)
                .unwrap_or_else(|| "unknown".to_string());
            rows.push(BookmakerOdds {
                bookmaker,
                home: nums[0],
                draw: nums[1],
                away: nums[2],
            });
        }
    }

    rows.dedup_by(|a, b| a.bookmaker == b.bookmaker && a.home == b.home && a.draw == b.draw && a.away == b.away);
    Ok(rows)
}

fn decode_jsonish(value: &str) -> String {
    value
        .replace("\\/", "/")
        .replace("\\\"", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
}

fn clean_label(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

- [ ] **Step 4: Run OddsPortal tests**

Run:

```bash
cargo test --test oddsportal_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/oddsportal.rs tests/oddsportal_tests.rs
git commit -m "feat: parse oddsportal fixtures"
```

### Task 4: SQLite Storage And Export

**Files:**
- Modify: `src/storage.rs`
- Modify: `src/model.rs`
- Test: `tests/storage_tests.rs`

- [ ] **Step 1: Add storage model types**

Append to `src/model.rs`:

```rust
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
```

- [ ] **Step 2: Write failing storage tests**

Create `tests/storage_tests.rs`:

```rust
use chrono::Utc;
use polymarket_analysis::model::{BookmakerOdds, MatchIdentity, ParseStatus};
use polymarket_analysis::storage::{connect_sqlite, insert_match, insert_oddsportal_snapshot, load_export_rows};

#[tokio::test]
async fn appends_multiple_odds_snapshots() {
    let pool = connect_sqlite("sqlite::memory:").await.unwrap();
    let identity = MatchIdentity {
        match_id: "southampton_vs_wrexham".to_string(),
        home_team: "Southampton".to_string(),
        away_team: "Wrexham".to_string(),
        match_time: None,
    };
    insert_match(&pool, &identity, "oddsportal", Some("https://example.test/match")).await.unwrap();

    let odds = vec![BookmakerOdds {
        bookmaker: "bet365".to_string(),
        home: 2.2,
        draw: 3.25,
        away: 3.25,
    }];
    insert_oddsportal_snapshot(&pool, &identity.match_id, Utc::now(), Some(200), ParseStatus::Parsed, None, &odds)
        .await
        .unwrap();
    insert_oddsportal_snapshot(&pool, &identity.match_id, Utc::now(), Some(200), ParseStatus::Parsed, None, &odds)
        .await
        .unwrap();

    let rows = load_export_rows(&pool, &identity.match_id).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source, "oddsportal");
}
```

- [ ] **Step 3: Run tests and verify failure**

Run:

```bash
cargo test --test storage_tests
```

Expected: FAIL because storage functions are not implemented.

- [ ] **Step 4: Implement storage**

Replace `src/storage.rs` with:

```rust
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use crate::cli::{ExportArgs, ExportFormat};
use crate::model::{BookmakerOdds, MatchIdentity, ParseStatus};

#[derive(Debug, Serialize)]
pub struct ExportRow {
    pub snapshot_id: i64,
    pub match_id: String,
    pub source: String,
    pub collected_at: String,
    pub bookmaker: Option<String>,
    pub home: Option<f64>,
    pub draw: Option<f64>,
    pub away: Option<f64>,
}

pub async fn connect_sqlite(url: &str) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new().max_connections(5).connect(url).await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS matches (
            id TEXT PRIMARY KEY,
            home_team TEXT NOT NULL,
            away_team TEXT NOT NULL,
            match_time TEXT,
            canonical_key TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS match_sources (
            match_id TEXT NOT NULL,
            source TEXT NOT NULL,
            url TEXT,
            external_id TEXT,
            PRIMARY KEY (match_id, source),
            FOREIGN KEY(match_id) REFERENCES matches(id)
        );
        CREATE TABLE IF NOT EXISTS snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id TEXT NOT NULL,
            source TEXT NOT NULL,
            collected_at TEXT NOT NULL,
            http_status INTEGER,
            parse_status TEXT NOT NULL,
            raw_hash TEXT,
            raw_artifact_path TEXT,
            error_message TEXT,
            FOREIGN KEY(match_id) REFERENCES matches(id)
        );
        CREATE TABLE IF NOT EXISTS oddsportal_odds (
            snapshot_id INTEGER NOT NULL,
            bookmaker TEXT NOT NULL,
            home REAL NOT NULL,
            draw REAL NOT NULL,
            away REAL NOT NULL,
            FOREIGN KEY(snapshot_id) REFERENCES snapshots(id)
        );
        CREATE TABLE IF NOT EXISTS polymarket_prices (
            snapshot_id INTEGER NOT NULL,
            market_id TEXT,
            market_title TEXT NOT NULL,
            outcome TEXT NOT NULL,
            price REAL NOT NULL,
            volume REAL,
            active INTEGER,
            FOREIGN KEY(snapshot_id) REFERENCES snapshots(id)
        );
        "#
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_match(pool: &SqlitePool, identity: &MatchIdentity, source: &str, url: Option<&str>) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO matches (id, home_team, away_team, match_time, canonical_key, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(id) DO UPDATE SET
            home_team = excluded.home_team,
            away_team = excluded.away_team,
            match_time = excluded.match_time
        "#
    )
    .bind(&identity.match_id)
    .bind(&identity.home_team)
    .bind(&identity.away_team)
    .bind(identity.match_time.map(|dt| dt.to_rfc3339()))
    .bind(&identity.match_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO match_sources (match_id, source, url, external_id)
        VALUES (?1, ?2, ?3, NULL)
        ON CONFLICT(match_id, source) DO UPDATE SET url = excluded.url
        "#
    )
    .bind(&identity.match_id)
    .bind(source)
    .bind(url)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_oddsportal_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
    odds: &[BookmakerOdds],
) -> Result<i64> {
    let snapshot_id = insert_snapshot(pool, match_id, "oddsportal", collected_at, http_status, parse_status, error_message).await?;
    for row in odds {
        sqlx::query("INSERT INTO oddsportal_odds (snapshot_id, bookmaker, home, draw, away) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(snapshot_id)
            .bind(&row.bookmaker)
            .bind(row.home)
            .bind(row.draw)
            .bind(row.away)
            .execute(pool)
            .await?;
    }
    Ok(snapshot_id)
}

async fn insert_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    source: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
) -> Result<i64> {
    let result = sqlx::query(
        "INSERT INTO snapshots (match_id, source, collected_at, http_status, parse_status, error_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    )
    .bind(match_id)
    .bind(source)
    .bind(collected_at.to_rfc3339())
    .bind(http_status)
    .bind(format!("{:?}", parse_status).to_lowercase())
    .bind(error_message)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn load_export_rows(pool: &SqlitePool, match_id: &str) -> Result<Vec<ExportRow>> {
    let rows = sqlx::query(
        r#"
        SELECT s.id, s.match_id, s.source, s.collected_at, o.bookmaker, o.home, o.draw, o.away
        FROM snapshots s
        LEFT JOIN oddsportal_odds o ON o.snapshot_id = s.id
        WHERE s.match_id = ?1
        ORDER BY s.id ASC
        "#
    )
    .bind(match_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|row| ExportRow {
        snapshot_id: row.get("id"),
        match_id: row.get("match_id"),
        source: row.get("source"),
        collected_at: row.get("collected_at"),
        bookmaker: row.get("bookmaker"),
        home: row.get("home"),
        draw: row.get("draw"),
        away: row.get("away"),
    }).collect())
}

pub async fn export_match(args: ExportArgs) -> Result<()> {
    let db_url = format!("sqlite://{}", args.db.display());
    let pool = connect_sqlite(&db_url).await?;
    let rows = load_export_rows(&pool, &args.match_id).await?;
    match args.format {
        ExportFormat::Jsonl => {
            for row in rows {
                println!("{}", serde_json::to_string(&row)?);
            }
        }
        ExportFormat::Csv => {
            let mut writer = csv::Writer::from_writer(std::io::stdout());
            for row in rows {
                writer.serialize(row)?;
            }
            writer.flush()?;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run storage tests**

Run:

```bash
cargo test --test storage_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/model.rs src/storage.rs tests/storage_tests.rs
git commit -m "feat: store snapshots in sqlite"
```

### Task 5: Provider Fetching And Polymarket Parsing

**Files:**
- Modify: `src/providers/mod.rs`
- Modify: `src/providers/polymarket.rs`
- Test: `tests/polymarket_tests.rs`

- [ ] **Step 1: Define provider shared types**

Replace `src/providers/mod.rs`:

```rust
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::model::{MatchIdentity, ProviderPayload};

pub mod oddsportal;
pub mod polymarket;

#[derive(Clone, Debug)]
pub struct ProviderTarget {
    pub url: String,
    pub identity: Option<MatchIdentity>,
}

#[derive(Clone, Debug)]
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
    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot>;
}
```

- [ ] **Step 2: Write failing Polymarket tests**

Create `tests/polymarket_tests.rs`:

```rust
use polymarket_analysis::providers::polymarket::{extract_polymarket_identity, parse_polymarket_market};

#[test]
fn extracts_teams_from_market_title() {
    let identity = extract_polymarket_identity(r#"{"question":"Southampton vs Wrexham"}"#).unwrap();
    assert_eq!(identity.match_id, "southampton_vs_wrexham");
}

#[test]
fn parses_polymarket_prices_from_synthetic_json() {
    let body = r#"{
        "id":"market_1",
        "question":"Southampton vs Wrexham",
        "active":true,
        "volume":12500.5,
        "outcomes":["Southampton","Wrexham"],
        "outcomePrices":["0.62","0.38"]
    }"#;
    let prices = parse_polymarket_market(body).unwrap();
    assert_eq!(prices.len(), 2);
    assert_eq!(prices[0].outcome, "Southampton");
    assert_eq!(prices[0].price, 0.62);
}
```

- [ ] **Step 3: Run tests and verify failure**

Run:

```bash
cargo test --test polymarket_tests
```

Expected: FAIL because Polymarket parsing is a stub.

- [ ] **Step 4: Implement Polymarket parser and fetcher**

Replace `src/providers/polymarket.rs`:

```rust
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

use crate::match_resolver::resolve_from_text;
use crate::model::{MatchIdentity, PolymarketPrice, ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

pub struct PolymarketProvider {
    client: Client,
}

impl PolymarketProvider {
    pub fn new() -> Self {
        Self { client: Client::new() }
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
        let identity = extract_polymarket_identity(&body).ok().or_else(|| target.identity.clone());
        let prices = parse_polymarket_market(&body)?;
        Ok(ProviderSnapshot {
            source: "polymarket",
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
            if let Some(text) = value.get(key).and_then(Value::as_str) {
                if let Ok(identity) = resolve_from_text(text) {
                    return Ok(identity);
                }
            }
        }
    }
    resolve_from_text(body)
}

pub fn parse_polymarket_market(body: &str) -> Result<Vec<PolymarketPrice>> {
    let value: Value = serde_json::from_str(body).context("polymarket response is not valid JSON")?;
    let market_id = value.get("id").and_then(Value::as_str).map(str::to_string);
    let market_title = value
        .get("question")
        .or_else(|| value.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("unknown market")
        .to_string();
    let volume = value.get("volume").and_then(Value::as_f64);
    let active = value.get("active").and_then(Value::as_bool);
    let outcomes = read_string_array(&value, "outcomes");
    let prices = read_f64_array(&value, "outcomePrices");

    let mut rows = Vec::new();
    for (idx, outcome) in outcomes.iter().enumerate() {
        if let Some(price) = prices.get(idx) {
            rows.push(PolymarketPrice {
                market_id: market_id.clone(),
                market_title: market_title.clone(),
                outcome: outcome.clone(),
                price: *price,
                volume,
                active,
            });
        }
    }
    Ok(rows)
}

fn read_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn read_f64_array(value: &Value, key: &str) -> Vec<f64> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| match item {
                    Value::Number(num) => num.as_f64(),
                    Value::String(text) => text.parse::<f64>().ok(),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}
```

- [ ] **Step 5: Run provider tests**

Run:

```bash
cargo test --test polymarket_tests --test oddsportal_tests
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/providers tests/polymarket_tests.rs
git commit -m "feat: parse polymarket market data"
```

### Task 6: Collector Scheduling And Backoff

**Files:**
- Modify: `src/collector.rs`
- Modify: `src/storage.rs`
- Test: `tests/collector_tests.rs`

- [ ] **Step 1: Write failing scheduler tests**

Create `tests/collector_tests.rs`:

```rust
use polymarket_analysis::collector::{BackoffPolicy, ScheduledProvider};

#[test]
fn polymarket_backoff_is_fast_but_capped() {
    let mut schedule = ScheduledProvider::new("polymarket", 1, BackoffPolicy::polymarket());
    assert_eq!(schedule.current_delay_seconds(), 1);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 2);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 5);
    schedule.record_success();
    assert_eq!(schedule.current_delay_seconds(), 1);
}

#[test]
fn oddsportal_backoff_doubles_to_cap() {
    let mut schedule = ScheduledProvider::new("oddsportal", 60, BackoffPolicy::oddsportal(60));
    assert_eq!(schedule.current_delay_seconds(), 60);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 120);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 240);
    schedule.record_failure();
    assert_eq!(schedule.current_delay_seconds(), 300);
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --test collector_tests
```

Expected: FAIL because scheduler types do not exist.

- [ ] **Step 3: Implement scheduler state and collection shell**

Replace `src/collector.rs`:

```rust
use anyhow::Result;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use crate::cli::CollectArgs;

#[derive(Clone, Debug)]
pub enum BackoffPolicy {
    Sequence { base: u64, steps: Vec<u64>, cap: u64 },
    Doubling { base: u64, cap: u64 },
}

impl BackoffPolicy {
    pub fn polymarket() -> Self {
        Self::Sequence { base: 1, steps: vec![2, 5, 10], cap: 60 }
    }

    pub fn oddsportal(base: u64) -> Self {
        Self::Doubling { base, cap: 300 }
    }
}

#[derive(Clone, Debug)]
pub struct ScheduledProvider {
    name: &'static str,
    base_interval_seconds: u64,
    policy: BackoffPolicy,
    failures: usize,
}

impl ScheduledProvider {
    pub fn new(name: &'static str, base_interval_seconds: u64, policy: BackoffPolicy) -> Self {
        Self { name, base_interval_seconds, policy, failures: 0 }
    }

    pub fn current_delay_seconds(&self) -> u64 {
        match &self.policy {
            BackoffPolicy::Sequence { base, steps, cap } => {
                if self.failures == 0 {
                    self.base_interval_seconds.max(*base)
                } else {
                    steps.get(self.failures - 1).copied().unwrap_or(*cap).min(*cap)
                }
            }
            BackoffPolicy::Doubling { base, cap } => {
                if self.failures == 0 {
                    self.base_interval_seconds.max(*base)
                } else {
                    let multiplier = 2_u64.saturating_pow(self.failures as u32);
                    base.saturating_mul(multiplier).min(*cap)
                }
            }
        }
    }

    pub fn record_success(&mut self) {
        self.failures = 0;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        warn!(provider = self.name, delay_seconds = self.current_delay_seconds(), "provider entered backoff");
    }
}

pub async fn collect(args: CollectArgs) -> Result<()> {
    let mut handles = Vec::new();

    if let Some(url) = args.polymarket_url.clone() {
        let mut schedule = ScheduledProvider::new(
            "polymarket",
            args.polymarket_interval_seconds,
            BackoffPolicy::polymarket(),
        );
        handles.push(tokio::spawn(async move {
            loop {
                info!(provider = "polymarket", url = %url, "collection tick");
                schedule.record_success();
                sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
            }
        }));
    }

    if let Some(url) = args.odds_url.clone() {
        let mut schedule = ScheduledProvider::new(
            "oddsportal",
            args.odds_interval_seconds,
            BackoffPolicy::oddsportal(args.odds_interval_seconds),
        );
        handles.push(tokio::spawn(async move {
            loop {
                info!(provider = "oddsportal", url = %url, "collection tick");
                schedule.record_success();
                sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
            }
        }));
    }

    for handle in handles {
        handle.await??;
    }
    Ok(())
}
```

- [ ] **Step 4: Run scheduler tests**

Run:

```bash
cargo test --test collector_tests
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/collector.rs tests/collector_tests.rs
git commit -m "feat: add provider collection schedules"
```

### Task 7: Wire Collection To Providers And SQLite

**Files:**
- Modify: `src/collector.rs`
- Modify: `src/storage.rs`
- Modify: `src/providers/oddsportal.rs`
- Test: `tests/storage_tests.rs`

- [ ] **Step 1: Add storage insert for Polymarket prices**

Add to `src/storage.rs`:

```rust
use crate::model::PolymarketPrice;

pub async fn insert_polymarket_snapshot(
    pool: &SqlitePool,
    match_id: &str,
    collected_at: DateTime<Utc>,
    http_status: Option<i64>,
    parse_status: ParseStatus,
    error_message: Option<&str>,
    prices: &[PolymarketPrice],
) -> Result<i64> {
    let snapshot_id = insert_snapshot(pool, match_id, "polymarket", collected_at, http_status, parse_status, error_message).await?;
    for row in prices {
        sqlx::query(
            "INSERT INTO polymarket_prices (snapshot_id, market_id, market_title, outcome, price, volume, active) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )
        .bind(snapshot_id)
        .bind(&row.market_id)
        .bind(&row.market_title)
        .bind(&row.outcome)
        .bind(row.price)
        .bind(row.volume)
        .bind(row.active.map(i64::from))
        .execute(pool)
        .await?;
    }
    Ok(snapshot_id)
}
```

- [ ] **Step 2: Implement OddsPortal fetcher**

Append to `src/providers/oddsportal.rs`:

```rust
use chrono::Utc;
use reqwest::Client;

use crate::model::{ProviderPayload};
use crate::providers::{Provider, ProviderSnapshot, ProviderTarget};

pub struct OddsPortalProvider {
    client: Client,
}

impl OddsPortalProvider {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }
}

impl Provider for OddsPortalProvider {
    fn source_name(&self) -> &'static str {
        "oddsportal"
    }

    async fn fetch_snapshot(&self, target: &ProviderTarget) -> Result<ProviderSnapshot> {
        let response = self.client
            .get(&target.url)
            .header("user-agent", "polymarket-analysis/0.1")
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let identity = extract_oddsportal_match_identity(&body).ok().or_else(|| target.identity.clone());
        let odds = parse_oddsportal_odds(&body)?;
        Ok(ProviderSnapshot {
            source: "oddsportal",
            collected_at: Utc::now(),
            http_status: Some(status),
            identity,
            payload: ProviderPayload::OddsPortal { odds },
            raw_body: Some(body),
        })
    }
}
```

- [ ] **Step 3: Replace collection shell with real provider writes**

Modify `src/collector.rs` so each provider task:

```rust
use sqlx::SqlitePool;

use crate::model::{ParseStatus, ProviderPayload};
use crate::providers::{Provider, ProviderTarget};
use crate::providers::oddsportal::OddsPortalProvider;
use crate::providers::polymarket::PolymarketProvider;
use crate::storage::{
    connect_sqlite,
    insert_match,
    insert_oddsportal_snapshot,
    insert_polymarket_snapshot,
};
```

Inside `collect`, open SQLite once:

```rust
let db_url = format!("sqlite://{}", args.db.display());
let pool = connect_sqlite(&db_url).await?;
```

Spawn provider loops with:

```rust
async fn run_provider_loop<P: Provider + Send + Sync + 'static>(
    provider: P,
    target: ProviderTarget,
    pool: SqlitePool,
    mut schedule: ScheduledProvider,
) {
    loop {
        match provider.fetch_snapshot(&target).await {
            Ok(snapshot) => {
                let identity = snapshot.identity.or_else(|| target.identity.clone());
                if let Some(identity) = identity {
                    let _ = insert_match(&pool, &identity, snapshot.source, Some(&target.url)).await;
                    match snapshot.payload {
                        ProviderPayload::OddsPortal { odds } => {
                            let parse_status = if odds.is_empty() { ParseStatus::Empty } else { ParseStatus::Parsed };
                            let _ = insert_oddsportal_snapshot(
                                &pool,
                                &identity.match_id,
                                snapshot.collected_at,
                                snapshot.http_status.map(i64::from),
                                parse_status,
                                None,
                                &odds,
                            ).await;
                        }
                        ProviderPayload::Polymarket { prices } => {
                            let parse_status = if prices.is_empty() { ParseStatus::Empty } else { ParseStatus::Parsed };
                            let _ = insert_polymarket_snapshot(
                                &pool,
                                &identity.match_id,
                                snapshot.collected_at,
                                snapshot.http_status.map(i64::from),
                                parse_status,
                                None,
                                &prices,
                            ).await;
                        }
                    }
                    schedule.record_success();
                } else {
                    schedule.record_failure();
                }
            }
            Err(error) => {
                warn!(provider = provider.source_name(), error = %error, "provider fetch failed");
                schedule.record_failure();
            }
        }
        sleep(Duration::from_secs(schedule.current_delay_seconds())).await;
    }
}
```

- [ ] **Step 4: Run full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/collector.rs src/storage.rs src/providers/oddsportal.rs
git commit -m "feat: write collected provider snapshots"
```

### Task 8: CLI Polish, README, And Go Retirement

**Files:**
- Modify: `README.md`
- Delete or archive: `main.go`, `go.mod`, `polymarket-analysis`
- Verify: all Rust tests

- [ ] **Step 1: Update README**

Replace `README.md` with:

```markdown
# Polymarket Match Odds Collector

Rust CLI for collecting Polymarket and OddsPortal match data into SQLite.

## Usage

Collect both sources:

```bash
cargo run -- collect \
  --polymarket-url "https://polymarket.com/..." \
  --odds-url "https://www.oddsportal.com/..." \
  --polymarket-interval-seconds 1 \
  --odds-interval-seconds 60 \
  --db data/polymarket_analysis.sqlite
```

Export retained match data:

```bash
cargo run -- export \
  --db data/polymarket_analysis.sqlite \
  --match-id southampton_vs_wrexham \
  --format jsonl
```

## Collection Frequency

Polymarket defaults to a 1 second interval. OddsPortal defaults to 60 seconds and backs off on HTTP 403, HTTP 429, empty responses, or parse failures.

## Fixtures

Captured HTML, text, and JSON files from the original Go prototype are preserved under `tests/fixtures/` and used for parser regression tests.
```

- [ ] **Step 2: Remove Go runtime files after Rust tests pass**

Run:

```bash
cargo test
```

Expected: PASS.

Then remove the Go prototype files:

```bash
git rm main.go go.mod polymarket-analysis
```

- [ ] **Step 3: Format and lint**

Run:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add README.md Cargo.toml Cargo.lock src tests
git commit -m "docs: document rust collector usage"
```

If `git rm` staged deleted Go files, include them in this same commit.

## Plan Self-Review

- Spec coverage: The plan covers Rust migration, fixture preservation, automatic team resolution, SQLite append-only snapshots, separate Polymarket/OddsPortal intervals, OddsPortal backoff, JSONL/CSV export, and README updates.
- Placeholder scan: No unresolved placeholder markers are present.
- Type consistency: Shared types are introduced before tests reference them. Provider payload variants match storage insertion tasks.
- Scope check: The plan stays within the first CLI implementation and does not include daemonization, browser automation, or additional odds providers.
