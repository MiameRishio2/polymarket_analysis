## 1. Tests

- [x] 1.1 Add scraper tests for extracting `/football/argentina/primera-nacional/` from third-level HTML.
- [x] 1.2 Add handler/storage tests for `/api/menu/:sport/:category` cache key behavior.
- [x] 1.3 Add frontend routing test coverage for `/menu/{sport}/{category}` URL derivation.

## 2. Implementation

- [x] 2.1 Implement path-aware third-level scraping in `src/menu/scraper.rs`.
- [x] 2.2 Add third-level API and refresh handlers in `src/menu/handlers.rs`.
- [x] 2.3 Update `public/menu.html` to route second-level rows to `/menu/{sport}/{category}/` and call third-level APIs.
- [x] 2.4 Update `web_design.md` with third-level page and API rules.

## 3. Verification and State

- [x] 3.1 Run `cargo test --all`.
- [x] 3.2 Run `cargo build`.
- [x] 3.3 Update `test.md`, `session.md`, and `change_log.md`.
