## Context

Event rows are parsed by `src/menu/events.rs` and returned from `/api/events/{sport}/{category}/{league}`. The frontend in `public/menu.html` uses these rows on fourth-level pages before falling back to category rendering.

The current `EventRow` contract has a single `url`, which is the OddsPortal destination. The frontend wraps the whole row in that URL and displays the URL text in the link column. That makes adding a second destination awkward and hides whether Polymarket matching succeeded.

## Approach Options

1. **Recommended: enrich event rows with explicit link fields.**
   Keep `url` as the OddsPortal URL for compatibility, add `polymarket_url: Option<String>`, and render both destinations as buttons. This is the smallest API extension and maps cleanly to the existing event API.

2. **Return a nested `links` object.**
   Replace `url` with `{ oddsportal, polymarket }`. This is cleaner long term, but it breaks current tests/callers that expect `url`.

3. **Resolve Polymarket entirely in the browser.**
   Keep the backend unchanged and generate/search links from JavaScript. This avoids Rust changes but moves network/domain logic into the static page and makes testing and caching worse.

The implementation will use option 1.

## Architecture Decisions

1. Event parsing remains the source of team names, date text, and OddsPortal URL.
2. Backend enrichment derives Polymarket slug candidates from event fields and the current competition context.
3. The event API remains `/api/events/{sport}/{category}/{league}` and continues to return event rows. Existing `url` remains the OddsPortal URL.
4. The frontend treats event rows as non-row-link table rows and renders explicit buttons in the "链接" column.
5. A missing Polymarket URL is a first-class state and renders as a disabled button.

## Polymarket Matching

For the user-provided World Cup example, the slug candidate format is:

```text
fifwc-{home_code}-{away_code}-{yyyy-mm-dd}
```

For Mexico vs South Africa on 2026-06-11, this yields:

```text
fifwc-mex-rsa-2026-06-11
```

The search layer should ask Polymarket for an exact slug match. If it returns a market/event with a usable slug, the UI link becomes:

```text
https://polymarket.com/sports/world-cup/{slug}
```

If no match is found, `polymarket_url` stays absent.

## Data Flow

```text
OddsPortal competition page
  -> extract EventRow { teams, start_time, oddsportal url }
  -> derive Polymarket slug candidates
  -> search Polymarket by slug
  -> EventRow { url, polymarket_url? }
  -> public/menu.html event table
  -> OddsPortal / Polymarket buttons under "链接"
```

## Error Handling

- Polymarket lookup failures must not prevent OddsPortal event rows from rendering.
- Lookup timeouts, HTTP errors, malformed JSON, or no exact slug match all produce `polymarket_url: null` / omitted.
- The Polymarket button is disabled when the field is absent.

## Testing

- Rust tests for World Cup slug candidate generation from Mexico vs South Africa.
- Rust tests that a seeded cached event row can omit `polymarket_url` and still serialize/serve correctly.
- Frontend Node tests that event rows render `OddsPortal` and `Polymarket` buttons, and the Polymarket button is disabled when no URL exists.
