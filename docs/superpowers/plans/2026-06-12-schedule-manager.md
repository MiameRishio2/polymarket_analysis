---
change: schedule-manager
design-doc: docs/superpowers/specs/2026-06-12-schedule-manager-design.md
base-ref: aad4b8d32ba1f8910a3a01570990e1794f1ac4e5
---

# Schedule Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a SQLite-backed scheduler manager for match monitoring candidates and expose it in the menu UI.

**Architecture:** Add `src/menu/scheduler.rs` for models, SQLite methods, and handlers that reuse the existing `Storage` connection. Extend `public/menu.html` so fourth-level event rows can schedule matches and `/menu` displays all scheduled matches.

**Tech Stack:** Rust, Axum, rusqlite, serde, static HTML/JavaScript, Node VM tests.

archived-with: 2026-06-12-schedule-manager
---

## File Structure

- Create `src/menu/scheduler.rs`: scheduler models, deterministic ids, SQLite storage methods, and Axum handlers.
- Modify `src/menu/storage.rs`: initialize `scheduled_matches` table during `Storage::new`.
- Modify `src/menu/mod.rs`: expose the scheduler module and public scheduler types used by tests.
- Modify `src/menu/handlers.rs`: mount scheduler routes into the existing router.
- Modify `public/menu.html`: load scheduler state, render action buttons, render root scheduler section, call scheduler APIs.
- Modify `tests/storage_test.rs`: add SQLite scheduler behavior tests.
- Modify `tests/menu_page_config_test.js`: add frontend scheduler rendering and action tests.
- Modify `openspec/changes/schedule-manager/tasks.md`: check off completed tasks as implementation proceeds.

## Task 1: Scheduler Storage

**Files:**
- Create: `src/menu/scheduler.rs`
- Modify: `src/menu/storage.rs`
- Modify: `src/menu/mod.rs`
- Test: `tests/storage_test.rs`

- [ ] **Step 1: Write failing storage tests**

Add tests that create a temp database, call `Storage::new`, then exercise:

```rust
let input = NewScheduledMatch {
    matchup: "Mexico VS South Africa".to_string(),
    home_team: Some("Mexico".to_string()),
    away_team: Some("South Africa".to_string()),
    start_time: Some("2026-06-18T03:00:00Z".to_string()),
    oddsportal_url: Some("https://www.oddsportal.com/football/h2h/mexico/south-africa/".to_string()),
    polymarket_url: Some("https://polymarket.com/event".to_string()),
    source_page: Some("/menu/football/world/world-championship-2026/".to_string()),
    monitoring_started: None,
};
let saved = storage.upsert_scheduled_match(&input).expect("upsert should work");
assert!(!saved.monitoring_started);
assert_eq!(storage.list_scheduled_matches().unwrap().len(), 1);
```

Include duplicate upsert, monitor toggle, delete, and ordering assertions.

- [ ] **Step 2: Run the storage test to verify it fails**

Run: `cargo test test_scheduler --test storage_test`

Expected: fail because scheduler types and methods do not exist.

- [ ] **Step 3: Implement storage**

