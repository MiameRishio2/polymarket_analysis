---
change: menu-event-status-parent-navigation
design-doc: docs/superpowers/specs/2026-06-13-menu-event-status-parent-navigation-design.md
base-ref: 13be0b09b0f639fc0a409533eac06959b90f6742
---

# Menu Event Status and Parent Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit ended/open marker to fourth-level menu event rows and verify nested menu parent navigation.

**Architecture:** The backend owns event status as `EventRow.ended`, derived from the existing `start_time` string during event row construction and serialized through the existing event API/cache paths. The frontend reads that boolean and adds a status column to the event table while preserving external links and scheduler actions. Parent navigation remains path-derived in `public/menu.html`.

**Tech Stack:** Rust 2021, Axum, serde, chrono, scraper crate, vanilla JavaScript, Node `vm` tests.

---

## File Structure

- Modify `src/menu/events.rs`: add `EventRow.ended`, date parsing/status helpers, and populate the field in `push_event_row`.
- Modify `tests/menu_third_level_test.rs`: update `EventRow` literals, add backend status derivation and serde compatibility tests, and assert API/cache responses include `ended`.
- Modify `public/menu.html`: add status badge styles, add event status rendering helper, and expand event table grid from three to four columns.
- Modify `tests/menu_page_config_test.js`: assert event status rendering, action preservation, and root/nested parent navigation rendering.
- Modify `openspec/changes/menu-event-status-parent-navigation/tasks.md`: check off tasks as implementation and verification complete.

## Task 1: Backend Event Status Contract

**Files:**
- Modify: `src/menu/events.rs`
- Modify: `tests/menu_third_level_test.rs`

- [ ] **Step 1: Write failing Rust status tests**

Add these imports near the existing `EventRow` imports in `tests/menu_third_level_test.rs`:

```rust
use chrono::{TimeZone, Utc};
```

Add these tests near the existing event row serialization tests:

```rust
#[test]
fn test_event_has_ended_for_past_start_time() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 0, 0, 0).unwrap();

    assert!(polymarket_analysis::menu::events::event_has_ended_at(
        "11 Jun 2026, 21:00",
        now
    ));
}

#[test]
fn test_event_has_not_ended_for_future_start_time() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 0, 0, 0).unwrap();

    assert!(!polymarket_analysis::menu::events::event_has_ended_at(
        "18 Jun 2026, 03:00",
        now
    ));
}

#[test]
fn test_event_has_not_ended_for_unknown_start_time() {
    let now = Utc.with_ymd_and_hms(2026, 6, 13, 0, 0, 0).unwrap();

    assert!(!polymarket_analysis::menu::events::event_has_ended_at("", now));
    assert!(!polymarket_analysis::menu::events::event_has_ended_at(
        "not a date",
        now
    ));
}

#[test]
fn test_event_row_defaults_missing_ended_to_false() {
    let json = serde_json::json!({
        "slug": "mexico-vs-south-africa",
        "home_team": "Mexico",
        "away_team": "South Africa",
        "matchup": "Mexico VS South Africa",
        "start_time": "18 Jun 2026, 03:00",
        "url": "https://www.oddsportal.com/football/h2h/mexico/south-africa/"
    });

    let event: EventRow = serde_json::from_value(json).expect("old event JSON should load");
    assert!(!event.ended);
}
```

- [ ] **Step 2: Run focused tests to verify failure**

Run:

```bash
cargo test --test menu_third_level_test test_event_has_ended_for_past_start_time test_event_has_not_ended_for_future_start_time test_event_has_not_ended_for_unknown_start_time test_event_row_defaults_missing_ended_to_false
```

Expected: compile failure because `event_has_ended_at` and `EventRow.ended` do not exist.

- [ ] **Step 3: Add `ended` field and status helpers**

In `src/menu/events.rs`, update the `EventRow` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    pub url: String,
    #[serde(default)]
    pub ended: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_url: Option<String>,
}
```

Add these helpers near the existing event date helpers:

```rust
pub fn event_has_ended_at(start_time: &str, now: DateTime<chrono::Utc>) -> bool {
    let Some(event_time) = parse_event_start_time(start_time) else {
        return false;
    };
    event_time < now
}

fn event_has_ended_now(start_time: &str) -> bool {
    event_has_ended_at(start_time, chrono::Utc::now())
}

