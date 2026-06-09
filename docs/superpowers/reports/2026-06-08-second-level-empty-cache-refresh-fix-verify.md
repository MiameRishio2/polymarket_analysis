---
change: second-level-empty-cache-refresh-fix
result: pass
verified-at: 2026-06-08T22:45:00+08:00
branch-status: kept-as-is
---

# Second-Level Empty Cache Refresh Fix Verification

## Summary

| Check | Result | Evidence |
|-------|--------|----------|
| Root cause reproduced | PASS | `curl http://127.0.0.1:23333/api/menu/football` returned empty cached `menu_football` data from the old running process |
| Remote runtime checked | PASS | `curl http://10.32.50.201:23333/menu/football` returned `Connection refused`, meaning no listener on that port from this environment |
| Regression tests pass | PASS | `empty_cached_category_data_is_not_usable` and `test_second_level_api_normalizes_cached_sport_key` pass |
| Full Rust tests pass | PASS | `cargo test --all` exit 0 |
| Build passes | PASS | `cargo build` exit 0 |
| Frontend route tests pass | PASS | `node tests/menu_page_config_test.js` exit 0 |
| Security check | PASS | No secrets, unsafe Rust, shell execution, or credential handling added |

## Fix

- `category_handler` now returns cached second-level data only when `categories` is non-empty.
- Cached second-level data is normalized before response so `data.sport` is `football`, not `menu_football`.
- Empty cached second-level data triggers a fresh scrape and cache overwrite.

## Follow-Up

The remote service at `10.32.50.201:23333` must be started or restarted. After the service is listening, refresh `/api/menu/football/refresh` or clear the stale `menu_football` cache.
