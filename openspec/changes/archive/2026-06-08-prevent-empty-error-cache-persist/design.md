# Design

Add a small handler-level predicate for cache persistence. Reuse it anywhere a second-level or third-level category refresh result is about to be saved.

The scraper continues to download the page body once and parse that local HTML. This hotfix avoids making a failed download/parser result permanent.
