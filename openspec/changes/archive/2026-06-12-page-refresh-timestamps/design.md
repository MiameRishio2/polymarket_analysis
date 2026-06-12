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
