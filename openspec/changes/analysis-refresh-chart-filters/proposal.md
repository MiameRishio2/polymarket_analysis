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
