# Rust Match Odds Collector Design

## Goal

Replace the current Go prototype with a Rust CLI that automatically identifies match teams from web data, collects Polymarket and odds-site data throughout a match, and preserves the full time series for later analysis.

The first implementation targets OddsPortal as the odds provider. The design keeps provider boundaries explicit so additional odds sites can be added later without changing the storage model or collector loop.

## Current Project Context

The repository currently contains a single Go CLI in `main.go`, a `go.mod`, generated binary `polymarket-analysis`, and captured sample data files:

- `oddsportal_debug.html`
- `odds_portal.html`
- `odds_portal_ajax.html`
- `odds_portal_eventdata.html`
- `odds_portal_eventdata_decoded.txt`
- `odds_portal_decoded.txt`
- `match_data_southampton_vs_wrexham_20260408_224015.json`
- `match_data_millwall_vs_west_brom_20260408_225019.json`
- generated comparison HTML files

There are no separate committed test files. These captured HTML, text, and JSON files must be preserved as regression fixtures for the Rust migration.

## User-Facing Behavior

The first Rust version is a CLI, with the internals structured so it can later become a background monitoring service.

Primary collection command:

```bash
cargo run -- collect \
  --polymarket-url "https://polymarket.com/..." \
  --odds-url "https://www.oddsportal.com/..." \
  --polymarket-interval-seconds 1 \
  --odds-interval-seconds 60 \
  --db data/polymarket_analysis.sqlite
```

Expected behavior:

- The user should not need to manually provide home and away team names during normal operation.
- If an OddsPortal URL is provided, the program parses the page data, embedded JSON, or URL/page title to identify teams.
- If a Polymarket URL is provided, the program parses the market title/outcomes and binds the market to the same match.
- If automatic team resolution fails, the command exits with a clear error. Manual team-name override arguments are out of scope for the first implementation.
- Polymarket collection defaults to a 1 second interval.
- OddsPortal collection defaults to a configurable, conservative interval. The initial default is 60 seconds.
- Polymarket and OddsPortal run on independent schedules so OddsPortal waiting or backoff does not slow Polymarket collection.

Export commands:

```bash
cargo run -- export --db data/polymarket_analysis.sqlite --match-id southampton_vs_wrexham --format jsonl
cargo run -- export --db data/polymarket_analysis.sqlite --match-id southampton_vs_wrexham --format csv
```

## Architecture

The Rust project should use focused modules with stable boundaries:

- `cli`: Defines commands and arguments.
- `providers`: Contains source-specific clients and parsers. First implementations are `PolymarketProvider` and `OddsPortalProvider`.
- `match_resolver`: Converts provider-specific metadata into normalized match identity: home team, away team, match time when available, and `match_id`.
- `collector`: Runs the independent provider schedules, applies retry and backoff policy, and writes every result to storage.
- `storage`: Owns SQLite schema, migrations, inserts, and export queries.
- `model`: Shared domain types such as `Match`, `Snapshot`, `PolymarketPrice`, and `BookmakerOdds`.

Provider interface shape:

```rust
pub trait Provider {
    fn source_name(&self) -> &'static str;
    async fn fetch_snapshot(&self, target: &ProviderTarget) -> anyhow::Result<ProviderSnapshot>;
}
```

The exact Rust signatures can change during implementation, but the boundary must keep network fetching, parsing, and source-specific metadata outside the storage layer.

## Data Model

SQLite is the primary store. JSONL and CSV are exports, not parallel runtime stores.

Tables:

- `matches`: One row per match.
  - `id`
  - `home_team`
  - `away_team`
  - `match_time`
  - `canonical_key`
  - `created_at`
- `match_sources`: Source URLs and source-specific IDs attached to a match.
  - `match_id`
  - `source`
  - `url`
  - `external_id`
- `snapshots`: One row per provider collection attempt, including failed attempts.
  - `id`
  - `match_id`
  - `source`
  - `collected_at`
  - `http_status`
  - `parse_status`
  - `raw_hash`
  - `raw_artifact_path`
  - `error_message`
- `polymarket_prices`: Parsed Polymarket values for a snapshot.
  - `snapshot_id`
  - `market_id`
  - `market_title`
  - `outcome`
  - `price`
  - `volume`
  - `active`
- `oddsportal_odds`: Parsed bookmaker odds for a snapshot.
  - `snapshot_id`
  - `bookmaker`
  - `home`
  - `draw`
  - `away`
- `raw_artifacts`: Optional raw response records when raw HTML or JSON is persisted to disk.
  - `snapshot_id`
  - `path`
  - `sha256`
  - `content_type`

Historical data must append. New snapshots must not overwrite previous snapshots.

## Rate Limiting And Backoff

Polymarket and OddsPortal use separate schedules.

Polymarket:

- Default interval: 1 second.
- Failure backoff: 1 second, then 2 seconds, then 5 seconds, then 10 seconds, capped at 60 seconds.
- A successful fetch resets the backoff.

OddsPortal:

- Default interval: 60 seconds.
- The interval is configurable with `--odds-interval-seconds`.
- First implementation should not make concurrent OddsPortal requests.
- Failure backoff: configured interval, then double it up to a 300 second cap.
- HTTP `429`, HTTP `403`, empty responses, and parse failures trigger backoff.
- Backoff must be visible in logs so the user knows why odds data is slower.

For future multi-match monitoring, OddsPortal should use a global rate limiter across all matches.

## Parsing And Match Resolution

OddsPortal team extraction priority:

1. Embedded event data if available.
2. Page H1/title fields such as `Southampton vs Wrexham` or `Wrexham - Southampton`.
3. Tournament rows such as `event` fields in captured page data.
4. URL slug fallback.

Polymarket team extraction priority:

1. Market question/title.
2. Outcome names.
3. URL slug fallback.

Normalization:

- Trim whitespace and HTML entities.
- Convert separators such as `vs`, `v`, and `-` into home/away candidates.
- Preserve display names exactly enough for reporting.
- Create a stable lowercase ASCII-ish `match_id` from normalized team names.
- Keep source-specific team order metadata when a provider presents teams in a different order.

## Error Handling

Every collection attempt writes a `snapshots` row. Failed attempts should include enough information to diagnose gaps in the time series.

The CLI exits early for configuration errors such as missing URLs or invalid intervals. Runtime provider failures do not stop the whole collection loop unless all configured providers are permanently invalid.

## Testing Strategy

Preserve current captured files as fixtures under `tests/fixtures/`. Tests should cover:

- OddsPortal team-name parsing from captured HTML and decoded text.
- OddsPortal bookmaker odds parsing from captured data.
- Polymarket market title and outcome parsing from fixture or synthetic JSON.
- `match_id` generation.
- SQLite append behavior across multiple snapshots.
- Export output shape for JSONL and CSV.
- Rate limiter behavior with a mock clock, especially that Polymarket can tick every 1 second while OddsPortal waits longer.

The old Go binary and generated HTML files do not need to remain part of the runtime design, but sample data content must be preserved for regression coverage.

## Out Of Scope For First Implementation

- A long-running daemon managed by systemd or another process manager.
- Browser automation for sites that require JavaScript execution.
- Multi-odds-site support beyond an interface and the first OddsPortal implementation.
- Automatic stop at final whistle unless reliable match-time metadata is available during implementation.
- Trading recommendations or betting strategy suggestions.
