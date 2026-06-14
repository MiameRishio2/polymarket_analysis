---
comet_change: analysis-refresh-chart-filters
role: technical-design
canonical_spec: openspec
---

# Analysis Refresh and Chart Filters Design

## Context

`public/analysis.html` is a single-page HTML/JavaScript view. It loads analysis data, renders one selected monitored item, fetches OddsPortal history for that item, and redraws the combined all-market SVG chart. The page currently refreshes on a one-second interval, so a background refresh can replace the selector or chart legend while the user is trying to interact with it.

The existing implementation already has useful boundaries:

- `analysisItemKey`, `visibleAnalysisItems`, and `renderAnalysisSelector` own monitored-item selection.
- `buildSeries` converts history snapshots into stable series objects.
- `combinedSeriesChart` renders SVG from a series array.
- `renderMarketHistory` owns chart and legend composition.

This change keeps those boundaries and adds display state around them.

## Technical Approach

### Refresh Interaction Protection

Add a small frontend interaction guard:

- Track the last interaction timestamp when the monitored-item selector or chart filter controls receive focus, pointer, input, or change events.
- Treat automatic refresh as protected for a short window after interaction.
- Skip automatic refresh work when protection is active.
- Keep manual refresh explicit: a button click still calls `loadAnalysis(..., { force: true })` or equivalent and bypasses the automatic-refresh guard.

This solves the real failure mode: DOM replacement during a user action. It avoids weakening the backend collection cadence and avoids adding debounce logic to API calls.

### Selected Item Preservation

Keep the selected analysis item in `selectedAnalysisItemKey`. On each successful data load:

- If the selected key still exists in the returned items, render that item.
- If it no longer exists, fall back to the first item and update `selectedAnalysisItemKey`.

Keys remain derived from `match_id`, URL fields, matchup, or index fallback. The implementation should prefer stable identifiers already present in the item.

### Chart Series Filtering

Use the existing `series.key` as the filter identity and add a `hiddenSeriesKeys` set.

`renderMarketHistory` should:

- Build the full series list from history.
- Prune hidden keys that no longer exist in the current full list.
- Render legend rows for the full list, each with a checkbox reflecting visibility.
- Pass only visible series to `combinedSeriesChart`.
- Render an explicit filtered-empty state if the full list exists but every line is hidden.

The legend remains visible even when all lines are hidden so the user can re-enable individual lines.

### Data Flow

```text
interval/manual refresh
        |
        v
refreshAnalysis -> loadAnalysis
        |
        v
items -> selectedAnalysisItemKey -> visible item
        |
        v
history snapshots -> buildSeries -> full series
        |
        v
hiddenSeriesKeys -> visible series -> combinedSeriesChart
        |
        v
legend controls update hiddenSeriesKeys and re-render current chart/card
```

## Alternatives Considered

### Increase Refresh Interval Only

This is simple but incomplete. A slower interval still can fire while the select menu or legend control is being used.

### Pause Backend Collection

This would reduce freshness and changes the wrong layer. The issue is frontend DOM replacement, not collection itself.

### Filter Before `buildSeries`

Filtering raw history rows would mix data normalization with display state. Filtering after `buildSeries` keeps raw series construction testable and lets the legend continue to show every available line.

## Edge Cases

- Empty history: keep the existing "no history" state.
- Unknown or removed bookmaker rows: preserve the existing filtering behavior.
- Hidden series disappears after refresh: remove its stale hidden key.
- All series hidden: show a filtered-empty message and keep legend controls enabled.
- User selects an item that disappears before the next refresh: fall back to the first item.

## Testing Strategy

Add focused tests in `tests/analysis_page_test.js`:

- `visibleAnalysisItems` preserves a selected item when present and falls back when absent.
- Automatic refresh skips while interaction protection is active.
- Manual refresh still runs while interaction protection is active.
- `renderMarketHistory` renders checkbox controls for each series.
- Hidden series are excluded from the SVG/chart output.
- All-hidden state shows the filtered-empty message while legend controls remain present.

Run the existing page JavaScript tests after implementation.
