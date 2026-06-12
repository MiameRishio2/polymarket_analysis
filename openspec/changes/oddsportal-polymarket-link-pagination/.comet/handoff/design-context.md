# Comet Design Handoff

- Change: oddsportal-polymarket-link-pagination
- Phase: design
- Mode: compact
- Context hash: ffc553357b78dcadc710cf83b919925262fd8dd3d245f3f60f51cbacfa80ca76

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/oddsportal-polymarket-link-pagination/proposal.md

- Source: openspec/changes/oddsportal-polymarket-link-pagination/proposal.md
- Lines: 1-37
- SHA256: dad7dac5e97a463d862a4126c9f5893ffac64f558a3d0221948bfb73c27e7f3b

```md
## Why

The fourth-level menu page can already parse OddsPortal competition event rows, but the "链接" column still behaves like a plain text URL / whole-row link. For a match such as Mexico vs South Africa, the useful workflow is to open either the OddsPortal H2H page or the matching Polymarket market.

The user wants the connected links to remain under the existing link column, but as explicit buttons: one for OddsPortal and one for Polymarket. The Polymarket link should be discovered by slug search rather than hard-coded from the menu page.

## What Changes

- Add a Polymarket URL field to event rows.
- Build deterministic Polymarket slug candidates from OddsPortal event data and search Polymarket by slug using the local Rust integration.
- For World Cup events, support the Polymarket sports path shape shown by the user, for example `fifwc-mex-rsa-2026-06-11`.
- Render the event table "链接" column as two buttons:
  - `OddsPortal`: enabled when the event has an OddsPortal URL.
  - `Polymarket`: enabled only when slug search finds a matching Polymarket market URL.
- When no Polymarket match is found, show a disabled Polymarket button instead of guessing or linking to a broad listing page.

## Capabilities

### Modified Capabilities

- `event-external-market-links`: Event rows expose structured external links for OddsPortal and Polymarket.
- `menu-third-level-page`: Fourth-level event tables render link buttons in the existing link column.

## Impact

- `src/menu/events.rs`: Enrich event rows with optional Polymarket links and slug search helpers.
- `src/menu/handlers.rs`: Keep event API routes stable while returning the expanded event row shape.
- `public/menu.html`: Replace event URL text with two external-link buttons.
- `tests/menu_third_level_test.rs`: Cover Polymarket slug candidate generation / disabled fallback in serialized event rows.
- `tests/menu_page_config_test.js`: Cover event link button rendering.

## Non-Goals

- Do not scrape odds prices or bookmaker lines.
- Do not trade, sign, or place orders through Polymarket CLOB.
- Do not show a broad Polymarket search/listing link when no exact market is found.
- Do not redesign the menu page outside the event table link column.
```

## openspec/changes/oddsportal-polymarket-link-pagination/design.md

- Source: openspec/changes/oddsportal-polymarket-link-pagination/design.md
- Lines: 1-72
- SHA256: 5e640ee78282d43ffd7e330b874eb043035b166d46b1d03ba4b45aadd02e7c5b

```md
## Context

Event rows are parsed by `src/menu/events.rs` and returned from `/api/events/{sport}/{category}/{league}`. The frontend in `public/menu.html` uses these rows on fourth-level pages before falling back to category rendering.

The current `EventRow` contract has a single `url`, which is the OddsPortal destination. The frontend wraps the whole row in that URL and displays the URL text in the link column. That makes adding a second destination awkward and hides whether Polymarket matching succeeded.

## Approach Options

1. **Recommended: enrich event rows with explicit link fields.**
   Keep `url` as the OddsPortal URL for compatibility, add `polymarket_url: Option<String>`, and render both destinations as buttons. This is the smallest API extension and maps cleanly to the existing event API.

2. **Return a nested `links` object.**
   Replace `url` with `{ oddsportal, polymarket }`. This is cleaner long term, but it breaks current tests/callers that expect `url`.

3. **Resolve Polymarket entirely in the browser.**
   Keep the backend unchanged and generate/search links from JavaScript. This avoids Rust changes but moves network/domain logic into the static page and makes testing and caching worse.

The implementation will use option 1.

## Architecture Decisions

1. Event parsing remains the source of team names, date text, and OddsPortal URL.
2. Backend enrichment derives Polymarket slug candidates from event fields and the current competition context.
3. The event API remains `/api/events/{sport}/{category}/{league}` and continues to return event rows. Existing `url` remains the OddsPortal URL.
4. The frontend treats event rows as non-row-link table rows and renders explicit buttons in the "链接" column.
5. A missing Polymarket URL is a first-class state and renders as a disabled button.

## Polymarket Matching

For the user-provided World Cup example, the slug candidate format is:

```text
fifwc-{home_code}-{away_code}-{yyyy-mm-dd}
```

For Mexico vs South Africa on 2026-06-11, this yields:

```text
fifwc-mex-rsa-2026-06-11
```

The search layer should ask Polymarket for an exact slug match. If it returns a market/event with a usable slug, the UI link becomes:

```text
https://polymarket.com/sports/world-cup/{slug}
```

If no match is found, `polymarket_url` stays absent.

## Data Flow

```text
OddsPortal competition page
  -> extract EventRow { teams, start_time, oddsportal url }
  -> derive Polymarket slug candidates
  -> search Polymarket by slug
  -> EventRow { url, polymarket_url? }
  -> public/menu.html event table
  -> OddsPortal / Polymarket buttons under "链接"
