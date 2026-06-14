# Comet Design Handoff

- Change: analysis-refresh-chart-filters
- Phase: design
- Mode: compact
- Context hash: 4581689916872601479ec2565461f08927c923e394d5c3409c96205623c80512

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/analysis-refresh-chart-filters/proposal.md

- Source: openspec/changes/analysis-refresh-chart-filters/proposal.md
- Lines: 1-23
- SHA256: 8432ae6bb58e0bed8c589c0e43e05bcf5a90fa2c5ba260298c580d0e39e430f9

```md
## Why

The analysis page refreshes OddsPortal monitoring data every second, which can replace the DOM while the user is trying to open or use selectors. The combined all-market history chart also shows every win-probability line at once, making noisy bookmakers or outcomes impossible to hide during comparison.

## What Changes

- Make background refreshes non-disruptive for user interaction on the analysis page.
- Preserve user-selected monitored match/card state across refreshes.
- Add controls to the all-market history chart so users can hide and show individual win-probability series they do not want to inspect.
- Keep backend API routes and response shapes unchanged.

## Capabilities

### New Capabilities
- `analysis-refresh-chart-controls`: Analysis page behavior for non-disruptive background refreshes and per-series visibility controls in the all-market time-series chart.

### Modified Capabilities

## Impact

- Affected code: `public/analysis.html`
- Affected tests: `tests/analysis_page_test.js`
- No database, backend route, API contract, or dependency changes.
```

## openspec/changes/analysis-refresh-chart-filters/design.md

- Source: openspec/changes/analysis-refresh-chart-filters/design.md
- Lines: 1-46
- SHA256: a26de324cb5cdecf511be0c9263121906af737ed368c619ea391a65b56e20628

```md
## Context

The analysis page is a single HTML/JavaScript view that polls `/api/analysis/odds/collect` on an interval and re-renders the summary, selector, cards, and OddsPortal history chart. Existing frontend state already tracks the selected monitored item, but a background refresh can still replace interactive controls while the user is trying to select or filter content. The all-market chart renders multiple probability series in one SVG and currently treats the legend as read-only.

## Goals / Non-Goals

**Goals:**
- Prevent automatic background refreshes from interrupting user selection or chart filtering.
- Preserve the selected monitored item when refreshed API data still contains that item.
- Add per-series visibility state for the all-market chart, exposed as compact controls in the legend.
- Keep hidden series excluded from the SVG and latest-value legend display until re-enabled.
- Cover the behavior with focused JavaScript regression tests.

**Non-Goals:**
- Do not change OddsPortal collection cadence on the backend.
- Do not change API routes, payload structure, storage, or history aggregation.
- Do not add a frontend framework or new dependency.
- Do not persist chart filter choices across browser sessions.

## Decisions

1. Keep refresh protection in frontend state.
   - Decision: Track whether a control is actively being used or recently changed, and skip or defer background refresh work during that short interaction window.
   - Rationale: The problem is DOM replacement during user interaction, not backend data collection.
   - Alternative considered: Increase the global refresh interval only. That reduces frequency but still permits a refresh at the wrong moment.

2. Preserve selected analysis item by stable item key.
   - Decision: Continue deriving a stable key from match identifiers and only fall back to the first item when the selected key no longer exists.
   - Rationale: This keeps user intent stable across fresh API responses while handling deleted or completed monitor entries gracefully.
   - Alternative considered: Preserve by array index. Indexes can shift as the API returns new ordering.

3. Model chart line visibility with a per-series key set.
   - Decision: Build a stable key from market, bookmaker, outcome, and series type, then store hidden keys in frontend state.
   - Rationale: The filter belongs to display state and should survive chart re-rendering during refreshes.
   - Alternative considered: Remove series data before `buildSeries`. That would blur raw data construction with display filtering.

4. Use legend controls as the filter surface.
   - Decision: Render each legend row with a checkbox/toggle that hides or shows that line.
   - Rationale: Users identify unwanted lines from the legend labels, so the control belongs next to each label.
   - Alternative considered: A separate multi-select menu. It is harder to scan when many similarly named lines exist.

## Risks / Trade-offs

- Background refresh may be skipped while the user interacts repeatedly -> Mitigation: only skip automatic background refreshes, while manual refresh still works.
- Hidden-line state can reference series that disappeared from later data -> Mitigation: prune hidden keys against the latest `buildSeries` output during render.
- The chart can become empty when all lines are hidden -> Mitigation: render an explicit empty state for the filtered chart while keeping legend controls visible for re-enabling lines.
```

