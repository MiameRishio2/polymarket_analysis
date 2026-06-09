# Comet Design Handoff

- Change: third-level-football-category-page
- Phase: design
- Mode: compact
- Context hash: 9be834a3aa62a3bf1f13d35ad58f155a022a0dceb5b6c02dfdd48333029a839a

Generated-by: comet-handoff.sh

OpenSpec remains the canonical capability spec. This handoff is a deterministic, source-traceable context pack, not an agent-authored summary.

## openspec/changes/third-level-football-category-page/proposal.md

- Source: openspec/changes/third-level-football-category-page/proposal.md
- Lines: 1-28
- SHA256: 454ebcbaa8a3ac32345de620f2b7dca7948bf54a265eccab5ce985757ea3b9e8

```md
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
```

## openspec/changes/third-level-football-category-page/design.md

- Source: openspec/changes/third-level-football-category-page/design.md
- Lines: 1-52
- SHA256: 7d92d6e577b70d94465d9c8983a4e4fe70efc6ee1393e8f7c4cb462855a29a32

```md
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
```

## openspec/changes/third-level-football-category-page/tasks.md

- Source: openspec/changes/third-level-football-category-page/tasks.md
- Lines: 1-18
- SHA256: 6ba5812fe5c771e30076715b1bd40958edea6db977b9bf0724eeb9cbb8d2fd7a

```md
## 1. Tests

- [ ] 1.1 Add scraper tests for extracting `/football/argentina/primera-nacional/` from third-level HTML.
- [ ] 1.2 Add handler/storage tests for `/api/menu/:sport/:category` cache key behavior.
- [ ] 1.3 Add frontend routing test coverage for `/menu/{sport}/{category}` URL derivation.

## 2. Implementation

- [ ] 2.1 Implement path-aware third-level scraping in `src/menu/scraper.rs`.
- [ ] 2.2 Add third-level API and refresh handlers in `src/menu/handlers.rs`.
- [ ] 2.3 Update `public/menu.html` to route second-level rows to `/menu/{sport}/{category}/` and call third-level APIs.
- [ ] 2.4 Update `web_design.md` with third-level page and API rules.

## 3. Verification and State

- [ ] 3.1 Run `cargo test --all`.
- [ ] 3.2 Run `cargo build`.
- [ ] 3.3 Update `test.md`, `session.md`, and `change_log.md`.
```

## openspec/changes/third-level-football-category-page/specs/menu-third-level-page/spec.md

- Source: openspec/changes/third-level-football-category-page/specs/menu-third-level-page/spec.md
- Lines: 1-37
- SHA256: bc9d36626dabab095c849d58407e5ae36243ed4c822038ebcc30949c9b35938e

```md
## ADDED Requirements

### Requirement: Third-level menu page
The system SHALL serve a local third-level menu page at `/menu/{sport}/{category}` using the existing static menu page shell.

#### Scenario: Open football Argentina page
- **WHEN** the browser requests `/menu/football/argentina`
- **THEN** the system returns the menu page HTML

### Requirement: Third-level menu API
The system SHALL expose third-level category data at `/api/menu/{sport}/{category}` and refresh it at `/api/menu/{sport}/{category}/refresh`.

#### Scenario: Fetch football Argentina children
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response contains `ok: true` and `data.sport` identifies the third-level path

#### Scenario: Refresh football Argentina children
- **WHEN** the client posts to `/api/menu/football/argentina/refresh`
- **THEN** the system fetches data from OddsPortal and stores it under a third-level cache key

### Requirement: Third-level child link extraction
The scraper SHALL extract child links with exactly one segment under `/{sport}/{category}/`, such as `/football/argentina/primera-nacional/`.

#### Scenario: Include direct league child
- **WHEN** the OddsPortal HTML contains `href="/football/argentina/primera-nacional/"`
- **THEN** the scraper returns a row with slug `primera-nacional` and URL `/football/argentina/primera-nacional/`

#### Scenario: Exclude deeper or unrelated links
- **WHEN** the OddsPortal HTML contains paths outside `/{sport}/{category}/{child}/` or with extra path segments
- **THEN** those paths are excluded from the third-level menu data

### Requirement: Third-level frontend links
The frontend SHALL convert second-level OddsPortal category URLs to local third-level page links while displaying the original OddsPortal URL in the URL column.

#### Scenario: Render Argentina row
- **WHEN** a second-level row has URL `/football/argentina/`
- **THEN** the row link targets `/menu/football/argentina/`
```

