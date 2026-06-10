---
change: event-list-parse
design-doc: docs/superpowers/specs/2026-06-09-event-list-parse-design.md
base-ref: 4938678df1807fd16d266ceabe941e9d867e8989
---

# Event List Parse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse and display OddsPortal World Championship 2026 event rows with matchup, start time, and H2H link.

**Architecture:** Add a focused `src/menu/events.rs` module with event models, parser, cache helpers, and handlers. Keep existing category APIs untouched; fourth-level frontend pages call `/api/events/...` first and fall back to `/api/menu/...` when no events are available.

**Tech Stack:** Rust, Axum, serde, scraper crate, rusqlite-backed existing `Storage`, static HTML/JavaScript, Node `vm` frontend tests.

---

## File Structure

- Create `src/menu/events.rs`: event row/data models, parser, fetcher, cache key, API handlers, refresh handlers, router helper.
- Modify `src/menu/mod.rs`: expose the new `events` module.
- Modify `src/menu/handlers.rs`: merge event routes into `create_router` with minimal route wiring.
- Modify `src/lib.rs`: export event models only if tests need public access.
- Modify `public/menu.html`: fourth-level event-first fetch, event rendering mode, event table grid.
- Modify `tests/menu_third_level_test.rs`: backend event parser and API/cache tests.
- Modify `tests/menu_page_config_test.js`: frontend event API/rendering/pagination tests.
- Modify `web_design.md`, `architect.md`, `test.md`, `session.md`, `change_log.md`: required project documentation updates.

## Task 1: Backend Event Tests

**Files:**
- Modify: `tests/menu_third_level_test.rs`
- Create later: `src/menu/events.rs`

- [ ] **Step 1: Add failing parser test**

Add imports at the top:

```rust
use polymarket_analysis::menu::events::extract_events_for_competition;
```

Add this test:

```rust
#[test]
fn test_extracts_world_championship_event_row() {
    let html = r##"
        <html>
          <body>
            <div data-testid="game-row">
              <span class="time">18 Jun 2026, 03:00</span>
              <a href="/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2">
                <span>Mexico</span>
                <span>South Africa</span>
              </a>
            </div>
            <a href="/football/world/world-championship-2026/results/">Results</a>
            <a href="/football/england/premier-league/">Premier League</a>
          </body>
        </html>
    "##;

    let events = extract_events_for_competition(html, "football", "world", "world-championship-2026")
        .expect("event extraction should parse representative HTML");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].home_team, "Mexico");
    assert_eq!(events[0].away_team, "South Africa");
    assert_eq!(events[0].matchup, "Mexico VS South Africa");
    assert_eq!(events[0].start_time, "18 Jun 2026, 03:00");
    assert_eq!(
        events[0].url,
        "/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2"
    );
}
```

- [ ] **Step 2: Add failing API/cache test**

Add imports:

```rust
use polymarket_analysis::menu::events::{EventData, EventRow};
```

Add this async test:

```rust
#[tokio::test]
async fn test_event_api_reads_event_cache_without_overwriting_category_cache() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let db_path = temp_dir.path().join("menu.db");
    let storage = Storage::new(&db_path).expect("storage should be created");

    let category_cached = CategoryData {
        sport: "football/world/world-championship-2026".to_string(),
        categories: vec![Category {
            slug: "winner".to_string(),
            name: "Winner".to_string(),
            url: "/football/world/world-championship-2026/winner/".to_string(),
            category_type: Some("league".to_string()),
        }],
        last_updated: "2026-06-09T00:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    storage
        .save("menu_football_world_world-championship-2026", &category_cached)
        .expect("category cache seed should save");

    let event_cached = EventData {
        sport: "football/world/world-championship-2026".to_string(),
        events: vec![EventRow {
            slug: "mexico-vs-south-africa".to_string(),
            home_team: "Mexico".to_string(),
            away_team: "South Africa".to_string(),
            matchup: "Mexico VS South Africa".to_string(),
            start_time: "18 Jun 2026, 03:00".to_string(),
            url: "/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
        }],
        last_updated: "2026-06-09T00:00:00Z".to_string(),
        source: "cache-test".to_string(),
    };
    polymarket_analysis::menu::events::save_event_data(
        &storage,
        "events_football_world_world-championship-2026",
        &event_cached,
    )
    .expect("event cache seed should save");

    let app = create_router(storage);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events/football/world/world-championship-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("response should be JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["sport"], "football/world/world-championship-2026");
    assert_eq!(json["data"]["events"][0]["matchup"], "Mexico VS South Africa");
    assert_eq!(json["data"]["events"][0]["start_time"], "18 Jun 2026, 03:00");
}
```

