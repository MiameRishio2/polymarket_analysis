## Why

The menu flow currently stops at the second level, so a link such as `/football/argentina/` cannot expose its child leagues inside the local UI. Adding a third-level page lets the user continue from country pages to league pages, starting with `/football/argentina/primera-nacional/`.

## What Changes

- Add a third-level menu page pattern for `/menu/{sport}/{category}` that displays child links from OddsPortal paths such as `/football/argentina/primera-nacional/`.
- Add menu API support for fetching and refreshing third-level category children.
- Update `web_design.md` to define the third-level page, layout, API contract, and scraping rule.
- Keep frontend code in `public/` and data fetching/parsing/cache logic in `src/menu/`.

## Capabilities

### New Capabilities

- `menu-third-level-page`: Display and fetch third-level menu entries beneath a sport category, such as football Argentina leagues.

### Modified Capabilities

- None.

## Impact

- `web_design.md`: documents the third-level page and link format.
- `public/menu.html`: parses `/menu/{sport}/{category}`, calls the third-level API, and renders rows using the existing table layout.
- `src/menu/handlers.rs`: adds third-level API and page routes while preserving existing `/api/menu` and `/api/menu/:sport` behavior.
- `src/menu/scraper.rs`: adds path-aware child scraping for `/football/argentina/`.
- `tests/`: adds coverage for third-level URL extraction, API/cache key behavior, and page config routing.
