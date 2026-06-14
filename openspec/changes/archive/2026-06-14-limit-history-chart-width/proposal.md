## Why

The all-market OddsPortal chart creates its SVG width from `snapshots.length * 72`, so the horizontal axis grows without bound as history accumulates. This makes the chart awkward to inspect because the page can require a very long horizontal scroll.

## Root Cause

`combinedSeriesChart` couples chart width to the number of captured snapshots even though the x-axis already samples displayed tick labels. The renderer should fit the available chart area and compress the timeline instead of extending the SVG for every additional point.

## Fix Goal

- Keep the combined history chart at a stable, bounded horizontal width.
- Preserve all series points, tooltips, and sampled time ticks.
- Avoid backend/API or data model changes.

## Impact

- Affected code: `public/analysis.html`
- Affected tests: `tests/analysis_page_test.js`
