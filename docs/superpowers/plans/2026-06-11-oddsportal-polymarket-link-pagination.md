---
change: oddsportal-polymarket-link-pagination
design-doc: docs/superpowers/specs/2026-06-11-oddsportal-polymarket-link-pagination-design.md
base-ref: 03966e8b5993634f43782a98bf9a5dff60fb82a6
---

# OddsPortal Polymarket Link Pagination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render OddsPortal and Polymarket as explicit event-row link buttons, resolving Polymarket by exact slug through `rs-clob-client-v2`.

**Architecture:** Extend `EventRow` with an optional `polymarket_url`, keep `/api/events` stable, isolate Polymarket slug derivation and lookup in `src/menu/events.rs`, and update `public/menu.html` to render link buttons in the existing "链接" column.

**Tech Stack:** Rust 2021, Axum, serde, chrono, `rs-clob-client-v2`, static HTML/JavaScript, Node VM frontend tests.

---

### Task 1: Backend Event Link Contract

**Files:**
- Modify: `tests/menu_third_level_test.rs`
- Modify: `src/menu/events.rs`
- Modify: `Cargo.toml`

- [x] **Step 1: Write failing Rust tests**

Add tests that expect `polymarket_url` serialization compatibility and World Cup slug generation:

```rust
#[test]
fn test_generates_world_cup_polymarket_slug_candidate() {
    let event = EventRow {
        slug: "mexico-vs-south-africa".to_string(),
        home_team: "Mexico".to_string(),
        away_team: "South Africa".to_string(),
        matchup: "Mexico VS South Africa".to_string(),
        start_time: "11 Jun 2026, 21:00".to_string(),
        url: "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
        polymarket_url: None,
    };

    let slugs = polymarket_analysis::menu::events::polymarket_slug_candidates(
        &event,
        "football",
        "world",
        "world-championship-2026",
    );

    assert!(slugs.iter().any(|slug| slug == "fifwc-mex-rsa-2026-06-11"));
}

#[test]
fn test_event_row_omits_missing_polymarket_url() {
    let event = EventRow {
        slug: "mexico-vs-south-africa".to_string(),
        home_team: "Mexico".to_string(),
        away_team: "South Africa".to_string(),
        matchup: "Mexico VS South Africa".to_string(),
        start_time: "11 Jun 2026, 21:00".to_string(),
        url: "https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2".to_string(),
        polymarket_url: None,
    };

    let json = serde_json::to_value(event).expect("event row should serialize");
    assert!(json.get("polymarket_url").is_none());
}
```

- [x] **Step 2: Run red test**

Run: `cargo test test_generates_world_cup_polymarket_slug_candidate test_event_row_omits_missing_polymarket_url`

Expected: FAIL because `polymarket_url` and `polymarket_slug_candidates` do not exist.

- [x] **Step 3: Implement contract and slug candidates**

Add `rs-clob-client-v2 = "0.2.1"` to `Cargo.toml`. Extend `EventRow`, update construction sites, and add public `polymarket_slug_candidates`.

- [x] **Step 4: Run green test**

Run: `cargo test test_generates_world_cup_polymarket_slug_candidate test_event_row_omits_missing_polymarket_url`

Expected: PASS.

### Task 2: Polymarket Lookup

**Files:**
- Modify: `tests/menu_third_level_test.rs`
- Modify: `src/menu/events.rs`

- [x] **Step 1: Write failing URL helper test**

Add a focused test:

```rust
#[test]
fn test_polymarket_world_cup_url_from_slug() {
    assert_eq!(
        polymarket_analysis::menu::events::polymarket_public_url_for_slug(
            "fifwc-mex-rsa-2026-06-11",
            "football",
            "world",
            "world-championship-2026",
        ),
        Some("https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11".to_string())
    );
}
```

- [x] **Step 2: Run red test**

Run: `cargo test test_polymarket_world_cup_url_from_slug`

Expected: FAIL because the helper does not exist.

- [x] **Step 3: Implement lookup helpers**

Implement `polymarket_public_url_for_slug`, a `build_polymarket_client` helper, and async enrichment that tries `get_event_by_slug` then `get_market_by_slug`, exact-matching returned slug values.

- [x] **Step 4: Run green test**

Run: `cargo test test_polymarket_world_cup_url_from_slug`

Expected: PASS.

### Task 3: Frontend Link Buttons

**Files:**
- Modify: `tests/menu_page_config_test.js`
- Modify: `public/menu.html`

- [x] **Step 1: Write failing Node tests**

Add tests for enabled buttons and disabled fallback:

```javascript
function testRenderEventLinkButtons() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      url: 'https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2',
      polymarket_url: 'https://polymarket.com/sports/world-cup/fifwc-mex-rsa-2026-06-11'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /OddsPortal/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
  assert.match(context.__elements.content.innerHTML, /href="https:\/\/polymarket\.com\/sports\/world-cup\/fifwc-mex-rsa-2026-06-11"/);
}

function testRenderDisabledPolymarketButton() {
  const context = loadMenuScript('/menu/football/world/world-championship-2026');
  vm.runInContext(`
    data = [{
      matchup: 'Mexico VS South Africa',
      start_time: '11 Jun 2026, 21:00',
      url: 'https://www.oddsportal.com/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2'
    }];
    dataMode = 'events';
    renderPage(1);
  `, context);

  assert.match(context.__elements.content.innerHTML, /link-btn disabled/);
  assert.match(context.__elements.content.innerHTML, /Polymarket/);
}
```

- [x] **Step 2: Run red test**

Run: `node tests/menu_page_config_test.js`

Expected: FAIL because event rows still render the raw URL instead of buttons.

- [x] **Step 3: Implement event link button rendering**

Add `.link-actions`, `.link-btn`, and disabled styles. Replace event `<a class="list-row">` rows with non-anchor rows containing `renderEventLinks(item)`.

- [x] **Step 4: Run green test**

Run: `node tests/menu_page_config_test.js`

Expected: PASS.

### Task 4: Final Verification

**Files:**
- Modify: `openspec/changes/oddsportal-polymarket-link-pagination/tasks.md`

- [x] **Step 1: Mark tasks complete**

Check off completed OpenSpec tasks after implementation and tests pass.

- [x] **Step 2: Run full verification**

Run:

```bash
cargo test --all
cargo build
node tests/menu_page_config_test.js
```

Expected: all commands exit 0.
