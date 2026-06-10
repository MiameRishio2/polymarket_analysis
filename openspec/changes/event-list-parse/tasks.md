## 1. Test Coverage

- [x] 1.1 Add Rust scraper test for extracting `Mexico VS South Africa`, start time, and OddsPortal H2H link from representative competition HTML.
- [x] 1.2 Add Rust API/cache tests proving event-list data uses an event-specific cache key and does not overwrite category menu cache.
- [x] 1.3 Add frontend Node tests for fourth-level event payload rendering, event links, and 10-row pagination.

## 2. Backend Implementation

- [x] 2.1 Add event row models and serialization shape for event-list API responses.
- [x] 2.2 Implement OddsPortal competition event-row extraction with filtering for unrelated/navigation links.
- [x] 2.3 Add event-list handler/routing and storage integration while preserving existing category API behavior.

## 3. Frontend Implementation

- [x] 3.1 Update `public/menu.html` page configuration to request event-list data for fourth-level competition pages before falling back to category data.
- [x] 3.2 Render event rows in the table with matchup, start time, and external OddsPortal link while preserving existing category table rendering.
- [x] 3.3 Keep stats and pagination behavior aligned with `web_design.md` for event rows.

## 4. Documentation and Verification

- [x] 4.1 Update `web_design.md`, `architect.md`, and `test.md` with event-list API, layout, and test coverage.
- [x] 4.2 Update `session.md` and `change_log.md` with task status and verification results.
- [x] 4.3 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js` successfully.
