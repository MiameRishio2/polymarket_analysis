---
comet_change: schedule-manager
role: technical-design
canonical_spec: openspec
---

# Schedule Manager Technical Design

## Context

The current web server is a single Axum application that serves `/menu`, nested menu pages, event APIs, and SQLite inspection pages. Menu and event data are persisted through `src/menu/storage.rs` in `data/menu.db`. Fourth-level competition pages such as `/menu/football/world/world-championship-2026/` first try `/api/events/{sport}/{category}/{league}` and render event rows with OddsPortal and Polymarket links.

The scheduler belongs to this same web surface for this change: users add matches from event rows, then inspect all scheduled matches from the root `/menu` page. The durable state must live in SQLite, not the legacy JSON cache from `src_old/scheduler.rs`.

## Architecture

Add `src/menu/scheduler.rs` as a focused module with:

- Serializable models for scheduler API payloads and responses.
- Storage methods implemented on `Storage`, reusing its internal SQLite connection.
- A deterministic id builder so repeated scheduling of the same event performs an upsert.
- Axum handlers for list, upsert, monitor toggle, and delete.

The existing `AppState` stays unchanged structurally: scheduler handlers receive `State<AppState>` and use `state.storage.as_ref()`. Routes are attached in `create_router` beside the existing menu and event routes.

## Data Model

Create a `scheduled_matches` table in `data/menu.db`:

- `id TEXT PRIMARY KEY`
- `matchup TEXT NOT NULL`
- `home_team TEXT`
- `away_team TEXT`
- `start_time TEXT`
- `oddsportal_url TEXT`
- `polymarket_url TEXT`
- `source_page TEXT`
- `monitoring_started INTEGER NOT NULL DEFAULT 0`
- `added_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

`Storage::init_schema` will create this table during normal startup. Existing `category_cache` rows are untouched.

The upsert id source order is:

1. `oddsportal_url`
2. `polymarket_url`
3. `matchup|start_time|source_page`

This keeps row creation idempotent when a user clicks the scheduler action more than once.

## API

Add endpoints under the current application:

- `GET /api/scheduler` returns all scheduled matches.
- `POST /api/scheduler` creates or updates a scheduled match.
- `POST /api/scheduler/:id/monitoring` updates `monitoring_started`.
- `DELETE /api/scheduler/:id` removes a scheduled match.

Responses use the existing `ApiResponse<T>` envelope. Storage failures become `ok: false` JSON responses rather than panics.

## Frontend

`public/menu.html` will keep the current page shell. The JavaScript gains a small scheduler state map loaded from `/api/scheduler`.

On fourth-level event pages, each event row will render:

- Existing OddsPortal link.
- Existing Polymarket link.
- Scheduler action button.

After a successful schedule request, the button state changes to scheduled and the scheduler map is refreshed.

On root `/menu`, the page renders a compact scheduler section above the sports list. Each row shows matchup, start time, links, monitor state, a monitor toggle, and delete action. Empty scheduler state is displayed without hiding the sports list.

## Testing

Rust tests will use temporary SQLite files to verify:

- Scheduler table creation does not block menu storage initialization.
- Upsert creates one row for duplicate event metadata.
- Monitoring toggles persist.
- Delete removes the row.
- List ordering is deterministic.

Frontend tests will inspect `public/menu.html` as source to ensure the scheduler endpoints, action renderer, root scheduler display, and monitor/delete controls are present. This matches the existing lightweight frontend test style in the repository.

## Risks

- Event metadata can be incomplete. The API accepts nullable team and URL fields, using the best available id source.
- The root menu can become visually crowded. The scheduler display is compact and only appears on the root menu page.
- Future background monitoring may need richer lifecycle states. This change intentionally stores only `monitoring_started`, leaving match result/status tracking for later.
