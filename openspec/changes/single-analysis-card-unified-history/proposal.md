## Why

The analysis page currently renders every monitored odds snapshot returned by the API, which makes the schedule area noisy when multiple snapshots exist. Its OddsPortal history area also renders each series as a separate small chart, making it harder to compare lines over the same time axis.

## What Changes

- Render one analysis schedule item at a time, with a manual selector when multiple items are available.
- Keep the API response shape unchanged and limit the display selection to the frontend.
- Render all OddsPortal market-history series in one combined time-series chart with a shared axis and legend.

## Capabilities

### New Capabilities
- `analysis-odds-display`: Analysis page presentation behavior for visible schedule items and combined OddsPortal history charts.

### Modified Capabilities

## Impact

- Affected code: `public/analysis.html`
- Affected tests: `tests/analysis_page_test.js`
- No database, API, dependency, or route changes.
