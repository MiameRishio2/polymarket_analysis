---
comet_change: third-level-football-category-page
role: technical-design
canonical_spec: openspec
archived-with: 2026-06-08-third-level-football-category-page
status: final
---

# Third-Level Football Category Page Technical Design

## Approach

Add third-level menu support inside the existing menu module. The browser still uses `public/menu.html` as a static shell, while `src/menu` owns the API, scraping, and cache behavior.

The third-level local page is `/menu/{sport}/{category}`. For `/menu/football/argentina`, the frontend calls `/api/menu/football/argentina`, and the backend scrapes `https://www.oddsportal.com/football/argentina/`.

## Backend Boundaries

`src/menu/scraper.rs` will expose:

- `extract_child_category_slug(href, sport, category)` for deterministic child-link extraction.
- `extract_categories_for_path(html, sport, category)` for testable HTML parsing.
- `fetch_categories_for_path(sport, category)` for network fetching.

The extraction rule accepts exactly one child segment after `/{sport}/{category}/`, such as `/football/argentina/primera-nacional/`, and rejects unrelated paths and deeper paths.

`src/menu/handlers.rs` will add:

- `GET /api/menu/:sport/:category`
- `POST /api/menu/:sport/:category/refresh`
- `GET /menu/:sport/:category`

Third-level cache keys use `menu_{sport}_{category}`. The response reuses `CategoryData`, with `sport` set to `{sport}/{category}` so clients can identify the path.

## Frontend Boundaries

`public/menu.html` will keep `getPageConfig()` as the routing boundary:

- `/menu` -> sports page.
- `/menu/{sport}` -> second-level sport categories.
- `/menu/{sport}/{category}` -> third-level children.

Second-level category rows link locally to `/menu/{sport}/{category}/` while the URL column still displays the original OddsPortal path, such as `/football/argentina/`. Third-level rows link to OddsPortal directly because this change intentionally stops at three levels.

## Testing Strategy

1. Rust scraper unit tests verify third-level child extraction and rejection rules.
2. Rust handler tests verify the third-level route reads cached data from `menu_{sport}_{category}`.
3. A lightweight Node test verifies `public/menu.html` route config for `/menu/football/argentina` and second-level local link derivation.
4. Full verification runs `cargo test --all` and `cargo build`.

## Risks

- OddsPortal may render noisy links with the same prefix. The exact-one-child-segment rule keeps the parser conservative.
- Existing `menu.html` routing can become ambiguous. Parsing will split pathname segments instead of using loose substring checks.
- The project currently contains `#[allow(dead_code)]` in existing files. This change will avoid adding new allowances and will not use allowances to hide warnings.
