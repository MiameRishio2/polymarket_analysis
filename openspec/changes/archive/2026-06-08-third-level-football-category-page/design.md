## Context

The current menu feature uses one static page, `public/menu.html`, for both `/menu` and `/menu/{sport}`. The backend owns data fetching through `src/menu`, with `handlers.rs` exposing HTTP APIs, `scraper.rs` parsing OddsPortal HTML, and `storage.rs` caching `CategoryData`.

The requested third-level page is the child of a second-level category. For example, the second-level row with `href="/football/argentina/"` should lead locally to `/menu/football/argentina`, whose rows include OddsPortal child paths such as `/football/argentina/primera-nacional/`.

## Goals / Non-Goals

**Goals:**

- Support `/menu/{sport}/{category}` as a third-level local page.
- Fetch third-level data from OddsPortal path `/{sport}/{category}/`.
- Render third-level rows as a table with type, name, and URL columns.
- Preserve clean layering: `public/` handles display and browser routing; `src/menu` handles API, scraping, and storage.
- Preserve existing `/menu`, `/menu/{sport}`, `/api/menu`, and `/api/menu/{sport}` behavior.

**Non-Goals:**

- Do not add arbitrary unlimited-depth navigation.
- Do not add new dependencies or database schema changes.
- Do not change the visual system beyond the documented table page behavior.

## Decisions

1. Add a path-aware scraper helper instead of overloading the existing second-level parser.

   The existing parser intentionally keeps only one path segment after the sport. Third-level scraping needs one additional segment after a category, so a dedicated helper keeps the two matching rules clear and testable.

2. Use `/api/menu/:sport/:category` and `/api/menu/:sport/:category/refresh`.

   This mirrors the existing `/api/menu/:sport` shape and keeps cross-layer communication HTTP-only. Frontend code derives API URLs from the browser path.

3. Cache third-level data under `menu_{sport}_{category}`.

   Existing second-level data uses `menu_{sport}`. Adding the category to the key prevents collision while avoiding a schema migration.

4. Reuse `CategoryData` for third-level rows.

   The rows still have `slug`, `name`, `url`, and `category_type`. Third-level league entries will normally be typed as `league`, with fallback `other` for unknown paths.

## Risks / Trade-offs

- OddsPortal pages can include unrelated links with the same prefix -> the scraper will only accept paths with exactly one child segment after `/{sport}/{category}/`.
- Some child labels may be league names rather than countries -> third-level classification will prefer `league` when the entry is not a known country or region.
- Static page reuse increases `menu.html` routing complexity -> keep routing logic isolated in `getPageConfig()` and add focused tests for URL derivation.

## Migration Plan

1. Add tests for third-level scraper extraction and handler/cache behavior.
2. Implement third-level scraper/API/page routing.
3. Update `web_design.md`, `test.md`, `session.md`, and `change_log.md`.
4. Run `cargo test --all` and `cargo build`.
