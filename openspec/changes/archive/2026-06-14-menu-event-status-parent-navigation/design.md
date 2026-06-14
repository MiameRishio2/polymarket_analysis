## Context

The existing menu shell (`public/menu.html`) is reused for root, sport, third-level, and fourth-level menu routes. Fourth-level pages prefer event API data (`/api/events/{sport}/{category}/{league}`) and fall back to category data when no events are available.

Event rows are represented by `EventRow` in `src/menu/events.rs`. They include matchup, start time, OddsPortal URL, and optional Polymarket URL, but no status that tells the UI whether a match has already ended. The UI already has a `top-nav` slot and derives a parent URL from the current path, but the behavior should be made explicit and covered by this change.

## Goals / Non-Goals

**Goals:**

- Add a stable `ended` boolean to event rows so API consumers do not need to infer status independently.
- Derive `ended` from the parsed `start_time` using a conservative rule: a match is ended when its scheduled start time is before the current server time.
- Display event status in the fourth-level menu event table with a compact visual marker.
- Keep parent navigation available on nested menu pages and absent on `/menu`.

**Non-Goals:**

- Do not scrape final scores or exact match result states from OddsPortal.
- Do not change scheduler persistence or monitoring behavior.
- Do not change OddsPortal or Polymarket URL matching rules.

## Decisions

1. Store event status as `ended: bool` on `EventRow`.

   Rationale: the status belongs to the event API contract and is reused by cached responses, stream completion messages, and the frontend. A boolean is enough for the requested "whether ended" marker and avoids overfitting to unavailable score/result data.

   Alternative considered: derive status only in JavaScript. This would make cached API responses less self-describing and duplicate date parsing in the browser.

2. Derive status from `start_time` at extraction time and normalize missing/unparseable dates to `false`.

   Rationale: existing rows already carry `start_time`, and `chrono` is already in use. Treating unknown dates as not ended is conservative because it avoids incorrectly hiding active monitoring opportunities.

   Alternative considered: add an enum such as `scheduled | ended | unknown`. This is more expressive, but the current requirement only asks for ended/non-ended and would require broader UI/API changes.

3. Render status as a separate event table column.

   Rationale: a dedicated column is scannable and does not interfere with external links or scheduler actions.

   Alternative considered: append text into the start-time cell. This is more compact but harder to scan on pages with many events.

4. Keep parent navigation path-derived.

   Rationale: menu pages already map directly to path hierarchy, so slicing one path segment is deterministic and requires no server-side breadcrumbs.

   Alternative considered: pass breadcrumbs from the server. This would add API surface without improving current route behavior.

## Risks / Trade-offs

- Date parsing may fail for some OddsPortal date formats -> default `ended` to `false` and keep the original start time visible.
- Timezone interpretation can differ from the displayed site timezone -> use existing parsed display dates conservatively and cover the known World Cup format with tests.
- Adding a new serialized field affects cached raw JSON -> serde defaults keep older cached rows loadable with `ended: false`.

## Migration Plan

No database schema migration is required because event rows are stored as raw JSON. Older cached rows deserialize with the default `ended: false`; refreshed event data will include the field.
