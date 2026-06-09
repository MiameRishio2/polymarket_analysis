## Why

The second-level page `/menu/football` can render as unavailable because `/api/menu/football` returns a stale empty cached record instead of refreshing data. The same cached response also exposes the internal cache key `menu_football` as `data.sport`.

## What Changes

- Treat empty second-level cached category data as unusable so the handler attempts a fresh scrape.
- Normalize second-level cached responses so `data.sport` is the public sport slug, such as `football`.
- Keep the existing API shape and page routes unchanged.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- None.

## Impact

- `src/menu/handlers.rs`: cache usability and response normalization.
- `tests/menu_third_level_test.rs`: regression coverage for second-level cached API behavior.
