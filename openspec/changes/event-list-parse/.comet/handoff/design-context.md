# Comet Design Handoff

- Change: event-list-parse
- Phase: design
- Mode: compact
- Context hash: b3710698e6ded39133541db28f454a7a5b463e13802dd9290483df19f2d8a972

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/event-list-parse/proposal.md

- Source: openspec/changes/event-list-parse/proposal.md
- Lines: 1-30
- SHA256: d0cecb3c9c06a31b693512544eae83f8f7e2681d51a0a7132b7750f9372a71c4

```md
## Why

The current `/menu/{sport}/{category}/{league}` page treats every direct child link as another menu category. For `https://www.oddsportal.com/football/world/world-championship-2026/`, the useful content is the actual match list: rows such as `Mexico VS South Africa`, their scheduled start time, and the downstream H2H/odds link.

This change lets the local menu flow surface playable event rows instead of forcing users to inspect the source OddsPortal page manually.

## What Changes

- Add an event-list parsing capability for OddsPortal competition pages such as `/football/world/world-championship-2026/`.
- Expose parsed event rows through a local API, including home team, away team, display label, start time, and target OddsPortal link.
- Render fourth-level menu pages as event rows when event data is available, while preserving existing category-list behavior for pages that still expose child category links.
- Keep rows table-based and paginated according to `web_design.md`.
- Do not introduce breaking changes to existing `/api/menu` response formats for sports/category menu pages.

## Capabilities

### New Capabilities
- `event-list-page`: Parses and displays match/event rows for OddsPortal competition pages.

### Modified Capabilities
- `menu-third-level-page`: Fourth-level menu pages may display event rows for competition pages instead of only category child links.

## Impact

- `src/menu/scraper.rs`: Add event-row extraction from OddsPortal competition HTML.
- `src/menu/models.rs`: Add event row data structures or extend menu payloads in a backward-compatible way.
- `src/menu/handlers.rs`: Add or route event-list API responses for fourth-level competition pages.
- `public/menu.html`: Render event rows with matchup, start time, and link while keeping existing table/pagination components.
- `tests/`: Add Rust scraper/API tests and Node frontend rendering tests before implementation.
- `web_design.md`, `architect.md`, `test.md`, `session.md`, `change_log.md`: Update documentation as required by project workflow.
```

## openspec/changes/event-list-parse/design.md

- Source: openspec/changes/event-list-parse/design.md
- Lines: 1-62
- SHA256: c80ef00bcc43ae16c33bfed52a183a6df0d3606ece6c8374fea238e0de1a062b

```md
## Context

The menu module currently models OddsPortal navigation as category lists. `/menu/{sport}/{category}/{league}` calls the fourth-level menu API and renders category-like rows with type, name, and URL. On competition pages such as `https://www.oddsportal.com/football/world/world-championship-2026/`, the page content the user needs is not another taxonomy level; it is a list of matches with scheduled time and downstream event/H2H links.

The project constraints remain unchanged: scraping and parsing stay in `src/`, static rendering stays in `public/`, and communication between them goes through HTTP APIs. The UI must keep the `web_design.md` table/pagination shape.

## Goals / Non-Goals

**Goals:**

- Parse direct match/event rows from OddsPortal competition pages.
- Return event rows with enough structured fields for display and linking: home team, away team, display matchup, start time, OddsPortal URL, and slug/id when available.
- Render fourth-level competition pages as a table of event rows when event rows exist.
- Keep existing category list behavior for pages that expose child categories instead of events.
- Cover parser, API, and frontend behavior with tests before implementation.

**Non-Goals:**

- Parse odds prices or bookmakers for each event in this change.
- Parse every possible OddsPortal sport-specific event layout if it diverges from the football competition markup.
- Follow the H2H link and scrape the destination page.
- Change existing `/api/menu`, `/api/menu/{sport}`, or lower-level category response semantics.

## Decisions

1. Add a dedicated event-row data shape instead of overloading `MenuCategory`.

   Event rows need team fields and start time. Encoding those as pseudo categories would blur the API contract and force the frontend to infer semantics from category fields. A dedicated shape keeps the fourth-level category parser intact and lets the frontend choose an event table only when the API returns event data.

2. Prefer a separate event-list API path or typed payload for fourth-level pages.

   The implementation should keep category APIs backward compatible. The practical route can be either `GET /api/events/{sport}/{category}/{league}` or a typed response from the existing fourth-level handler, but it must not remove fields existing callers expect. The implementation plan will choose the least invasive option after reading current handler structure.

3. Use parser-first extraction with strict URL filtering.

   The scraper should only accept links belonging to the target competition page or OddsPortal H2H/event destinations. It should extract visible team labels and nearby time text from the HTML around the row, then normalize `Mexico - South Africa`, `Mexico vs South Africa`, or split team nodes into the display label `Mexico VS South Africa`.

4. Frontend rendering remains table-based.

   The event table should use the existing `stats-bar`, `pagination`, and `list-table` components. Event pages should show columns for matchup, start time, and link. The link target is the parsed OddsPortal URL, opened as an external destination.

## Risks / Trade-offs

- OddsPortal markup is dynamic or obfuscated -> Mitigation: write parser tests from representative static snippets and keep extraction tolerant around class names, relying primarily on href structure and text content.
- Some competition pages may contain both event links and child category links -> Mitigation: event rows take precedence only when valid event rows are parsed; otherwise fall back to existing category rendering.
- Start time may be relative, localized, or missing -> Mitigation: preserve the display text exactly when an absolute timestamp cannot be normalized, and render an empty/unknown value only when no nearby time exists.
- API shape ambiguity can break frontend callers -> Mitigation: add explicit tests for existing category routes and new event rendering before changing implementation.