fn parse_event_start_time(start_time: &str) -> Option<DateTime<chrono::Utc>> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(start_time) {
        return Some(datetime.with_timezone(&chrono::Utc));
    }

    let naive = chrono::NaiveDateTime::parse_from_str(start_time.trim(), "%d %b %Y, %H:%M")
        .ok()?;
    Some(naive.and_utc())
}
```

Update `push_event_row` so new rows set `ended`:

```rust
let ended = event_has_ended_now(&start_time);
events.push(EventRow {
    slug,
    home_team,
    away_team,
    matchup,
    ended,
    start_time,
    url,
    polymarket_url: None,
});
```

- [ ] **Step 4: Update existing Rust `EventRow` literals**

In `tests/menu_third_level_test.rs`, add `ended: false,` to every manually constructed future/open event literal, and use `ended: true,` only for any test literal intentionally representing a past event. Keep the field next to `start_time` for readability:

```rust
start_time: "13 Jun 2026, 21:00".to_string(),
ended: false,
url: "https://www.oddsportal.com/football/h2h/qatar/switzerland/".to_string(),
```

- [ ] **Step 5: Run focused tests to verify pass**

Run:

```bash
cargo test --test menu_third_level_test test_event_has_ended_for_past_start_time test_event_has_not_ended_for_future_start_time test_event_has_not_ended_for_unknown_start_time test_event_row_defaults_missing_ended_to_false
```

Expected: all four tests pass.

- [ ] **Step 6: Commit backend contract task**

Run:

```bash
git add src/menu/events.rs tests/menu_third_level_test.rs
git commit -m "feat: add event ended status"
```

## Task 2: Backend API and Cache Coverage

**Files:**
- Modify: `tests/menu_third_level_test.rs`
- Modify: `src/menu/events.rs` if tests reveal missing propagation

- [ ] **Step 1: Add API/cache assertions**

In `test_extracts_world_championship_event_row`, assert the representative 18 Jun 2026 event is not ended:

```rust
assert!(!events[0].ended);
```

In `test_event_api_reads_event_cache_without_overwriting_category_cache`, add `ended: false,` to the cached `EventRow`, then assert the API response includes it:

```rust
assert_eq!(json["data"]["events"][0]["ended"], false);
```

In `test_event_row_omits_missing_polymarket_url`, keep the existing missing Polymarket assertion and add:

```rust
assert_eq!(json["ended"], false);
```

- [ ] **Step 2: Run backend integration tests**

Run:

```bash
cargo test --test menu_third_level_test
```

Expected: all tests in `menu_third_level_test` pass. If failures show missing `ended` fields in literals, add the field explicitly to those test rows and rerun.

- [ ] **Step 3: Commit backend propagation task**

Run:

```bash
git add src/menu/events.rs tests/menu_third_level_test.rs
git commit -m "test: cover event ended propagation"
```

## Task 3: Frontend Event Status Display and Parent Navigation Tests

**Files:**
- Modify: `tests/menu_page_config_test.js`
- Modify: `public/menu.html`

- [ ] **Step 1: Add failing frontend status tests**

In `tests/menu_page_config_test.js`, add this test near `testRenderEventRows`:

```javascript
function testRenderEventStatusBadges() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      ended: true,
      url: 'https://www.oddsportal.com/football/h2h/mexico/south-africa/',
      polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11'
    }, {
      matchup: 'Qatar VS Switzerland',
      start_time: '13 Jun 2026, 21:00',
      ended: false,
      url: 'https://www.oddsportal.com/football/h2h/qatar/switzerland/'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /状态/);
  assert.match(context.__elements.content.innerHTML, /已结束/);
  assert.match(context.__elements.content.innerHTML, /未结束/);
  assert.match(context.__elements.content.innerHTML, /OddsPortal/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
  assert.match(context.__elements.content.innerHTML, /加入监控/);
}
```

Add this test near `testRenderParentNavigation`:

```javascript
function testRenderRootHasNoParentNavigation() {
  const context = loadMenuScript('/menu');
  context.renderParentNavigation();

  assert.strictEqual(context.__elements['top-nav'].innerHTML, '');
}
```

Call both tests in the synchronous test list:

```javascript
testRenderEventStatusBadges();
testRenderRootHasNoParentNavigation();
```

- [ ] **Step 2: Run frontend tests to verify failure**

Run:

```bash
node tests/menu_page_config_test.js
```

Expected: failure because the event table does not yet render a status column or labels.

- [ ] **Step 3: Implement frontend status rendering**

In `public/menu.html`, change the event grid columns:

```css
.list-header.event-grid,
.list-row.event-grid { grid-template-columns: 1.4fr 1fr 0.75fr 2fr; }
```

Add badge styles near the existing `.scheduler-state` styles:

```css
.event-status {
  display: inline-flex;
  width: fit-content;
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
}
.event-status.open {
  background: #dcfce7;
  color: #166534;
}
.event-status.ended {
  background: #fee2e2;
  color: #991b1b;
}
```

Add a renderer near `renderEventLinks`:

```javascript
function renderEventStatus(item) {
  const ended = item && item.ended === true;
  return `<span class="event-status ${ended ? 'ended' : 'open'}">${ended ? '已结束' : '未结束'}</span>`;
}
```

Update the event table header and row:

```javascript
<div class="list-header event-grid">
  <span>比赛</span>
  <span>开始时间</span>
  <span>状态</span>
  <span>链接</span>