Create `src/menu/scheduler.rs` with:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ScheduledMatch { /* id, matchup, optional metadata, monitoring_started, timestamps */ }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NewScheduledMatch { /* same mutable input fields, monitoring_started: Option<bool> */ }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateMonitoringRequest {
    pub monitoring_started: bool,
}
```

Implement `Storage::init_scheduler_schema`, `upsert_scheduled_match`, `list_scheduled_matches`, `set_scheduled_match_monitoring`, and `delete_scheduled_match`.

- [ ] **Step 4: Run storage tests**

Run: `cargo test test_scheduler --test storage_test`

Expected: pass.

## Task 2: Scheduler API

**Files:**
- Modify: `src/menu/scheduler.rs`
- Modify: `src/menu/handlers.rs`
- Test: compile coverage through `cargo test`

- [ ] **Step 1: Add handlers**

In `src/menu/scheduler.rs`, add:

```rust
pub(crate) async fn scheduler_list_handler(State(state): State<AppState>) -> Json<ApiResponse<Vec<ScheduledMatch>>>;
pub(crate) async fn scheduler_upsert_handler(State(state): State<AppState>, Json(payload): Json<NewScheduledMatch>) -> Json<ApiResponse<ScheduledMatch>>;
pub(crate) async fn scheduler_monitoring_handler(Path(id): Path<String>, State(state): State<AppState>, Json(payload): Json<UpdateMonitoringRequest>) -> Json<ApiResponse<ScheduledMatch>>;
pub(crate) async fn scheduler_delete_handler(Path(id): Path<String>, State(state): State<AppState>) -> Json<ApiResponse<bool>>;
```

Each handler should return `ok: false` with an error string on storage errors.

- [ ] **Step 2: Mount routes**

In `src/menu/handlers.rs`, import scheduler handlers and add routes:

```rust
.route("/api/scheduler", get(scheduler_list_handler).post(scheduler_upsert_handler))
.route("/api/scheduler/:id/monitoring", post(scheduler_monitoring_handler))
.route("/api/scheduler/:id", axum::routing::delete(scheduler_delete_handler))
```

- [ ] **Step 3: Run compile tests**

Run: `cargo test --lib`

Expected: pass.

## Task 3: Menu Frontend Scheduler UI

**Files:**
- Modify: `public/menu.html`
- Test: `tests/menu_page_config_test.js`

- [ ] **Step 1: Write failing frontend tests**

Add tests that verify:

```js
assert.match(context.renderEventLinks(eventItem), /加入监控/);
assert.match(context.renderSchedulerSection(), /scheduler-section/);
```

Also test that a fourth-level event row posts to `/api/scheduler` with event metadata.

- [ ] **Step 2: Run frontend test to verify it fails**

Run: `node tests/menu_page_config_test.js`

Expected: fail because scheduler UI functions do not exist.

- [ ] **Step 3: Implement frontend state and rendering**

Add:

```js
let schedulerItems = [];
let schedulerById = new Map();
```

Add `fetchScheduler()`, `scheduleEvent(item)`, `toggleScheduledMonitoring(id, value)`, `deleteScheduledMatch(id)`, `schedulerIdFor(item)`, `renderSchedulerSection()`, and `renderSchedulerAction(item)`.

Update `fetchData()` to load scheduler state before rendering, update event links to include scheduler action, and update root `/menu` rendering to show scheduler entries above the sports list.

- [ ] **Step 4: Run frontend tests**

Run: `node tests/menu_page_config_test.js`

Expected: pass.

## Task 4: Final Verification and Task Sync

**Files:**
- Modify: `openspec/changes/schedule-manager/tasks.md`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt`

Expected: no formatting errors.

- [ ] **Step 2: Run relevant tests**

Run:

```bash
cargo test test_scheduler --test storage_test
cargo test --lib
node tests/menu_page_config_test.js
```

Expected: all pass.

- [ ] **Step 3: Check off OpenSpec tasks**

Mark all completed items in `openspec/changes/schedule-manager/tasks.md` with `[x]`.

- [ ] **Step 4: Run build guard**

Run:

```bash
bash -lc 'COMET_ENV="${COMET_ENV:-$(find . "$HOME"/.*/skills "$HOME/.config" "$HOME/.gemini" -path "*/comet/scripts/comet-env.sh" -type f -print -quit 2>/dev/null)}"; . "$COMET_ENV"; "$COMET_BASH" "$COMET_GUARD" schedule-manager build --apply'
```

Expected: guard passes and moves the change to verify.

## Self-Review

- Spec coverage: scheduler storage, metadata, monitor toggle, API, root display, fourth-level row action, and external link preservation all map to tasks.
- Placeholder scan: no TBD/TODO placeholders remain.
- Type consistency: scheduler models use `ScheduledMatch`, `NewScheduledMatch`, and `UpdateMonitoringRequest` consistently across storage, handlers, and tests.