## Migration Plan

1. Add tests for event extraction, API response shape, and frontend event-row rendering.
2. Implement the smallest compatible backend API/data model change.
3. Update `public/menu.html` to render event tables only for event payloads.
4. Update documentation and session/change logs.
5. Verify with `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.

Rollback is straightforward: remove the new event API/rendering path and the fourth-level page falls back to existing category-list behavior.

## Open Questions

- The implementation phase should verify the current OddsPortal HTML shape from cached or live data before finalizing the parser selectors.
- If the page exposes multiple market tabs under a match, this change will use the event/H2H destination link only and leave market-specific parsing for a later change.
```

## openspec/changes/event-list-parse/tasks.md

- Source: openspec/changes/event-list-parse/tasks.md
- Lines: 1-23
- SHA256: 86f6ee4afe6e1ba6466525d1df4a7fc1df12b8052ca6e4abd96d6f87b34eb420

```md
## 1. Test Coverage

- [ ] 1.1 Add Rust scraper test for extracting `Mexico VS South Africa`, start time, and OddsPortal H2H link from representative competition HTML.
- [ ] 1.2 Add Rust API/cache tests proving event-list data uses an event-specific cache key and does not overwrite category menu cache.
- [ ] 1.3 Add frontend Node tests for fourth-level event payload rendering, event links, and 10-row pagination.

## 2. Backend Implementation

- [ ] 2.1 Add event row models and serialization shape for event-list API responses.
- [ ] 2.2 Implement OddsPortal competition event-row extraction with filtering for unrelated/navigation links.
- [ ] 2.3 Add event-list handler/routing and storage integration while preserving existing category API behavior.

## 3. Frontend Implementation

- [ ] 3.1 Update `public/menu.html` page configuration to request event-list data for fourth-level competition pages before falling back to category data.
- [ ] 3.2 Render event rows in the table with matchup, start time, and external OddsPortal link while preserving existing category table rendering.
- [ ] 3.3 Keep stats and pagination behavior aligned with `web_design.md` for event rows.

## 4. Documentation and Verification

- [ ] 4.1 Update `web_design.md`, `architect.md`, and `test.md` with event-list API, layout, and test coverage.
- [ ] 4.2 Update `session.md` and `change_log.md` with task status and verification results.
- [ ] 4.3 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js` successfully.
```

## openspec/changes/event-list-parse/specs/event-list-page/spec.md

- Source: openspec/changes/event-list-parse/specs/event-list-page/spec.md
- Lines: 1-30
- SHA256: a2af2ca406edf0eb14c9ff4c37079fa6d3ab53e4ecba0170fda60fb5b10695b9

```md
## ADDED Requirements

### Requirement: Event list API
The system SHALL expose parsed event rows for an OddsPortal competition page without changing existing category menu API contracts.

#### Scenario: Fetch World Championship events
- **WHEN** the client requests event rows for `/football/world/world-championship-2026/`
- **THEN** the response contains `ok: true` and event data with matchup, start time, and OddsPortal link fields

#### Scenario: Empty event extraction fallback
- **WHEN** the target competition page contains no valid event rows
- **THEN** the event response returns an empty event list without persisting an error-shaped category cache entry

### Requirement: Event row extraction
The scraper SHALL extract match/event rows from a football competition page, including home team, away team, display matchup, start time text, and target OddsPortal URL.

#### Scenario: Extract Mexico South Africa row
- **WHEN** the OddsPortal HTML contains an event row for Mexico and South Africa with a link to `/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2`
- **THEN** the scraper returns an event with display matchup `Mexico VS South Africa`, both team names, the row start time, and that link

#### Scenario: Ignore unrelated links
- **WHEN** the OddsPortal HTML contains navigation, results, standings, archive, or unrelated sport links
- **THEN** those links are excluded from event rows

### Requirement: Event list caching
The system SHALL cache event-list data separately from category menu data.

#### Scenario: Event cache does not overwrite category cache
- **WHEN** event rows are saved for `football/world/world-championship-2026`
- **THEN** existing `menu_football_world_world-championship-2026` category cache data is not overwritten with incompatible event JSON
```

## openspec/changes/event-list-parse/specs/menu-third-level-page/spec.md

- Source: openspec/changes/event-list-parse/specs/menu-third-level-page/spec.md
- Lines: 1-19
- SHA256: 4570f53938207c8f915bbec96aeaf4b03db3a8b03857455c9b6778b1d4c29417

```md
## ADDED Requirements

### Requirement: Fourth-level event table rendering
The frontend SHALL render fourth-level competition pages as an event table when event-list data is available.

#### Scenario: Render Mexico South Africa event row
- **WHEN** `/menu/football/world/world-championship-2026` receives an event row for Mexico and South Africa
- **THEN** the list table displays `Mexico VS South Africa`, the event start time, and the parsed OddsPortal link

#### Scenario: Preserve category fallback
- **WHEN** `/menu/{sport}/{category}/{league}` receives category data instead of event data
- **THEN** the page keeps rendering the existing type, name, and URL category table

### Requirement: Event table pagination
The frontend SHALL paginate event rows with the same 10 items per page rule used by menu tables.

#### Scenario: Paginate event rows
- **WHEN** a fourth-level event page has more than 10 parsed events
- **THEN** only 10 event rows are shown on the first page and pagination controls allow navigation to later rows
```

