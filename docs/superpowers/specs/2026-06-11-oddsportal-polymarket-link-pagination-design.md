---
comet_change: oddsportal-polymarket-link-pagination
role: technical-design
canonical_spec: openspec
---

# OddsPortal Polymarket Link Pagination Technical Design

## Context

The fourth-level menu page already prefers `/api/events/{sport}/{category}/{league}` for competition pages and falls back to category rows when no events are available. Each `EventRow` currently exposes one `url`, which is the OddsPortal H2H link. The event table then makes the entire row an external link and prints that URL in the existing "链接" column.

The requested workflow needs two explicit destinations in that same column: OddsPortal and Polymarket. Polymarket must be resolved by slug lookup through `rs-clob-client-v2`, not by sending users to a broad listing page.

## Technical Approach

Extend the event row contract instead of replacing it:

```rust
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    pub url: String,
    pub polymarket_url: Option<String>,
}
```

`url` remains the OddsPortal URL for compatibility. `polymarket_url` is optional and omitted from JSON when no exact slug match is found.

Polymarket lookup uses `rs-clob-client-v2` with unauthenticated Gamma endpoints:

```rust
ClobClient::new(
    "https://clob.polymarket.com".to_string(),
    "https://gamma-api.polymarket.com".to_string(),
    Chain::Polygon,
    None,
    None,
    None,
    None,
    None,
    false,
    None,
    None,
)?
```

For each generated slug candidate, the lookup tries `get_event_by_slug(slug)` first, then `get_market_by_slug(slug)`. A response only counts when the returned `slug` exactly matches the requested slug. The resulting public URL is `https://polymarket.com/sports/world-cup/{slug}` for World Cup competition rows.

## Slug Derivation

The initial implementation covers the user-provided World Cup page:

```text
/menu/football/world/world-championship-2026
```

Candidate format:

```text
fifwc-{home_code}-{away_code}-{yyyy-mm-dd}
```

Team codes are deterministic aliases, with the required example:

```text
Mexico -> mex
South Africa -> rsa
```

The date is extracted from the event `start_time`. ISO dates and OddsPortal JSON-LD-derived display dates such as `11 Jun 2026, 21:00` both normalize to `2026-06-11`.

If the competition is not recognized or a date cannot be parsed, no Polymarket candidate is generated and the UI shows the disabled fallback.

## Data Flow

```text
OddsPortal HTML
  -> extract EventRow with OddsPortal URL
  -> derive World Cup slug candidates
  -> rs-clob-client-v2 get_event_by_slug / get_market_by_slug
  -> attach polymarket_url when exact slug exists
  -> event API JSON
  -> public/menu.html event table buttons
```

## Frontend Design

Event rows become plain grid rows instead of `<a>` wrappers. The "链接" column renders:

- enabled `OddsPortal` anchor when `item.url` exists
- enabled `Polymarket` anchor when `item.polymarket_url` exists
- disabled `Polymarket` button when `item.polymarket_url` is missing

The table keeps the existing event columns, pagination size, stats, and fourth-level category fallback.

## Error Handling

- `rs-clob-client-v2` construction or lookup errors are logged and treated as no match.
- Polymarket lookup never prevents OddsPortal events from being returned.
- Cached legacy event rows without `polymarket_url` deserialize with `None`.

## Testing Strategy

Follow TDD:

1. Rust red tests for `EventRow` JSON compatibility and World Cup slug generation.
2. Rust red test for building the Polymarket sports URL from an exact slug result.
3. Frontend red tests for enabled OddsPortal/Polymarket buttons and disabled Polymarket fallback.
4. Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.