- [ ] **Step 3: Run backend tests and verify red**

Run:

```bash
cargo test test_extracts_world_championship_event_row test_event_api_reads_event_cache_without_overwriting_category_cache
```

Expected: FAIL because `polymarket_analysis::menu::events` and event types/functions do not exist yet.

- [ ] **Step 4: Commit red tests only if project policy allows red-test commits**

Default for this repo: do not commit failing tests separately unless the user asks. Keep them uncommitted and proceed to implementation.

## Task 2: Backend Event Module

**Files:**
- Create: `src/menu/events.rs`
- Modify: `src/menu/mod.rs`
- Modify: `src/menu/handlers.rs`
- Modify: `src/lib.rs` if public exports are needed by tests

- [ ] **Step 1: Create `src/menu/events.rs` with event models and cache helpers**

Implement these public types and helpers:

```rust
use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::handlers::{ApiResponse, AppState};
use super::scraper::{fetch_url, ScraperError};
use super::storage::{Storage, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub sport: String,
    pub events: Vec<EventRow>,
    pub last_updated: String,
    pub source: String,
}

pub fn event_cache_key(sport: &str, category: &str, league: &str) -> String {
    format!("events_{}_{}_{}", sport, category, league)
}

fn event_sport_key(sport: &str, category: &str, league: &str) -> String {
    format!("{}/{}/{}", sport, category, league)
}
```

Add `save_event_data` and `load_event_data` using the existing `category_cache` table with event JSON in `categories_json`:

```rust
pub fn save_event_data(storage: &Storage, key: &str, data: &EventData) -> Result<(), StorageError> {
    storage.save_raw_json(key, &serde_json::to_string(&data.events)?, &data.last_updated, &data.source)
}

pub fn load_event_data(storage: &Storage, key: &str, sport_key: String) -> Result<Option<EventData>, StorageError> {
    storage.load_raw_json(key).and_then(|raw| {
        raw.map(|(events_json, last_updated, source)| {
            serde_json::from_str::<Vec<EventRow>>(&events_json).map(|events| EventData {
                sport: sport_key,
                events,
                last_updated,
                source,
            })
        })
        .transpose()
        .map_err(StorageError::Json)
    })
}
```

This requires adding `Storage::save_raw_json` and `Storage::load_raw_json` in Task 2 Step 2.

- [ ] **Step 2: Add raw JSON storage methods**

Modify `src/menu/storage.rs` with focused public methods:

```rust
pub fn save_raw_json(
    &self,
    sport: &str,
    json: &str,
    last_updated: &str,
    source: &str,
) -> Result<(), StorageError> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO category_cache 
         (sport, categories_json, last_updated, source, updated_at) 
         VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![sport, json, last_updated, source],
    )?;
    Ok(())
}

pub fn load_raw_json(&self, sport: &str) -> Result<Option<(String, String, String)>, StorageError> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT categories_json, last_updated, source FROM category_cache WHERE sport = ?1"
    )?;
    let result = stmt.query_row(params![sport], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    });
    match result {
        Ok(raw) => Ok(Some(raw)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StorageError::Sqlite(e)),
    }
}
```

- [ ] **Step 3: Implement event parser**

In `src/menu/events.rs`, implement:

