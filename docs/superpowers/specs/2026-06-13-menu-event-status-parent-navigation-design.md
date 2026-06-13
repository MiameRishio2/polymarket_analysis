---
comet_change: menu-event-status-parent-navigation
role: technical-design
canonical_spec: openspec
---

# Menu Event Status and Parent Navigation Design

## Context

The menu UI uses one static shell, `public/menu.html`, for `/menu`, sport pages, third-level pages, and fourth-level competition pages. Fourth-level pages first request event data from `/api/events/{sport}/{category}/{league}` and only fall back to category data when no events are available.

Event rows are modeled in `src/menu/events.rs` as `EventRow`. The current row contains matchup, teams, start time, OddsPortal URL, and optional Polymarket URL. It does not expose whether the event has already ended, so the World Cup page cannot distinguish events that still need attention from events that are already in the past.

The menu shell already has a `top-nav` container and a path-derived `getParentMenuHref()` helper. The implementation should preserve that lightweight route model and make the behavior explicit in tests.

## Technical Approach

Add a stable `ended: bool` field to `EventRow`.

The backend will derive `ended` when building event rows:

- Past parsed start time -> `ended: true`
- Future parsed start time -> `ended: false`
- Empty or unparseable start time -> `ended: false`

The field should use `#[serde(default)]` so older cached event JSON remains loadable. It should not use `skip_serializing_if`; API consumers need a predictable boolean.

The calculation should live in `src/menu/events.rs` near the event parsing helpers. It can reuse existing date parsing patterns used for Polymarket date candidates, but should keep a separate helper such as `event_has_ended(start_time, now)` so tests can provide a fixed clock. The public extraction flow can call a `event_has_ended_now(start_time)` wrapper.

The frontend will update the event table from three columns to four:

1. Matchup
2. Start time
3. Status
4. Links/actions

The status cell will render compact badges:

- `ended: true` -> `已结束`
- missing or false `ended` -> `未结束`

Existing OddsPortal, Polymarket, and scheduler controls remain in the links/actions column. The fourth-level category fallback table stays unchanged.

Parent navigation stays path-derived:

- `/menu` -> no parent link
- `/menu/football` -> `/menu`
- `/menu/football/world` -> `/menu/football`
- `/menu/football/world/world-championship-2026/` -> `/menu/football/world`

No breadcrumb API or server-side route metadata is needed.

## Data Flow

```text
OddsPortal page
  -> extract_events_for_competition()
  -> push_event_row(... start_time ...)
  -> EventRow { ended, ... }
  -> /api/events/... or refresh stream complete message
  -> public/menu.html event table
  -> status badge + existing event links/actions
```

Cached event data uses `save_raw_json` with serialized `Vec<EventRow>`. Because `ended` has a serde default, older cached rows deserialize cleanly and will gain explicit `ended` values after the next refresh.

## Error Handling

Date parsing must be conservative. A malformed or empty `start_time` must not fail event extraction and must not mark a row ended. The UI should also tolerate missing `ended` from stale browser data by treating it as `false`.

Network, Polymarket enrichment, scheduler, and category fallback error handling are unchanged.

## Testing Strategy

Rust tests in `tests/menu_third_level_test.rs` should cover:

- Extracted event rows include `ended`.
- A fixed past start time produces `ended: true`.
- A fixed future start time produces `ended: false`.
- Empty or unparseable start time produces `ended: false`.
- Event rows serialized before this field existed still deserialize with `ended: false`.
- Event API/cache tests continue to preserve `refreshed_at` and include the new boolean.

Frontend VM tests in `tests/menu_page_config_test.js` should cover:

- Event table renders the new status column and both status labels.
- OddsPortal, Polymarket, and scheduler actions remain visible with status enabled.
- Root `/menu` renders no parent link.
- Nested pages render the immediate parent link.

Final verification should run:

```bash
node tests/menu_page_config_test.js
cargo test --test menu_third_level_test
cargo test --all
```

If local port binding tests fail under the sandbox, rerun the affected Rust test command with approval and record the reason.

## Risks and Trade-offs

- OddsPortal display times may omit timezone context. The implementation accepts this limitation and uses the existing displayed time convention consistently.
- A boolean status cannot represent canceled, postponed, live, or unknown states. That is intentional for this change because the requested behavior is only whether the event is ended.
- Older cached rows will load as not ended until refresh. This is safer than marking unknown data as ended and avoids a cache migration.

## Open Questions

None. The implementation can proceed with the boolean status model and path-derived parent navigation.
