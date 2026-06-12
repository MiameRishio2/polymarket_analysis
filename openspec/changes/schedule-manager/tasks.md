## 1. Scheduler Storage and API

- [x] 1.1 Add scheduler models and SQLite table initialization under `src/menu`.
- [x] 1.2 Implement scheduler list, upsert, monitor toggle, and delete storage methods.
- [x] 1.3 Add Axum scheduler handlers and routes using the existing API response envelope.
- [x] 1.4 Export scheduler module paths needed by tests and the application.

## 2. Menu UI Integration

- [x] 2.1 Add event-row scheduler actions on fourth-level competition pages.
- [x] 2.2 Load scheduler state in the menu frontend and reflect scheduled rows.
- [x] 2.3 Render all scheduler entries on the root `/menu` page with monitor controls and delete actions.

## 3. Verification

- [x] 3.1 Add focused Rust tests for scheduler storage behavior.
- [x] 3.2 Add or update frontend behavior tests for scheduler actions and root scheduler display.
- [x] 3.3 Run formatting and the relevant test suite.
