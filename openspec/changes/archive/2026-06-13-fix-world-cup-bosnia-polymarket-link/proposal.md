## Why

World Cup event rows can show a disabled Polymarket link even when the matching Polymarket market exists. Canada VS Bosnia & Herzegovina exposed one missing country-code alias, and USA VS Paraguay exposed the broader issue: the enrichment path relies on one hand-built slug shape per match, while Polymarket may publish slugs using either team codes or normalized country names.

## What Changes

- Generate multiple World Cup slug candidates from team codes and normalized country-name aliases.
- Generate adjacent date variants to tolerate display-timezone date boundaries.
- Add a Gamma keyset World Cup event fallback that accepts a result only when both teams and the event date match.
- Make generated slug fallback use silent HTTP lookups instead of noisy CLOB client slug calls.
- Add regression tests for Bosnia & Herzegovina aliases, USA VS Paraguay country-name/date candidates, search-result selection, and keyset event selection.
- Keep existing event API and scheduler contracts unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `menu-third-level-page`: World Cup event rows with supported teams should generate matching Polymarket public URL candidates across alias/date variants and safely fall back to validated search results.

## Impact

- Affected code: `src/menu/events.rs`
- Affected tests: `tests/menu_third_level_test.rs`
- No API, database, dependency, or frontend contract changes.
