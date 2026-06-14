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
