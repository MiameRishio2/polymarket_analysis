## Why

Deep menu pages such as `/menu/football/world/world-championship-2026/` currently show event rows without an explicit finished/open marker, making it hard to decide which matches still need attention. Nested menu pages also need a consistent parent navigation affordance so users can move back up the hierarchy without manually editing the URL.

## What Changes

- Add an explicit ended/open status to competition event rows returned by the menu event APIs.
- Render the event status in the fourth-level menu event table.
- Ensure every nested `/menu/...` page exposes a clear "back to parent" navigation target, while the root `/menu` page remains without a parent link.
- Keep existing OddsPortal, Polymarket, scheduler, refresh, and pagination behavior intact.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `menu-third-level-page`: Add event finished/open status behavior and consistent parent navigation behavior for nested menu pages.

## Impact

- `src/menu/events.rs`: event row model, event extraction, stream progress/completion payloads, and tests.
- `public/menu.html`: nested page navigation rendering, event table columns, status badge styling, and client handling of cached/refreshed event rows.
- `tests/` or module tests: coverage for event status derivation and parent navigation behavior.
