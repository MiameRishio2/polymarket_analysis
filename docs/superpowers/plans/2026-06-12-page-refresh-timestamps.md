---
change: page-refresh-timestamps
design-doc: docs/superpowers/specs/2026-06-12-page-refresh-timestamps-design.md
base-ref: 03966e8b5993634f43782a98bf9a5dff60fb82a6
archived-with: 2026-06-12-page-refresh-timestamps
---

# Page Refresh Timestamps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist and display page-level refresh timestamps for every menu page depth.

**Architecture:** Add `refreshed_at` to the shared SQLite cache row and expose it through existing category and event response structs. The shared menu page reads the field from whichever API response is currently displayed.

**Tech Stack:** Rust, Axum, rusqlite, serde, chrono, vanilla JavaScript tests.

archived-with: 2026-06-12-page-refresh-timestamps
---

### Task 1: Storage Schema And Models

**Files:**
- Modify: `src/menu/storage.rs`
- Modify: `src/menu/models.rs`
- Modify: `src/menu/events.rs`
- Test: `tests/storage_test.rs`
- Test: `tests/menu_third_level_test.rs`

- [ ] **Step 1: Add failing storage coverage**

Add tests for migrating a legacy `category_cache` table without `refreshed_at`, loading default epoch refresh time, and raw JSON refresh timestamp round-trips.

- [ ] **Step 2: Run focused storage tests**

Run: `cargo test --test storage_test`

Expected before implementation: failure mentioning missing `refreshed_at` behavior or struct fields.

- [ ] **Step 3: Implement storage migration and model fields**

Add `refreshed_at` to table creation, migration, `CategoryData`, `EventData`, `save`, `save_raw_json`, `load`, and `load_raw_json`.

- [ ] **Step 4: Run focused Rust tests**

Run: `cargo test --test storage_test --test menu_third_level_test`

Expected after implementation: tests pass.

### Task 2: API Handler Timestamp Propagation

**Files:**
- Modify: `src/menu/handlers.rs`
- Modify: `src/menu/events.rs`
- Test: `tests/menu_third_level_test.rs`

- [ ] **Step 1: Add API assertions**

Update nested menu/event API tests to assert `data.refreshed_at` is present and uses cached values when data is loaded from SQLite.

- [ ] **Step 2: Run focused API tests**

Run: `cargo test --test menu_third_level_test`

Expected before implementation: failure on missing or incorrect `refreshed_at`.

- [ ] **Step 3: Populate timestamps in all fresh handler responses**

Set `refreshed_at` when constructing `CategoryData` and `EventData` in fetch, fallback, and refresh paths.

- [ ] **Step 4: Re-run focused API tests**

Run: `cargo test --test menu_third_level_test`

Expected after implementation: tests pass.

### Task 3: Menu UI Display

**Files:**
- Modify: `public/menu.html`
- Test: `tests/menu_page_config_test.js`

- [ ] **Step 1: Add JavaScript rendering tests**

Add tests that simulate category and event API responses with `refreshed_at` and assert `stats-bar` contains `刷新时间`.

- [ ] **Step 2: Run JavaScript tests**

Run: `node tests/menu_page_config_test.js`

Expected before implementation: failure because refresh time is not rendered.

- [ ] **Step 3: Implement UI state and badge**

Add `refreshedAt`, default it to `1970-01-01T00:00:00Z`, update it from API responses, and render it in `renderStats()`.

- [ ] **Step 4: Re-run JavaScript tests**

Run: `node tests/menu_page_config_test.js`

Expected after implementation: tests pass.

### Task 4: Final Verification And Comet Task Sync

**Files:**
- Modify: `openspec/changes/page-refresh-timestamps/tasks.md`

- [ ] **Step 1: Run targeted verification**

Run: `cargo test --test storage_test --test menu_third_level_test`

Run: `node tests/menu_page_config_test.js`

- [ ] **Step 2: Mark OpenSpec tasks complete**

Check off completed tasks in `openspec/changes/page-refresh-timestamps/tasks.md`.

- [ ] **Step 3: Review changed files**

Run: `git diff --stat` and inspect only files touched for this change, preserving pre-existing unrelated diffs.