```rust
pub fn extract_events_for_competition(
    html: &str,
    sport: &str,
    _category: &str,
    _league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let document = scraper::Html::parse_document(html);
    let link_selector = scraper::Selector::parse("a[href]")
        .map_err(|e| ScraperError::Parse(format!("Invalid selector: {e}")))?;
    let mut events = Vec::new();

    for element in document.select(&link_selector) {
        let Some(href) = element.value().attr("href") else { continue; };
        if !href.starts_with(&format!("/{sport}/h2h/")) {
            continue;
        }
        let text = element.text().collect::<Vec<_>>().join(" ");
        let (home_team, away_team) = teams_from_link_or_text(href, &text)?;
        let start_time = nearest_time_text(&element).unwrap_or_default();
        let slug = format!("{}-vs-{}", slugify(&home_team), slugify(&away_team));
        let matchup = format!("{} VS {}", home_team, away_team);
        if events.iter().any(|event: &EventRow| event.url == href) {
            continue;
        }
        events.push(EventRow {
            slug,
            home_team,
            away_team,
            matchup,
            start_time,
            url: href.to_string(),
        });
    }

    Ok(events)
}
```

Implement helpers used above:

```rust
fn teams_from_link_or_text(href: &str, text: &str) -> Result<(String, String), ScraperError> { /* parse visible text first, fallback to h2h path */ }
fn nearest_time_text(element: &scraper::ElementRef<'_>) -> Option<String> { /* parent text token with a date/time pattern */ }
fn decode_team_slug(segment: &str) -> String { /* mexico-O6iHcNkd -> Mexico */ }
fn slugify(value: &str) -> String { /* lower, spaces to hyphens */ }
```

The concrete implementation must satisfy the tests; keep helpers private unless tests need them.

- [ ] **Step 4: Implement event fetching and handlers**

In `src/menu/events.rs`, add:

```rust
pub async fn fetch_events_for_competition(
    sport: &str,
    category: &str,
    league: &str,
) -> Result<Vec<EventRow>, ScraperError> {
    let url = format!("https://www.oddsportal.com/{}/{}/{}/", sport, category, league);
    let html = fetch_url(&url).await?;
    extract_events_for_competition(&html, sport, category, league)
}

fn ok_response<T: serde::Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse { ok: true, data: Some(data), error: None })
}

pub async fn event_list_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<EventData>> { /* load events_ cache, else fetch, save if source != error */ }

pub async fn event_list_refresh_handler(
    Path((sport, category, league)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Json<ApiResponse<EventData>> { /* force fetch and save non-error events */ }

pub fn event_routes() -> Router<Arc<Storage>> { /* if AppState route merge is awkward, skip helper and wire in handlers.rs */ }
```

- [ ] **Step 5: Wire module and routes**

Modify `src/menu/mod.rs`:

```rust
pub mod events;
```

Modify `src/menu/handlers.rs` imports and router:

```rust
use super::events::{event_list_handler, event_list_refresh_handler};
```

Add routes before `/api/menu/:sport...` routes:

```rust
.route(
    "/api/events/:sport/:category/:league/refresh",
    post(event_list_refresh_handler),
)
.route(
    "/api/events/:sport/:category/:league",
    get(event_list_handler),
)
```

If `ApiResponse` fields are private and block construction from `events.rs`, change them to:

```rust
pub(crate) ok: bool,
pub(crate) data: Option<T>,
pub(crate) error: Option<String>,
```

- [ ] **Step 6: Run backend tests and verify green**

Run:

```bash
cargo test test_extracts_world_championship_event_row test_event_api_reads_event_cache_without_overwriting_category_cache
```

Expected: PASS.

Then run:

```bash
cargo test --all
```

Expected: PASS. If sandbox blocks local port tests, rerun with escalation.

- [ ] **Step 7: Commit backend work**

```bash
git add src/menu/events.rs src/menu/mod.rs src/menu/handlers.rs src/menu/storage.rs src/lib.rs tests/menu_third_level_test.rs
git commit -m "feat(menu): add event list backend"
```

## Task 3: Frontend Event Rendering

