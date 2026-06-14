---
change: analysis-refresh-chart-filters
design-doc: docs/superpowers/specs/2026-06-14-analysis-refresh-chart-filters-design.md
base-ref: 3ee2da26f8889723170f9a83e134ddd4041fccb5
---

# Analysis Refresh Chart Filters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the analysis page refresh without interrupting user selections and allow users to hide/show individual all-market chart lines.

**Architecture:** Keep this as a frontend-only change in `public/analysis.html`. Add small display-state helpers for interaction protection and hidden chart series, then test those helpers and rendered HTML through the existing VM-based JavaScript test file.

**Tech Stack:** Plain HTML/CSS/JavaScript, Node.js `assert`, `fs`, and `vm` for tests.

---

## File Structure

- Modify `public/analysis.html`: interaction protection state, selector event hooks, chart visibility helpers, legend checkbox rendering, filtered-empty state, and refresh bypass for manual refresh.
- Modify `tests/analysis_page_test.js`: VM test harness additions plus regression tests for refresh protection, selected-item fallback, and chart series filtering.
- Modify `openspec/changes/analysis-refresh-chart-filters/tasks.md`: check off completed tasks after implementation and verification.

### Task 1: Refresh Interaction Protection

**Files:**
- Modify: `tests/analysis_page_test.js`
- Modify: `public/analysis.html`
- Track: `openspec/changes/analysis-refresh-chart-filters/tasks.md`

- [ ] **Step 1: Write failing tests for selected item preservation and refresh guard**

Add tests equivalent to:

```javascript
function testVisibleAnalysisItemsFallsBackWhenSelectionDisappears() {
  const context = loadAnalysisScript();
  const items = [
    { match_id: 'first', matchup: 'First Match' },
    { match_id: 'second', matchup: 'Second Match' }
  ];

  const visibleItems = context.visibleAnalysisItems(items, 'missing');

  assert.strictEqual(visibleItems.length, 1);
  assert.strictEqual(visibleItems[0].match_id, 'first');
}

async function testAutomaticRefreshSkipsDuringInteraction() {
  const fetchCalls = [];
  const context = loadAnalysisScript({
    fetch: async url => {
      fetchCalls.push(url);
      return { json: async () => ({ ok: true, data: [] }) };
    }
  });

  context.markAnalysisInteraction();
  await context.refreshAnalysis(false, { automatic: true });

  assert.strictEqual(fetchCalls.length, 0);
}

async function testManualRefreshBypassesInteractionProtection() {
  const fetchCalls = [];
  const context = loadAnalysisScript({
    fetch: async url => {
      fetchCalls.push(url);
      return { json: async () => ({ ok: true, data: [] }) };
    }
  });

  context.markAnalysisInteraction();
  await context.refreshAnalysis(false, { automatic: false, force: true });

  assert.strictEqual(fetchCalls.length, 1);
  assert.strictEqual(fetchCalls[0], '/api/analysis/odds/collect');
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node tests/analysis_page_test.js`

Expected before implementation: failure because `markAnalysisInteraction` or the new `refreshAnalysis` options behavior is missing.

- [ ] **Step 3: Implement refresh interaction guard**

In `public/analysis.html`, add state and helpers:

```javascript
const interactionProtectionMs = 1200;
let lastAnalysisInteractionAt = 0;

function nowMs() {
  return Date.now();
}

function markAnalysisInteraction() {
  lastAnalysisInteractionAt = nowMs();
}

function analysisInteractionProtected() {
  return nowMs() - lastAnalysisInteractionAt < interactionProtectionMs;
}
```

Update selector rendering to mark interaction:

```html
<select id="analysis-item-select"
  onfocus="markAnalysisInteraction()"
  onpointerdown="markAnalysisInteraction()"
  oninput="markAnalysisInteraction()"
  onchange="selectAnalysisItem(this.value)">
```

Update `selectAnalysisItem`:

```javascript
function selectAnalysisItem(value) {
  markAnalysisInteraction();
  selectedAnalysisItemKey = String(value || '');
  loadAnalysis(false, false, { force: true });
}
```

Update `loadAnalysis` / `refreshAnalysis` signatures:

```javascript
async function loadAnalysis(showLoading = true, collect = false, options = {}) {
  const automatic = Boolean(options.automatic);
  const force = Boolean(options.force);
  if (automatic && !force && analysisInteractionProtected()) return false;
  // existing body
  return true;
}

async function refreshAnalysis(showLoading = false, options = {}) {
  if (refreshInFlight) return false;
  if (options.automatic && !options.force && analysisInteractionProtected()) return false;
  refreshInFlight = true;
  try {
    return await loadAnalysis(showLoading, true, options);
  } finally {
    refreshInFlight = false;
  }
}
```

Update interval and manual button path so automatic refresh passes `{ automatic: true }`, while the button remains forceful.

- [ ] **Step 4: Run test to verify it passes**

Run: `node tests/analysis_page_test.js`

Expected: all tests pass.

- [ ] **Step 5: Mark OpenSpec tasks complete**

Check off tasks 1.1, 1.2, 1.3, 3.1 in `openspec/changes/analysis-refresh-chart-filters/tasks.md`.

### Task 2: Chart Series Filtering

**Files:**
- Modify: `tests/analysis_page_test.js`
- Modify: `public/analysis.html`
- Track: `openspec/changes/analysis-refresh-chart-filters/tasks.md`