</div>
```

```javascript
<div class="list-row event-grid">
  <span class="name">${item.matchup || ''}</span>
  <span class="time">${item.start_time || ''}</span>
  ${renderEventStatus(item)}
  ${renderEventLinks(item, start + index)}
</div>
```

- [ ] **Step 4: Run frontend tests to verify pass**

Run:

```bash
node tests/menu_page_config_test.js
```

Expected: `menu_page_config_test passed`.

- [ ] **Step 5: Commit frontend display task**

Run:

```bash
git add public/menu.html tests/menu_page_config_test.js
git commit -m "feat: show event ended status"
```

## Task 4: OpenSpec Task Tracking and Final Verification

**Files:**
- Modify: `openspec/changes/menu-event-status-parent-navigation/tasks.md`

- [ ] **Step 1: Check off completed OpenSpec tasks**

Update `openspec/changes/menu-event-status-parent-navigation/tasks.md` so all implemented tasks are checked:

```markdown
- [x] 1. Add `ended` to `EventRow` with serde compatibility for older cached event JSON.
- [x] 2. Implement server-side ended-status derivation from existing event `start_time` values.
- [x] 3. Update event refresh stream/progress completion payloads to carry the new event row field.
- [x] 4. Render event ended/open markers in the fourth-level menu event table.
- [x] 5. Verify and tighten parent navigation behavior for `/menu`, sport, third-level, and fourth-level pages.
- [x] 6. Add focused Rust and frontend tests for event status and parent navigation.
- [x] 7. Run project verification commands.
```

- [ ] **Step 2: Run final frontend verification**

Run:

```bash
node tests/menu_page_config_test.js
```

Expected: `menu_page_config_test passed`.

- [ ] **Step 3: Run focused Rust verification**

Run:

```bash
cargo test --test menu_third_level_test
```

Expected: all tests in the test target pass.

- [ ] **Step 4: Run full Rust verification**

Run:

```bash
cargo test --all
```

Expected: all Rust tests pass. If sandboxed execution fails because existing wiremock/TcpListener tests cannot bind local ports, rerun this same command with escalation and record that the sandbox restriction was the cause.

- [ ] **Step 5: Commit tracking and verification task**

Run:

```bash
git add openspec/changes/menu-event-status-parent-navigation/tasks.md
git commit -m "chore: complete menu event status tasks"
```

## Self-Review

Spec coverage:

- Competition event ended marker: Task 1 adds the backend field and fixed-clock derivation tests; Task 2 asserts API/cache propagation.
- Competition event status display: Task 3 adds frontend status rendering and tests both labels while preserving links/actions.
- Parent navigation for nested menu pages: Task 3 adds root no-link coverage and keeps existing nested parent link tests.
- Verification: Task 4 runs frontend, focused Rust, and full Rust verification.

Placeholder scan: no `TBD`, `TODO`, "implement later", or unresolved file paths remain.

Type consistency: the plan uses `EventRow.ended`, `event_has_ended_at`, `renderEventStatus`, and existing `getParentMenuHref` consistently across backend, frontend, and tests.