**Files:**
- Modify: `tests/menu_page_config_test.js`
- Modify: `public/menu.html`

- [ ] **Step 1: Add failing frontend tests**

In `tests/menu_page_config_test.js`, extend `loadMenuScript` so callers can pass a fetch implementation:

```javascript
function loadMenuScript(pathname, fetchImpl) {
  // keep existing body
  fetch: fetchImpl || (async () => ({ json: async () => ({ ok: true, data: { categories: [] } }) })),
}
```

Add tests:

```javascript
function testFourthLevelEventConfig() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  const config = context.getPageConfig();

  assert.strictEqual(config.eventApiUrl, '/api/events/football/world/world-championship-2026');
  assert.strictEqual(config.eventRefreshUrl, '/api/events/football/world/world-championship-2026/refresh');
}

function testRenderEventRows() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  context.data = [{
    matchup: 'Mexico VS South Africa',
    start_time: '18 Jun 2026, 03:00',
    url: '/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2'
  }];
  context.dataMode = 'events';
  context.renderPage(1);

  assert.match(context.__elements.content.innerHTML, /Mexico VS South Africa/);
  assert.match(context.__elements.content.innerHTML, /18 Jun 2026, 03:00/);
  assert.match(context.__elements.content.innerHTML, /football\/h2h\/mexico-O6iHcNkd\/south-africa-W2ijYvlr/);
}

function testEventPaginationUsesTenRows() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  context.data = Array.from({ length: 11 }, (_, index) => ({
    matchup: `Team ${index + 1} VS Opponent ${index + 1}`,
    start_time: '18 Jun 2026, 03:00',
    url: `/event-${index + 1}`
  }));
  context.dataMode = 'events';
  context.renderPage(1);

  assert.match(context.__elements.content.innerHTML, /Team 10 VS Opponent 10/);
  assert.doesNotMatch(context.__elements.content.innerHTML, /Team 11 VS Opponent 11/);
  assert.match(context.__elements.pagination.innerHTML, /下一页/);
}
```

Call the new tests at the bottom.

- [ ] **Step 2: Run frontend tests and verify red**

Run:

```bash
node tests/menu_page_config_test.js
```

Expected: FAIL because `eventApiUrl`, `dataMode`, and event rendering do not exist.

- [ ] **Step 3: Add event config and mode state**

In `public/menu.html`, add global state:

```javascript
let dataMode = 'categories';
```

In fourth-level `getPageConfig()`, add:

```javascript
eventApiUrl: `/api/events/${sport}/${category}/${league}`,
eventRefreshUrl: `/api/events/${sport}/${category}/${league}/refresh`,
eventLocalKey: `events_${sport}_${category}_${league}`,
```

- [ ] **Step 4: Implement event-first fetch fallback**

Update `fetchData()`:

```javascript
if (config.type === 'fourth-level' && config.eventApiUrl) {
  const eventResponse = await fetch(config.eventApiUrl);
  const eventResult = await eventResponse.json();
  if (eventResult.ok && eventResult.data && Array.isArray(eventResult.data.events) && eventResult.data.events.length > 0) {
    data = eventResult.data.events;
    dataMode = 'events';
    document.getElementById('error-container').innerHTML = '';
    renderPage(1);
    return;
  }
}
```

Then keep the existing category fetch and set `dataMode = 'categories'` before assigning `result.data.categories`.

- [ ] **Step 5: Render event stats and event table**

Update `renderStats()` so event mode returns:

```javascript
if (dataMode === 'events') {
  statsBar.innerHTML = `<span class="stat-badge">总计: ${data.length} 场比赛</span>`;
  return;
}
```

Update `renderPage()` before category rendering:

```javascript
if (dataMode === 'events') {
  document.getElementById('content').innerHTML = `
    <div class="list-table">
      <div class="list-header event-grid">
        <span>比赛</span>
        <span>开始时间</span>
        <span>链接</span>
      </div>
      ${pageItems.map(item => `
        <a href="${item.url || '#'}" target="_blank" class="list-row event-grid">
          <span class="name">${item.matchup || ''}</span>
          <span class="time">${item.start_time || ''}</span>
          <span class="url">${item.url || '(无链接)'}</span>
        </a>
      `).join('')}
    </div>
  `;
  renderPagination(page, totalPages);
  return;
}
```