## openspec/changes/analysis-refresh-chart-filters/tasks.md

- Source: openspec/changes/analysis-refresh-chart-filters/tasks.md
- Lines: 1-18
- SHA256: c71bfa7779f4afe83e8d1aec0ab5df933076777066ee8e2421c0628305f03302

```md
## 1. Refresh Interaction

- [ ] 1.1 Add frontend state to detect active or recent user interaction with analysis controls.
- [ ] 1.2 Make automatic background refreshes skip or defer DOM replacement while interaction protection is active.
- [ ] 1.3 Preserve the selected monitored item across refreshed API responses when its stable key remains available.

## 2. Chart Series Filtering

- [ ] 2.1 Add stable per-series keys for all-market history lines.
- [ ] 2.2 Render legend controls that toggle individual series visibility.
- [ ] 2.3 Filter hidden series out of the combined SVG chart while keeping controls available.
- [ ] 2.4 Show a clear empty filtered state when all series are hidden.

## 3. Verification

- [ ] 3.1 Add focused JavaScript regression tests for refresh interaction protection and selected-item preservation.
- [ ] 3.2 Add focused JavaScript regression tests for chart series hide/show rendering.
- [ ] 3.3 Run the relevant frontend/page tests and fix regressions.
```

## openspec/changes/analysis-refresh-chart-filters/specs/analysis-refresh-chart-controls/spec.md

- Source: openspec/changes/analysis-refresh-chart-filters/specs/analysis-refresh-chart-controls/spec.md
- Lines: 1-38
- SHA256: 0e4b834e13640a18555250136b4d7c8ce2dabfb352e1ad44bc22373714d91918

```md
## ADDED Requirements

### Requirement: Non-disruptive analysis refresh
The analysis page SHALL avoid replacing interactive controls during automatic background refreshes.

#### Scenario: User is choosing a monitored item
- **WHEN** the user is interacting with the monitored-item selector and an automatic background refresh interval fires
- **THEN** the page keeps the current selector DOM stable and does not interrupt the user's selection

#### Scenario: Manual refresh still reloads data
- **WHEN** the user clicks the manual refresh button
- **THEN** the page reloads analysis data even if a recent interaction occurred

### Requirement: Preserve selected monitored item
The analysis page SHALL preserve the selected monitored item across refreshes when that item is still present in the API response.

#### Scenario: Selected item remains available
- **WHEN** refreshed analysis data includes the previously selected monitored item
- **THEN** the page continues rendering that item instead of falling back to the first item

#### Scenario: Selected item disappears
- **WHEN** refreshed analysis data no longer includes the previously selected monitored item
- **THEN** the page falls back to the first available monitored item

### Requirement: Filter all-market chart series
The all-market OddsPortal history chart SHALL allow users to hide and show individual win-probability series.

#### Scenario: Hide one series
- **WHEN** the user disables a series from the chart legend
- **THEN** that series is omitted from the combined SVG chart while other enabled series remain visible

#### Scenario: Show hidden series
- **WHEN** the user re-enables a previously hidden series from the chart legend
- **THEN** that series appears in the combined SVG chart again

#### Scenario: All series hidden
- **WHEN** the user hides every available series
- **THEN** the chart area shows an empty filtered state and keeps legend controls available
```

