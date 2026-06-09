# Prevent Empty Error Cache Persist

## Problem

When `/api/menu/:sport` sees an empty stale cache and the scrape fails, the handler builds an error `CategoryData` with zero categories and saves it back to storage. The production log shows exactly this sequence for `menu_football`: refresh fails with `error decoding response body`, then storage saves 0 categories.

## Root Cause

The handler persists every scrape result, including `source = "error"` and empty category lists. A transient network/body failure therefore poisons the cache and makes later page loads fast but empty.

## Fix Goal

Only persist category refresh results when they contain real categories. Failed or empty scrape results should be returned to the caller for visibility but must not overwrite the cache.
