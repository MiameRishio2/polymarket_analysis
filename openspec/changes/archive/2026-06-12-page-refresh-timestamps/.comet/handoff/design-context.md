# Comet Design Handoff

- Change: page-refresh-timestamps
- Phase: design
- Mode: compact
- Context hash: f41e4a650123bad4460d9f4a19c7b6d461f9a97c57e8b57e24e345af0c1ae952

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/page-refresh-timestamps/proposal.md

- Source: openspec/changes/page-refresh-timestamps/proposal.md
- Lines: 1-27
- SHA256: 5a7841aa8a76cf117b32260e93c089c037d8531806259d110169fe5fc7e8507e

```md
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
```

## openspec/changes/page-refresh-timestamps/design.md

- Source: openspec/changes/page-refresh-timestamps/design.md
- Lines: 1-67
- SHA256: 5359bb78c7c602e4afb70982f5478ef211480b327773cbe9a5fd21d9ea05f513

```md
## Context

The menu application stores category and event payloads in SQLite table `category_cache`. Each row currently has `last_updated` in the response payload and `updated_at` as a SQLite bookkeeping timestamp, but there is no dedicated persisted field that represents the page refresh time shown to users. The shared `public/menu.html` shell serves every menu depth, including `/menu`, `/menu/{sport}`, `/menu/{sport}/{category}`, and `/menu/{sport}/{category}/{league}`.

Existing code also stores fourth-level event lists as raw JSON through the same table. The refresh timestamp design must therefore work for both category payloads and event payloads.

## Goals / Non-Goals

**Goals:**
- Persist a dedicated refresh timestamp for every cache key in SQLite.
- Preserve existing databases by adding the field through an idempotent migration.
- Backfill rows that do not have a refresh timestamp with `1970-01-01T00:00:00Z`.
- Return the persisted refresh timestamp from menu/category/event APIs.
- Render the refresh timestamp consistently in the shared menu page UI for every page depth.

**Non-Goals:**
- Change OddsPortal scraping behavior.
- Change cache key naming.
- Add live auto-refresh or background polling.
- Redesign the SQLite browser page.

## Decisions

1. Store refresh time in a dedicated `refreshed_at` column.

   `updated_at` remains an internal SQLite write timestamp, while `refreshed_at` is the user-facing refresh timestamp. This avoids overloading `updated_at` and keeps the API contract explicit.

   Alternative considered: reuse `last_updated`. This is simpler, but it leaves old rows and raw event JSON tied to payload-specific semantics instead of a storage-level refresh timestamp.

2. Migrate existing SQLite tables during `Storage::init_schema`.

   The storage initializer already runs for the application and tests, so it is the right place to detect whether `category_cache.refreshed_at` exists and add it when missing. The migration will set the column default to `1970-01-01T00:00:00Z` for historical rows.

   Alternative considered: create a separate migration runner. That is unnecessary for the current single-table embedded SQLite setup.

3. Expose `refreshed_at` on category and event response structs.

   `CategoryData` and `EventData` will include `refreshed_at` with serde default handling so older JSON construction remains manageable during migration. Storage loads should prefer the database field and fall back to the default epoch if absent or blank.

   Alternative considered: wrap all API responses in metadata. That would touch every handler and frontend consumer more broadly than needed.

4. Frontend stores and renders the last response timestamp.

   `public/menu.html` will keep a page-level `refreshedAt` state populated from either category data or event data. The stats/header area will display a stable label such as `刷新时间: <timestamp>`, using the epoch default when the API returns no value.

   Alternative considered: per-row timestamps. The user requested page refresh time per page, so a page-level display is clearer.

## Risks / Trade-offs

- Existing databases may already have the table without the new column -> use `PRAGMA table_info` before `ALTER TABLE`.
- Some handler branches produce error data that should not be persisted -> still return a timestamp for display, but only successful persistable data updates SQLite.
- Fourth-level pages prefer event data when events exist -> the visible timestamp must come from the event API response in that mode, otherwise from category data.
- `ALTER TABLE ... ADD COLUMN ... DEFAULT` support differs across SQLite versions -> use a text column with a string default and keep the migration simple.

## Migration Plan

1. Extend schema initialization to create new databases with `refreshed_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'`.
2. For existing databases, detect missing `refreshed_at` and add it with the same default.
3. Update save paths to write the supplied/current refresh timestamp.
4. Update load paths to read and return `refreshed_at`, defaulting to the epoch value if necessary.
5. Update frontend rendering and tests.

Rollback: older code can ignore the extra SQLite column; the migration is additive.

## Open Questions

- None. The requested default for missing historical values is `1970-01-01T00:00:00Z`.
```

## openspec/changes/page-refresh-timestamps/tasks.md

- Source: openspec/changes/page-refresh-timestamps/tasks.md
- Lines: 1-23
- SHA256: 56b5b264509e33fa6a16192ae53f6b86e0213afe686170816957e1757fc1d53e

```md
## 1. SQLite Storage

