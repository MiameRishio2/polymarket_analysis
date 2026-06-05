---
name: analysis-odds-ui
description: Use when wiring collected bookmaker odds into a local analysis dashboard, especially adding latest-odds panels, match-scoped AJAX endpoints, two-way market display rules, and verification for Rust/Axum or similar web UIs.
---

# Analysis Odds UI

Use this skill when collected bookmaker odds need to be visible in an analysis page without reloading the whole app.

## Core Workflow

1. Confirm odds are persisted with a stable match ID.
2. Add a match-scoped API endpoint for latest odds.
3. Render an initially empty panel in the selected match view.
4. Fetch latest odds from the browser with AJAX when the selected match changes.
5. Display two-way and three-way markets correctly.
6. Verify backend tests and a local page request.

## Backend Shape

Expose a match-scoped latest endpoint:

```text
GET /api/analysis/match/{match_id}/latest
```

Recommended behavior:

- Return a small JSON envelope with rows and an optional error string.
- Limit rows, for example 80 latest bookmaker rows.
- Return a clear error when the analysis database is missing.
- Keep series/history endpoints separate from latest snapshot endpoints.

For Axum-style routing, register latest before broader dynamic routes when route ambiguity is possible:

```text
/api/analysis/match/:match_id/latest
/api/analysis/match/:match_id/series
/api/analysis/match/:match_id
```

## Frontend Pattern

In the selected analysis view:

1. Render a latest odds panel with `data-latest-match-id`.
2. Call `loadSelectedLatestOdds()` after rendering the selected match.
3. Use `fetch('/api/analysis/match/' + encodeURIComponent(matchId) + '/latest')`.
4. Re-check the panel's current `data-latest-match-id` before writing results, so stale responses cannot overwrite a newly selected match.
5. Show loading, empty, error, and populated states.

Keep latest odds separate from the historical odds chart. The latest panel answers "what is available now"; the chart answers "how did it move over time".

## Two-Way Market Display

Esports and many head-to-head markets have no draw. Store draw as `0.0` or `null`, but do not show it as a real price.

Display rule:

```text
show draw only when draw is present and draw > 0
```

Compact odds text should follow the same rule:

```text
H 1.83 / A 2.00
```

not:

```text
H 1.83 / D 0.00 / A 2.00
```

## Verification

Run the project tests after backend and frontend wiring:

```text
cargo test -- --nocapture
```

Then start the web server and verify the analysis page responds:

```text
cargo run -- serve-web
curl -I http://127.0.0.1:{port}/analysis
```

If the sandbox blocks listening on a port, rerun the server command with escalation instead of changing implementation.

## Implementation Notes

- Keep the API response match-scoped; avoid sending all bookmaker odds for all matches to the browser.
- Escape HTML when rendering bookmaker names and values.
- Format numeric odds with the app's existing number formatter.
- Prefer existing storage helpers and UI conventions over inventing a parallel data path.
