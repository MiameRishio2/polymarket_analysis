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

Polymarket defaults to a 1 second interval. OddsPortal defaults to 60 seconds and backs off on failed HTTP requests, empty responses, or parse failures.

## Stored Data

The collector resolves a match identity from the configured provider URLs before entering the polling loop, then stores append-only snapshots in SQLite. Successful snapshots retain parsed Polymarket prices or OddsPortal bookmaker odds. Failed, empty, or non-2xx collection attempts are recorded as diagnostic snapshots so gaps in the match history are visible in the database.

## Fixtures

Captured HTML, text, and JSON files from the original Go prototype are preserved under `tests/fixtures/` and used for parser regression tests.
