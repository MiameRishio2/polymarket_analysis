## 1. SQLite Storage

- [x] 1.1 Add `refreshed_at` to new `category_cache` table creation with default `1970-01-01T00:00:00Z`
- [x] 1.2 Add an idempotent migration for existing databases missing `refreshed_at`
- [x] 1.3 Update category and raw JSON save/load paths to persist and return `refreshed_at`

## 2. API Models

- [x] 2.1 Add `refreshed_at` to `CategoryData` with epoch default handling
- [x] 2.2 Add `refreshed_at` to `EventData` with epoch default handling
- [x] 2.3 Ensure all menu and event handlers return refresh timestamps for cache hits, fresh fetches, and refresh responses

## 3. Menu UI

- [x] 3.1 Track the active page refresh timestamp in `public/menu.html`
- [x] 3.2 Render the refresh timestamp on root, sport, third-level, and fourth-level pages
- [x] 3.3 Use event response timestamps when fourth-level pages display event data

## 4. Verification

- [x] 4.1 Add or update Rust storage/API tests for migration default and refreshed timestamp persistence
- [x] 4.2 Add or update JavaScript menu page tests for timestamp rendering across page levels
- [x] 4.3 Run targeted Rust and JavaScript tests covering the refresh timestamp change
