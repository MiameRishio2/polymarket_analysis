## Context

`category_handler` reads from SQLite using cache key `menu_{sport}`. `Storage::load()` constructs `CategoryData.sport` from the storage key, so cached second-level responses can expose `menu_football` instead of `football`. If the cached row has an empty category array, the handler currently returns it as successful data.

## Goals / Non-Goals

**Goals:**

- Do not return empty second-level cached category data as usable page data.
- Do not expose internal cache keys in public second-level API responses.
- Preserve existing routes and storage schema.

**Non-Goals:**

- Do not change the scraper parser.
- Do not add new public APIs.
- Do not alter third-level cache behavior beyond shared helper reuse if needed.

## Decisions

- Add a small cache usability helper: cached category data is usable only when `categories` is non-empty.
- Normalize `data.sport` in `category_handler` before returning cached data.
- Leave refresh semantics unchanged; refresh always fetches and overwrites cache.

## Risks / Trade-offs

- A truly empty real category page will be retried instead of served from cache. For known second-level sport pages this is preferable because an empty category list is usually a scrape/proxy failure.