```

## Error Handling

- Polymarket lookup failures must not prevent OddsPortal event rows from rendering.
- Lookup timeouts, HTTP errors, malformed JSON, or no exact slug match all produce `polymarket_url: null` / omitted.
- The Polymarket button is disabled when the field is absent.

## Testing

- Rust tests for World Cup slug candidate generation from Mexico vs South Africa.
- Rust tests that a seeded cached event row can omit `polymarket_url` and still serialize/serve correctly.
- Frontend Node tests that event rows render `OddsPortal` and `Polymarket` buttons, and the Polymarket button is disabled when no URL exists.
```

## openspec/changes/oddsportal-polymarket-link-pagination/tasks.md

- Source: openspec/changes/oddsportal-polymarket-link-pagination/tasks.md
- Lines: 1-23
- SHA256: a126519a523d0f088041759d4aa0db903dcdd71824d4a4bef355b6c457dccdf5

```md
## 1. OpenSpec Repair

- [ ] 1.1 Recreate missing change metadata and proposal/design/tasks artifacts.
- [ ] 1.2 Add delta spec requirements for event external market links.

## 2. Backend

- [ ] 2.1 Extend event row serialization with optional Polymarket URL while preserving existing `url`.
- [ ] 2.2 Add slug candidate generation for World Cup event rows, including Mexico vs South Africa -> `fifwc-mex-rsa-2026-06-11`.
- [ ] 2.3 Add Polymarket by-slug lookup that returns an exact matching sports URL or `None`.
- [ ] 2.4 Keep Polymarket lookup failures non-fatal for event list rendering.

## 3. Frontend

- [ ] 3.1 Render event rows with explicit OddsPortal and Polymarket buttons under the existing "链接" column.
- [ ] 3.2 Disable the Polymarket button when `polymarket_url` is missing.
- [ ] 3.3 Preserve existing event pagination and category fallback behavior.

## 4. Verification

- [ ] 4.1 Add Rust tests for slug generation and event API compatibility.
- [ ] 4.2 Add Node tests for button rendering and disabled Polymarket fallback.
- [ ] 4.3 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.
```

## openspec/changes/oddsportal-polymarket-link-pagination/specs/event-external-market-links/spec.md

- Source: openspec/changes/oddsportal-polymarket-link-pagination/specs/event-external-market-links/spec.md
- Lines: 1-37
- SHA256: 7b1a9da13b1447bb0492d2b4d0bb99793b842596565bd764606be1e3f86fb85e

```md
## ADDED Requirements

### Requirement: Event rows expose external market links
The system SHALL expose structured external destinations for parsed event rows while preserving the existing OddsPortal `url` field.

#### Scenario: Event row includes OddsPortal and Polymarket destinations
- **WHEN** the event API returns a World Cup event row with a matching Polymarket slug
- **THEN** the row contains the OddsPortal H2H URL in `url`
- **AND** the row contains the matching Polymarket sports URL in `polymarket_url`

#### Scenario: Missing Polymarket match is represented explicitly
- **WHEN** Polymarket slug lookup returns no exact match for an OddsPortal event
- **THEN** the event row still appears with its OddsPortal URL
- **AND** `polymarket_url` is absent or null

### Requirement: World Cup Polymarket slug lookup
The system SHALL derive World Cup Polymarket slug candidates from OddsPortal event teams and start date, then search Polymarket by exact slug.

#### Scenario: Mexico South Africa World Cup slug
- **WHEN** an event row has home team `Mexico`, away team `South Africa`, and date `2026-06-11`
- **THEN** the slug candidate list includes `fifwc-mex-rsa-2026-06-11`

#### Scenario: Lookup failures are non-fatal
- **WHEN** Polymarket lookup fails, times out, or returns malformed data
- **THEN** the event API still returns the OddsPortal event row

### Requirement: Event table renders link buttons
The frontend SHALL render event row external destinations as buttons in the existing "链接" column.

#### Scenario: Both links are available
- **WHEN** an event row has `url` and `polymarket_url`
- **THEN** the "链接" column shows enabled `OddsPortal` and `Polymarket` buttons

#### Scenario: Polymarket is unavailable
- **WHEN** an event row has no `polymarket_url`
- **THEN** the "链接" column shows an enabled `OddsPortal` button
- **AND** shows a disabled `Polymarket` button
```

