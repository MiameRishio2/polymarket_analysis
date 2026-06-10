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