- [ ] **Step 1: Write failing tests for chart controls and hidden series**

Add tests equivalent to:

```javascript
function testMarketHistoryRendersSeriesFilterControls() {
  const context = loadAnalysisScript();
  const html = context.renderMarketHistory(twoBookmakerHistoryFixture());

  assert.ok(html.includes('type="checkbox"'));
  assert.ok(html.includes('onchange="toggleHistorySeries'));
  assert.ok(html.includes('aria-label="显示或隐藏'));
}

function testMarketHistoryOmitsHiddenSeriesFromChart() {
  const context = loadAnalysisScript();
  const built = context.buildSeries(twoBookmakerHistoryFixture());
  context.hideHistorySeries(built.series[0].key);

  const html = context.renderMarketHistory(twoBookmakerHistoryFixture());

  assert.ok(!html.includes(`${built.series[0].bookmakerId} · ${built.series[0].outcomeLabel} · 1.50`));
  assert.ok(html.includes(`${built.series[1].bookmakerId} · ${built.series[1].outcomeLabel}`));
}

function testMarketHistoryShowsEmptyStateWhenAllSeriesHidden() {
  const context = loadAnalysisScript();
  const built = context.buildSeries(twoBookmakerHistoryFixture());
  built.series.forEach(series => context.hideHistorySeries(series.key));

  const html = context.renderMarketHistory(twoBookmakerHistoryFixture());

  assert.ok(html.includes('已隐藏全部时序线'));
  assert.ok(html.includes('type="checkbox"'));
}
```

Use a shared `twoBookmakerHistoryFixture()` helper in the test file to avoid duplicated fixture rows.

- [ ] **Step 2: Run test to verify it fails**

Run: `node tests/analysis_page_test.js`

Expected before implementation: failure because filter controls and hide helpers do not exist.

- [ ] **Step 3: Implement hidden series state and controls**

In `public/analysis.html`, add:

```javascript
const hiddenHistorySeriesKeys = new Set();

function hideHistorySeries(key) {
  hiddenHistorySeriesKeys.add(String(key || ''));
}

function showHistorySeries(key) {
  hiddenHistorySeriesKeys.delete(String(key || ''));
}

function toggleHistorySeries(key, checked) {
  if (checked) showHistorySeries(key);
  else hideHistorySeries(key);
  const card = document.querySelector('[data-history-card="active"]');
  if (card && latestVisibleHistory) {
    card.outerHTML = renderCard(latestVisibleItem, latestVisibleHistory);
  }
}

function pruneHiddenHistorySeries(series) {
  const available = new Set(series.map(item => item.key));
  Array.from(hiddenHistorySeriesKeys).forEach(key => {
    if (!available.has(key)) hiddenHistorySeriesKeys.delete(key);
  });
}

function visibleHistorySeries(series) {
  pruneHiddenHistorySeries(series);
  return series.filter(item => !hiddenHistorySeriesKeys.has(item.key));
}
```

If direct card re-rendering needs too much state, use the simpler path: make `toggleHistorySeries` call `loadAnalysis(false, false, { force: true })`.

Update `renderMarketHistory` to compute:

```javascript
const visibleSeries = visibleHistorySeries(series);
const chartHtml = visibleSeries.length > 0
  ? combinedSeriesChart(snapshots, visibleSeries, colors)
  : '<div class="history-empty">已隐藏全部时序线。勾选下方项目可重新显示。</div>';
```

Render each legend row with a checkbox:

```html
<input
  class="legend-toggle"
  type="checkbox"
  checked
  aria-label="显示或隐藏 ${escapeHtml(label)}"
  onchange="toggleHistorySeries('${escapeHtml(row.key)}', this.checked)"
/>
```

When a row key is hidden, omit the `checked` attribute.

- [ ] **Step 4: Add CSS for legend checkbox controls**

In `public/analysis.html`, keep the legend compact:

```css
.legend-toggle {
  width: 1rem;
  height: 1rem;
  margin: 0;
  accent-color: #2563eb;
}
.legend-item {
  grid-template-columns: 1rem 0.75rem 1fr auto;
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `node tests/analysis_page_test.js`

Expected: all tests pass.

- [ ] **Step 6: Mark OpenSpec tasks complete**

Check off tasks 2.1, 2.2, 2.3, 2.4, and 3.2 in `openspec/changes/analysis-refresh-chart-filters/tasks.md`.

### Task 3: Final Verification

**Files:**
- Modify: `openspec/changes/analysis-refresh-chart-filters/tasks.md`

- [ ] **Step 1: Run focused page tests**

Run: `node tests/analysis_page_test.js`

Expected: process exits 0 with no assertion failures.

- [ ] **Step 2: Run available broader verification**

Run the project test command if available from repository metadata. If no single project-wide command exists, record that only the focused page tests were applicable.

- [ ] **Step 3: Mark verification task complete**

Check off task 3.3 in `openspec/changes/analysis-refresh-chart-filters/tasks.md`.

- [ ] **Step 4: Review diff**

Run: `git diff -- public/analysis.html tests/analysis_page_test.js openspec/changes/analysis-refresh-chart-filters docs/superpowers/specs/2026-06-14-analysis-refresh-chart-filters-design.md docs/superpowers/plans/2026-06-14-analysis-refresh-chart-filters.md`

Expected: diff contains only this change plus pre-existing compatible edits in the touched page/test files.
