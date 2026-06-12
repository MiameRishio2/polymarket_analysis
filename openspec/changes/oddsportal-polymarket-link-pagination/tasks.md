## 1. OpenSpec Repair

- [x] 1.1 Recreate missing change metadata and proposal/design/tasks artifacts.
- [x] 1.2 Add delta spec requirements for event external market links.

## 2. Backend

- [x] 2.1 Extend event row serialization with optional Polymarket URL while preserving existing `url`.
- [x] 2.2 Add slug candidate generation for World Cup event rows, including Mexico vs South Africa -> `fifwc-mex-rsa-2026-06-11`.
- [x] 2.3 Add Polymarket by-slug lookup that returns an exact matching sports URL or `None`.
- [x] 2.4 Keep Polymarket lookup failures non-fatal for event list rendering.

## 3. Frontend

- [x] 3.1 Render event rows with explicit OddsPortal and Polymarket buttons under the existing "链接" column.
- [x] 3.2 Disable the Polymarket button when `polymarket_url` is missing.
- [x] 3.3 Preserve existing event pagination and category fallback behavior.

## 4. Verification

- [x] 4.1 Add Rust tests for slug generation and event API compatibility.
- [x] 4.2 Add Node tests for button rendering and disabled Polymarket fallback.
- [x] 4.3 Run `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.
