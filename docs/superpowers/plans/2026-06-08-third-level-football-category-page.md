---
change: third-level-football-category-page
design-doc: docs/superpowers/specs/2026-06-08-third-level-football-category-page-design.md
base-ref: 756dca528c5f82ad26236175081035602da1e983
---

# Third-Level Football Category Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a third-level local menu page for football country children, starting with `/menu/football/argentina` and OddsPortal child URL `/football/argentina/primera-nacional/`.

**Architecture:** Keep the static frontend in `public/menu.html` and all fetching, parsing, API, and cache behavior in `src/menu`. Reuse `CategoryData` and SQLite cache storage with distinct third-level keys.

**Tech Stack:** Rust, Axum, rusqlite, scraper crate, static HTML/JavaScript, Node for frontend route tests.

---

## File Structure

- Modify `src/menu/scraper.rs`: add testable third-level extraction and fetch helpers.
- Modify `src/menu/handlers.rs`: add third-level API/page routes and cache key helpers.
- Modify `src/menu/mod.rs`: export new scraper helpers used by tests if needed.
- Modify `public/menu.html`: parse third-level paths and convert second-level rows to local child pages.
- Add `tests/menu_third_level_test.rs`: scraper and handler tests.
- Add `tests/menu_page_config_test.js`: frontend routing behavior test.
- Modify `web_design.md`: document third-level page and API contract.
- Modify `test.md`: record new tests.
- Modify `session.md` and `change_log.md`: record task completion.

## Task 1: Scraper Third-Level Extraction

**Files:**
- Modify: `src/menu/scraper.rs`
- Test: `tests/menu_third_level_test.rs`

- [ ] **Step 1: Write the failing scraper test**

Create `tests/menu_third_level_test.rs` with a test that calls `extract_categories_for_path` on HTML containing `/football/argentina/primera-nacional/`, unrelated paths, and deeper paths.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `cargo test --test menu_third_level_test test_extracts_third_level_child_categories -- --nocapture`

Expected: compile failure or test failure because `extract_categories_for_path` does not exist.

- [ ] **Step 3: Implement minimal scraper helpers**

Add `extract_child_category_slug`, `extract_categories_for_path`, and `fetch_categories_for_path` to `src/menu/scraper.rs`. Keep the existing second-level behavior unchanged.

- [ ] **Step 4: Run the focused test to verify GREEN**

Run: `cargo test --test menu_third_level_test test_extracts_third_level_child_categories -- --nocapture`

Expected: PASS.

## Task 2: Third-Level API and Cache Key

**Files:**
- Modify: `src/menu/handlers.rs`
- Test: `tests/menu_third_level_test.rs`

- [ ] **Step 1: Write the failing handler test**

Add a test that seeds `Storage` with key `menu_football_argentina`, calls the router at `/api/menu/football/argentina`, and asserts that the cached `primera-nacional` row is returned.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `cargo test --test menu_third_level_test test_third_level_api_reads_distinct_cache_key -- --nocapture`

Expected: FAIL because the route is missing.

- [ ] **Step 3: Implement handler route and cache key helper**

Add `category_child_handler`, `category_child_refresh_handler`, `third_level_cache_key`, and the `/api/menu/:sport/:category` plus `/api/menu/:sport/:category/refresh` routes before the less-specific one-segment routes.

- [ ] **Step 4: Run the focused test to verify GREEN**

Run: `cargo test --test menu_third_level_test test_third_level_api_reads_distinct_cache_key -- --nocapture`

Expected: PASS.

## Task 3: Frontend Third-Level Routing

**Files:**
- Modify: `public/menu.html`
- Test: `tests/menu_page_config_test.js`

- [ ] **Step 1: Write the failing frontend routing test**

Create a Node test that extracts the `public/menu.html` script, evaluates it with a mocked `window.location.pathname`, and asserts `/menu/football/argentina` maps to `/api/menu/football/argentina`.

- [ ] **Step 2: Run the frontend test to verify RED**

Run: `node tests/menu_page_config_test.js`

Expected: FAIL because the current config treats `football/argentina` as one category string.

- [ ] **Step 3: Implement frontend routing**

Update `getPageConfig()`, `getSportFromPath()`, title handling, and category row link generation so second-level rows link to local third-level pages and third-level rows link to OddsPortal.

- [ ] **Step 4: Run the frontend test to verify GREEN**

Run: `node tests/menu_page_config_test.js`

Expected: PASS.

## Task 4: Documentation and Full Verification

**Files:**
- Modify: `web_design.md`
- Modify: `test.md`
- Modify: `session.md`
- Modify: `change_log.md`
- Modify: `openspec/changes/third-level-football-category-page/tasks.md`

- [ ] **Step 1: Update project docs**

Add the third-level page/API/scraping rules to `web_design.md`, add new tests to `test.md`, and update task state docs.

- [ ] **Step 2: Run all verification**

Run: `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.

Expected: all pass without new warnings.

- [ ] **Step 3: Mark OpenSpec tasks complete**

Change every checkbox in `openspec/changes/third-level-football-category-page/tasks.md` to checked after verification passes.
