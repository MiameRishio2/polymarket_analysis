## Context

The current application serves menu pages and event lists through one Axum router backed by `src/menu/storage.rs` and `data/menu.db`. Competition pages such as `/menu/football/world/world-championship-2026/` prefer event data from `/api/events/{sport}/{category}/{league}` and render each event with OddsPortal and Polymarket links.

Older code in `src_old/scheduler.rs` stored scheduled matches in JSON, but the current application uses SQLite for durable web state. The scheduler should therefore live with the current `src/menu` web surface and reuse the existing SQLite file.

## Goals / Non-Goals

**Goals:**
- Persist scheduled matches in SQLite.
- Add a small manager module under `src` for scheduler storage and API behavior.
- Let users add event rows from competition pages into the scheduler.
- Track whether monitoring has started for each scheduled match.
- Show all scheduler entries on `/menu`.

**Non-Goals:**
- Implement the background monitoring worker itself.
- Change OddsPortal scraping logic beyond using existing event rows as scheduler input.
- Replace the existing menu cache schema.
- Add user accounts or multi-user permissions.

## Decisions

1. Store scheduler rows in a dedicated SQLite table in `data/menu.db`.

   The existing web server already initializes one SQLite-backed `Storage`, so adding a table keeps deployment simple and transactional enough for a single local application. The alternative was a separate `scheduler.db`, but that would duplicate connection setup and complicate startup configuration without a clear benefit.

2. Add `src/menu/scheduler.rs` with scheduler models and methods.

   The scheduler is tightly coupled to menu/event pages, so placing it under `src/menu` keeps route handlers close to the current `AppState`. The alternative was a top-level `src/scheduler.rs`; that is reasonable later if monitoring becomes a separate runtime subsystem.

3. Extend the existing `AppState` route tree instead of adding a second router.

   Scheduler endpoints will use the same `Arc<Storage>` state and response envelope as menu APIs. This keeps CORS, page serving, and SQLite initialization consistent with existing handlers.

4. Use deterministic upsert keys for scheduler entries.

   Entries should be idempotent when the same event is added repeatedly. The primary key will be derived from the event URL when available, falling back to matchup and start time. This avoids duplicate rows from repeated button clicks.

5. Model monitor state as an explicit boolean.

   The new field `monitoring_started` records whether the scheduler entry is active for monitoring. It is separate from match status so future monitoring code can update match status without losing the user's monitoring decision.

## Risks / Trade-offs

- [Risk] Event rows may lack a complete URL or start time. -> Mitigation: accept partial metadata and derive IDs from the best available stable fields.
- [Risk] A button click can race with a page refresh or duplicate click. -> Mitigation: use SQLite upsert semantics keyed by deterministic IDs.
- [Risk] The `/menu` page can become crowded. -> Mitigation: render the scheduler as a compact top-level section only on the root menu page.
- [Risk] Future monitoring may need more states than a boolean. -> Mitigation: keep `monitoring_started` as the current contract and leave status fields independent for future expansion.
