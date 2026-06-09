## 1. Regression Tests

- [x] 1.1 Add a failing test that cached `/api/menu/football` returns public sport `football`, not `menu_football`.
- [x] 1.2 Add a failing test that empty cached category data is not considered usable.

## 2. Fix

- [x] 2.1 Normalize cached second-level `CategoryData.sport` before returning.
- [x] 2.2 Treat empty cached second-level categories as stale and fetch fresh data.

## 3. Verification

- [x] 3.1 Run focused regression tests.
- [x] 3.2 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.
- [x] 3.3 Update `session.md` and `change_log.md`.