Add CSS:

```css
.list-header.event-grid,
.list-row.event-grid {
  grid-template-columns: 1.5fr 1fr 2fr;
}
.list-row .time {
  font-size: 0.85rem;
  color: #475569;
}
```

- [ ] **Step 6: Update refresh/save for event mode**

For `refreshData()`, if `dataMode === 'events'` or fourth-level has `eventRefreshUrl`, POST event refresh first and use events when non-empty; otherwise fall back to category refresh.

For `saveData()`, use `config.eventLocalKey` when `dataMode === 'events'`:

```javascript
const localKey = dataMode === 'events' && config.eventLocalKey ? config.eventLocalKey : config.localKey;
localStorage.setItem(localKey, JSON.stringify(data));
```

- [ ] **Step 7: Run frontend tests and verify green**

Run:

```bash
node tests/menu_page_config_test.js
```

Expected: PASS.

- [ ] **Step 8: Commit frontend work**

```bash
git add public/menu.html tests/menu_page_config_test.js
git commit -m "feat(menu): render event rows on fourth-level pages"
```

## Task 4: Documentation, State, and Verification

**Files:**
- Modify: `web_design.md`
- Modify: `architect.md`
- Modify: `test.md`
- Modify: `session.md`
- Modify: `change_log.md`
- Modify: `openspec/changes/event-list-parse/tasks.md`

- [ ] **Step 1: Update documentation**

Update `web_design.md` with:

```md
#### GET /api/events/:sport/:category/:league

获取指定赛事页的比赛列表，字段包括 matchup、home_team、away_team、start_time、url。
```

Update the fourth-level page section to say fourth-level pages first render event rows when event data exists, otherwise category rows.

Update `architect.md` API table with `/api/events/:sport/:category/:league` and refresh route. Add `src/menu/events.rs` to the directory structure.

Update `test.md` with the new Rust and Node test names.

- [ ] **Step 2: Update session and change log**

Set `session.md` task fields to `event-list-parse`, mark current completed and pending steps accurately. Append a `change_log.md` entry with files changed and test results.

- [ ] **Step 3: Check off completed OpenSpec tasks**

Edit `openspec/changes/event-list-parse/tasks.md` and mark all completed items with `- [x]` only after implementation and verification pass.

- [ ] **Step 4: Run full verification**

Run:

```bash
node tests/menu_page_config_test.js
cargo test --all
cargo build
```

Expected: all pass. If `cargo test --all` fails because sandbox blocks local port binding for wiremock/TcpListener tests, rerun with escalation and record that in `change_log.md`.

- [ ] **Step 5: Commit docs and state**

```bash
git add web_design.md architect.md test.md session.md change_log.md openspec/changes/event-list-parse docs/superpowers/specs/2026-06-09-event-list-parse-design.md docs/superpowers/plans/2026-06-09-event-list-parse.md
git commit -m "docs: record event list parse workflow"
```

## Self-Review

Spec coverage:

- Event API: Task 2 implements `/api/events/:sport/:category/:league`; Task 1 tests it.
- Event row extraction: Task 2 parser implements it; Task 1 parser test covers Mexico/South Africa.
- Event cache separation: Task 2 cache helpers use `events_`; Task 1 API/cache test covers category separation.
- Fourth-level event rendering: Task 3 implements event-first fetch and event table; frontend tests cover rendering and fallback.
- Pagination: Task 3 tests and implementation preserve 10-row paging.

Placeholder scan: no `TBD`, `TODO`, or unspecified implementation steps remain. Helper functions have explicit expected behavior and are constrained by tests.

Type consistency: `EventRow`, `EventData`, `events`, `start_time`, `eventApiUrl`, and `dataMode` names are consistent across backend, frontend, and tests.
