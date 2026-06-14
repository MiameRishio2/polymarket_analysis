## Overview

This is a local analysis page presentation tweak. The backend can continue returning all available snapshot rows, while the page lets the user choose which row to display. History rendering stays driven by `buildSeries`, but the renderer changes from one sparkline per row to one shared SVG chart.

## Implementation

- Add a small `visibleAnalysisItems(items, selectedKey)` helper that returns the selected item, falling back to the first item.
- Add a compact selector above the card when more than one monitored item is available.
- Use the visible item list for summary counts, history loading, and card rendering.
- Add `combinedSeriesChart(snapshots, series, colors)` to draw every history series on one SVG with a common x axis and a normalized y scale.
- Replace per-row sparkline output with a single combined chart and a compact legend/latest-value list.

## Non-Goals

- Do not change `/api/analysis/odds` or history endpoint contracts.
- Do not change snapshot collection or stored odds history.
