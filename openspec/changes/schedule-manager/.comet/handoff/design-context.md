# Comet Design Handoff

- Change: schedule-manager
- Phase: design
- Mode: compact
- Context hash: 0d4f3adca9249ee313e5e14fa137fe0a258bb3e17d342a9037924f0d1e38b5a4

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/schedule-manager/proposal.md

- Source: openspec/changes/schedule-manager/proposal.md
- Lines: 1-26
- SHA256: b1f6f7f8a5f5b67aed71d9d48343889e138439e29b43e450213203d939d3a612

```md
## Why

Competition event pages already expose candidate matches, but there is no durable way to mark which matches should be watched later. Users need to add matches from pages such as `/menu/football/world/world-championship-2026/`, decide whether monitoring is enabled, and see all scheduled matches from the `/menu` landing page.

## What Changes

- Add a scheduler manager under `src` that stores scheduled matches in SQLite.
- Add APIs for listing scheduler entries, adding or updating a scheduled match, deleting an entry, and toggling whether monitoring has started.
- Add a scheduler action next to event links on competition pages so event rows can be added to the scheduler.
- Render all scheduler entries under `/menu`, including monitor state, source URLs, and basic match metadata.
- Preserve the existing menu and event cache behavior.

## Capabilities

### New Capabilities
- `schedule-manager`: Scheduler storage, API, and menu UI behavior for tracked matches.

### Modified Capabilities
- `menu-third-level-page`: Competition event pages gain scheduler entry actions for match rows.

## Impact

- Affected code: `src/menu`, `src/lib.rs`, `src/main.rs`, `public/menu.html`, and tests.
- Data: adds a new SQLite table in `data/menu.db`.
- APIs: adds scheduler endpoints under the existing Axum application.
- No new external runtime service is required.
```

## openspec/changes/schedule-manager/design.md

- Source: openspec/changes/schedule-manager/design.md
- Lines: 1-49
- SHA256: a103367b5177ed1b9b27c307d8473aeb8d871c15aa25f44b4ac96762505c52dd

```md
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
```

## openspec/changes/schedule-manager/tasks.md

- Source: openspec/changes/schedule-manager/tasks.md
- Lines: 1-18
- SHA256: e87c5d65114a27cf55f2a4cf3d0bce4874519176def295ec57d6d08917890a85

```md
## 1. Scheduler Storage and API

- [ ] 1.1 Add scheduler models and SQLite table initialization under `src/menu`.
- [ ] 1.2 Implement scheduler list, upsert, monitor toggle, and delete storage methods.
- [ ] 1.3 Add Axum scheduler handlers and routes using the existing API response envelope.
- [ ] 1.4 Export scheduler module paths needed by tests and the application.

## 2. Menu UI Integration

- [ ] 2.1 Add event-row scheduler actions on fourth-level competition pages.
- [ ] 2.2 Load scheduler state in the menu frontend and reflect scheduled rows.
- [ ] 2.3 Render all scheduler entries on the root `/menu` page with monitor controls and delete actions.

## 3. Verification

- [ ] 3.1 Add focused Rust tests for scheduler storage behavior.
- [ ] 3.2 Add or update frontend behavior tests for scheduler actions and root scheduler display.
- [ ] 3.3 Run formatting and the relevant test suite.
```

## openspec/changes/schedule-manager/specs/menu-third-level-page/spec.md

- Source: openspec/changes/schedule-manager/specs/menu-third-level-page/spec.md
- Lines: 1-16
- SHA256: f1e1926aac24741bf72778fd41a4e8f1c6014f09d73c1264350b5de670fbab16

```md
## ADDED Requirements

### Requirement: Competition event scheduler action
Competition event rows rendered under `/menu/{sport}/{category}/{league}` SHALL include a scheduler action when event data is displayed.

#### Scenario: Render scheduler action on World Championship page
- **WHEN** the browser renders event rows for `/menu/football/world/world-championship-2026/`
- **THEN** each event row includes an action that can add the event to the scheduler

#### Scenario: Schedule event from row
- **WHEN** the user activates the scheduler action for an event row
- **THEN** the frontend posts the event metadata to the scheduler API and shows the row as scheduled after the request succeeds

#### Scenario: Preserve external links
- **WHEN** a competition event row includes OddsPortal or Polymarket links
- **THEN** the scheduler action does not remove or disable those external links
```

## openspec/changes/schedule-manager/specs/schedule-manager/spec.md

- Source: openspec/changes/schedule-manager/specs/schedule-manager/spec.md
- Lines: 1-56
- SHA256: 34987249636b2cc476d58363a5629b1e510e02c930141c92026fd8a7820d42e5

```md
## ADDED Requirements

### Requirement: Scheduler SQLite storage
The system SHALL persist scheduled matches in SQLite using the same database file as the menu cache.

#### Scenario: Initialize scheduler storage
- **WHEN** the application starts with an existing or new menu database
- **THEN** the scheduler table exists without removing existing menu cache rows

#### Scenario: Upsert scheduled match
- **WHEN** the same event is added to the scheduler more than once
- **THEN** the system stores one scheduler row and updates its mutable fields

### Requirement: Scheduler match metadata
The system SHALL store enough metadata for each scheduled match to identify, display, and monitor it.

#### Scenario: Store event row as scheduled match
- **WHEN** a client schedules an event row from a competition page
- **THEN** the persisted row includes matchup, home team when available, away team when available, start time, OddsPortal URL, Polymarket URL when available, monitoring state, added timestamp, and updated timestamp

### Requirement: Scheduler monitor toggle
The system SHALL expose whether monitoring has started for each scheduled match and allow that state to be changed.

#### Scenario: Add match with monitoring disabled
- **WHEN** a client adds a match without specifying monitor state
- **THEN** the scheduler entry has `monitoring_started` set to false

#### Scenario: Toggle monitor state
- **WHEN** a client updates a scheduler entry monitor state
- **THEN** the persisted scheduler entry reflects the new `monitoring_started` value

### Requirement: Scheduler API
The system SHALL expose scheduler APIs under the existing web server.

#### Scenario: List scheduled matches
- **WHEN** a client requests the scheduler list API
- **THEN** the response contains all scheduler entries ordered by monitoring state, start time, and matchup

#### Scenario: Add scheduled match
- **WHEN** a client posts a valid scheduled match payload
- **THEN** the response contains the persisted scheduler entry

#### Scenario: Delete scheduled match
- **WHEN** a client deletes a scheduler entry by id
- **THEN** subsequent scheduler list responses do not include that entry

### Requirement: Root menu scheduler display
The `/menu` page SHALL display all scheduler entries.

#### Scenario: Open root menu
- **WHEN** the browser renders `/menu`
- **THEN** the page shows a scheduler section populated from the scheduler list API

#### Scenario: Empty scheduler
- **WHEN** the scheduler list API returns no entries
- **THEN** the root menu page shows an empty scheduler state without hiding the sports list
```