- [ ] 1.1 Add `refreshed_at` to new `category_cache` table creation with default `1970-01-01T00:00:00Z`
- [ ] 1.2 Add an idempotent migration for existing databases missing `refreshed_at`
- [ ] 1.3 Update category and raw JSON save/load paths to persist and return `refreshed_at`

## 2. API Models

- [ ] 2.1 Add `refreshed_at` to `CategoryData` with epoch default handling
- [ ] 2.2 Add `refreshed_at` to `EventData` with epoch default handling
- [ ] 2.3 Ensure all menu and event handlers return refresh timestamps for cache hits, fresh fetches, and refresh responses

## 3. Menu UI

- [ ] 3.1 Track the active page refresh timestamp in `public/menu.html`
- [ ] 3.2 Render the refresh timestamp on root, sport, third-level, and fourth-level pages
- [ ] 3.3 Use event response timestamps when fourth-level pages display event data

## 4. Verification

- [ ] 4.1 Add or update Rust storage/API tests for migration default and refreshed timestamp persistence
- [ ] 4.2 Add or update JavaScript menu page tests for timestamp rendering across page levels
- [ ] 4.3 Run targeted Rust and JavaScript tests covering the refresh timestamp change
```

## openspec/changes/page-refresh-timestamps/specs/menu-third-level-page/spec.md

- Source: openspec/changes/page-refresh-timestamps/specs/menu-third-level-page/spec.md
- Lines: 1-16
- SHA256: 926dfe5946c4e5a920136b0cee042c6cb7585c546538d9580b6026eefacf5b9e

```md
## MODIFIED Requirements

### Requirement: Third-level menu API
The system SHALL expose third-level category data at `/api/menu/{sport}/{category}` and refresh it at `/api/menu/{sport}/{category}/refresh`, including the page refresh timestamp in each response.

#### Scenario: Fetch football Argentina children
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response contains `ok: true` and `data.sport` identifies the third-level path

#### Scenario: Refresh football Argentina children
- **WHEN** the client posts to `/api/menu/football/argentina/refresh`
- **THEN** the system fetches data from OddsPortal and stores it under a third-level cache key

#### Scenario: Third-level response includes refresh time
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response data contains `refreshed_at`
```

## openspec/changes/page-refresh-timestamps/specs/page-refresh-timestamps/spec.md

- Source: openspec/changes/page-refresh-timestamps/specs/page-refresh-timestamps/spec.md
- Lines: 1-57
- SHA256: 71bb382e4b39b4ea6b3766555f515ff465c717ceb105ccb35ab4a32de589b835

```md
## ADDED Requirements

### Requirement: Persist page refresh timestamp
The system SHALL persist a dedicated refresh timestamp for each cached menu or event page in SQLite.

#### Scenario: New cache row stores refresh time
- **WHEN** the system saves refreshed category or event data for a cache key
- **THEN** the SQLite row for that cache key contains a non-empty `refreshed_at` timestamp representing the refresh time

#### Scenario: Existing cache row without refresh time
- **WHEN** the application opens an existing SQLite database whose cache rows do not have refresh timestamps
- **THEN** the system adds the refresh timestamp field and treats existing rows as refreshed at `1970-01-01T00:00:00Z`

### Requirement: Return page refresh timestamp
The system SHALL return the persisted page refresh timestamp in API data for every menu and event page level.

#### Scenario: Fetch root menu page data
- **WHEN** the client requests `/api/menu`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch sport menu page data
- **WHEN** the client requests `/api/menu/football`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch nested category menu page data
- **WHEN** the client requests `/api/menu/football/world`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch competition menu page data
- **WHEN** the client requests `/api/menu/football/world/world-championship-2026`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch competition event page data
- **WHEN** the client requests `/api/events/football/world/world-championship-2026`
- **THEN** the response data includes `refreshed_at`

### Requirement: Update timestamp on refresh
The system SHALL update the persisted page refresh timestamp when a page refresh successfully stores new data.

#### Scenario: Refresh root menu page
- **WHEN** the client posts to `/api/menu/refresh` and the system stores refreshed data
- **THEN** the persisted `refreshed_at` value for the root menu cache key is updated from the previous value

#### Scenario: Refresh nested competition events
- **WHEN** the client posts to `/api/events/football/world/world-championship-2026/refresh` and the system stores refreshed event data
- **THEN** the persisted `refreshed_at` value for the event cache key is updated from the previous value

### Requirement: Display page refresh timestamp
The menu UI SHALL display the page refresh timestamp for every menu page level.

#### Scenario: Open root menu page
- **WHEN** the browser renders `/menu`
- **THEN** the page shows a refresh timestamp label populated from the API response or `1970-01-01T00:00:00Z` when unavailable

#### Scenario: Open competition page
- **WHEN** the browser renders `/menu/football/world/world-championship-2026/`
- **THEN** the page shows a refresh timestamp label populated from the event API response when event data is displayed
```

