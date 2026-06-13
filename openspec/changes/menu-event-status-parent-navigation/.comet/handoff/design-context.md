# Comet Design Handoff

- Change: menu-event-status-parent-navigation
- Phase: design
- Mode: compact
- Context hash: 1d14a2cafbcbbb4ac0ed84e81da6ae7b4d4272c321219021eb56b40f45b76819

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/menu-event-status-parent-navigation/proposal.md

- Source: openspec/changes/menu-event-status-parent-navigation/proposal.md
- Lines: 1-26
- SHA256: 5ceda01f523b9cf43d7ade53a154912e011eb03a4272574dc3d3e3517edc75ca

```md
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
```

## openspec/changes/menu-event-status-parent-navigation/design.md

- Source: openspec/changes/menu-event-status-parent-navigation/design.md
- Lines: 1-56
- SHA256: 215df28f24e8dbf0b1ff8e12b7d804ad8ef44ed5551ec08906969df45743854c

```md
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
```

## openspec/changes/menu-event-status-parent-navigation/tasks.md

- Source: openspec/changes/menu-event-status-parent-navigation/tasks.md
- Lines: 1-9
- SHA256: 11ff3e4fbf8558c473ec79cfd44091f8d909ef31d6364befedf00ef8f811478f

```md
# Tasks

- [ ] 1. Add `ended` to `EventRow` with serde compatibility for older cached event JSON.
- [ ] 2. Implement server-side ended-status derivation from existing event `start_time` values.
- [ ] 3. Update event refresh stream/progress completion payloads to carry the new event row field.
- [ ] 4. Render event ended/open markers in the fourth-level menu event table.
- [ ] 5. Verify and tighten parent navigation behavior for `/menu`, sport, third-level, and fourth-level pages.
- [ ] 6. Add focused Rust and frontend tests for event status and parent navigation.
- [ ] 7. Run project verification commands.
```

## openspec/changes/menu-event-status-parent-navigation/specs/menu-third-level-page/spec.md

- Source: openspec/changes/menu-event-status-parent-navigation/specs/menu-third-level-page/spec.md
- Lines: 1-46
- SHA256: 88d863075b0393361e7389c2f28156b4d1753f0161fb906ba11873c9c6e02228

```md
## ADDED Requirements

### Requirement: Competition event ended marker
Competition event rows returned for `/menu/{sport}/{category}/{league}` SHALL include an `ended` boolean that identifies whether the scheduled event start time is before the current server time.

#### Scenario: Future event is open
- **WHEN** an event row has a future `start_time`
- **THEN** the API response contains `ended: false` for that event

#### Scenario: Past event is ended
- **WHEN** an event row has a past `start_time`
- **THEN** the API response contains `ended: true` for that event

#### Scenario: Unknown event time is open
- **WHEN** an event row has an empty or unparseable `start_time`
- **THEN** the API response contains `ended: false` for that event

### Requirement: Competition event status display
The fourth-level menu event table SHALL display each event row's ended/open status without removing OddsPortal, Polymarket, or scheduler actions.

#### Scenario: Render ended event row
- **WHEN** the browser renders an event row with `ended: true`
- **THEN** the row displays an ended status marker

#### Scenario: Render open event row
- **WHEN** the browser renders an event row with `ended: false`
- **THEN** the row displays an open status marker

#### Scenario: Preserve event actions
- **WHEN** the browser renders event status markers
- **THEN** OddsPortal links, Polymarket links, and scheduler actions remain available according to the row data

### Requirement: Parent navigation for nested menu pages
Nested menu pages SHALL expose a parent navigation link that targets the immediate parent menu path, while the root `/menu` page SHALL not render a parent link.

#### Scenario: Root menu has no parent link
- **WHEN** the browser renders `/menu`
- **THEN** no parent navigation link is shown

#### Scenario: Sport menu parent link
- **WHEN** the browser renders `/menu/football`
- **THEN** the parent navigation link targets `/menu`

#### Scenario: Competition menu parent link
- **WHEN** the browser renders `/menu/football/world/world-championship-2026/`
- **THEN** the parent navigation link targets `/menu/football/world`
```

