## Why

OddsPortal history rows can contain placeholder bookmaker labels such as `Bookmaker 1159` when the feed lacks a readable bookmaker slug. The analysis history UI currently renders those placeholders as if they were real bookmaker names, which makes the market timeline noisy and misleading.

## What Changes

- Hide OddsPortal market-history lines whose bookmaker name is missing, numeric-only, or a generated `Bookmaker {id}` placeholder.
- Preserve lines with known readable bookmaker names, including existing ID-to-name mappings such as 1xBet and bet365.
- Keep the API shape unchanged.

## Capabilities

### New Capabilities
- `analysis-odds-history`: Analysis page behavior for rendering OddsPortal market-history series.

### Modified Capabilities

## Impact

- Affected code: `public/analysis.html`
- Affected tests: `tests/analysis_page_test.js`
- No database, API, dependency, or route changes.
