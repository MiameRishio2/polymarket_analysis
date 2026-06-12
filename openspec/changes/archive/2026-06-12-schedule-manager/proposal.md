## Why

Competition event pages already expose candidate matches, but there is no durable way to mark which matches should be watched later. Users need to add matches from pages such as `/menu/football/world/world-championship-2026/`, decide whether monitoring is enabled, and see all scheduled matches from the `/menu` landing page.

## What Changes

- Add a scheduler manager under `src` that stores scheduled matches in SQLite.
- Add APIs for listing scheduler entries, adding or updating a scheduled match, deleting an entry, and toggling whether monitoring has started.
- Add a scheduler action next to event links on competition pages so event rows can be added to the scheduler.
- Render all scheduler entries under `/menu`, including monitor state, source URLs, and basic match metadata.
- Preserve the existing menu and event cache behavior.

## Capabilities

### New Capabilities
- `schedule-manager`: Scheduler storage, API, and menu UI behavior for tracked matches.

### Modified Capabilities
- `menu-third-level-page`: Competition event pages gain scheduler entry actions for match rows.

## Impact

- Affected code: `src/menu`, `src/lib.rs`, `src/main.rs`, `public/menu.html`, and tests.
- Data: adds a new SQLite table in `data/menu.db`.
- APIs: adds scheduler endpoints under the existing Axum application.
- No new external runtime service is required.
