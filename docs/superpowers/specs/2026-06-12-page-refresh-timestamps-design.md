---
comet_change: page-refresh-timestamps
role: technical-design
canonical_spec: openspec
archived-with: 2026-06-12-page-refresh-timestamps
status: final
---

# Page Refresh Timestamps Design

## Scope

Add a page-level refresh timestamp to the menu cache, API responses, and shared menu UI. The timestamp applies to every menu depth served by `public/menu.html`: `/menu`, `/menu/{sport}`, `/menu/{sport}/{category}`, and `/menu/{sport}/{category}/{league}`. Fourth-level pages that render event data use the event cache timestamp.

## Data Model

SQLite table `category_cache` gets a dedicated user-facing field:

```sql
refreshed_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'
```

`updated_at` remains the SQLite write bookkeeping field. `last_updated` remains a compatibility field already present in `CategoryData` and `EventData`. UI display and new tests use `refreshed_at`.

Existing databases are migrated in `Storage::init_schema()`:

1. Create new tables with `refreshed_at`.
2. Inspect `PRAGMA table_info(category_cache)`.
3. If missing, run `ALTER TABLE category_cache ADD COLUMN refreshed_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'`.

This keeps the migration additive and safe for existing `data/menu.db` files.

## API Model

`CategoryData` and `EventData` include:

```rust
pub refreshed_at: String
```

Fresh fetches and refresh handlers set `refreshed_at` to `chrono::Utc::now().to_rfc3339()`. Cache loads read `refreshed_at` from SQLite and default to `1970-01-01T00:00:00Z` if the field is blank or unavailable during migration-oriented paths.

Storage methods own persistence:

- `Storage::save()` writes category JSON, `last_updated`, `source`, and `refreshed_at`.
- `Storage::save_raw_json()` writes event JSON with `refreshed_at`.
- `Storage::load()` returns `CategoryData` with the DB `refreshed_at`.
- `Storage::load_raw_json()` returns raw JSON plus `last_updated`, `source`, and `refreshed_at`.

## UI Behavior

`public/menu.html` keeps a page-level `refreshedAt` state. `fetchData()` and `refreshData()` update that state from the API response before rendering. `renderStats()` includes a stable badge:

```text
刷新时间: <timestamp>
```

If a response omits the field, the UI displays `1970-01-01T00:00:00Z`.

Fourth-level pages first try the event API. When event rows are displayed, the visible timestamp comes from `EventData.refreshed_at`. If the page falls back to category data, it uses `CategoryData.refreshed_at`.

## Testing Strategy

- Storage tests cover migrating an existing table without `refreshed_at`.
- Storage tests cover saving and loading category data with a non-epoch refresh time.
- Event storage tests cover raw JSON load/save returning `refreshed_at`.
- Menu page JavaScript tests cover timestamp display state for category and event responses.
- Existing menu route tests continue to cover nested URL behavior.

## Risks

- Some current files are already modified by another change. Implementation must preserve those changes and only add the refresh timestamp behavior on top.
- `public/menu.html` is shared by all page depths. The UI change must update one shared rendering path rather than duplicating per-depth logic.
- Fourth-level pages have dual data modes. The timestamp source must follow the active mode.
