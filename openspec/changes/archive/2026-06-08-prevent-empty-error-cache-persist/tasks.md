## 1. Regression Test

- [x] 1.1 Add a failing test that error/empty category data is not eligible for cache persistence.

## 2. Fix

- [x] 2.1 Prevent category handlers from saving empty/error scrape results.

## 3. Verification

- [x] 3.1 Run focused Rust tests.
- [x] 3.2 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.
- [x] 3.3 Update `session.md`, `change_log.md`, and `test.md`.
