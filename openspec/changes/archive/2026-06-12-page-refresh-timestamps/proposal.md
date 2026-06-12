## Why

Menu pages currently show fetched category or event data without a reliable page-level refresh timestamp. Operators need to see when each cached page was last refreshed, including deep pages such as `/menu/football/world/world-championship-2026/`, and existing SQLite rows need a safe default when no refresh timestamp has been recorded yet.

## What Changes

- Add a dedicated SQLite refresh timestamp field for cached menu and event payloads.
- Backfill existing cache rows with a default refresh timestamp of `1970-01-01T00:00:00Z` when the field is missing.
- Update cache writes so successful refreshes store the current refresh timestamp.
- Return the refresh timestamp from all menu and event API responses.
- Display the refresh timestamp on every menu page level: root `/menu`, sport `/menu/{sport}`, category `/menu/{sport}/{category}`, and competition `/menu/{sport}/{category}/{league}`.

## Capabilities

### New Capabilities
- `page-refresh-timestamps`: Tracks and presents page-level refresh timestamps for cached menu and event pages.

### Modified Capabilities
- `menu-third-level-page`: Extend menu page requirements so nested menu pages expose their latest refresh timestamp.

## Impact

- SQLite `category_cache` schema and migration path.
- Storage load/save methods for category and raw event JSON payloads.
- Menu and event API response models.
- `public/menu.html` refresh-time rendering for all menu page depths.
- Rust and JavaScript tests covering default, cached, and refreshed timestamp behavior.
