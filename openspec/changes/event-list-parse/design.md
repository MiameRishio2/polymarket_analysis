## Context

The menu module currently models OddsPortal navigation as category lists. `/menu/{sport}/{category}/{league}` calls the fourth-level menu API and renders category-like rows with type, name, and URL. On competition pages such as `https://www.oddsportal.com/football/world/world-championship-2026/`, the page content the user needs is not another taxonomy level; it is a list of matches with scheduled time and downstream event/H2H links.

The project constraints remain unchanged: scraping and parsing stay in `src/`, static rendering stays in `public/`, and communication between them goes through HTTP APIs. The UI must keep the `web_design.md` table/pagination shape.

## Goals / Non-Goals

**Goals:**

- Parse direct match/event rows from OddsPortal competition pages.
- Return event rows with enough structured fields for display and linking: home team, away team, display matchup, start time, OddsPortal URL, and slug/id when available.
- Render fourth-level competition pages as a table of event rows when event rows exist.
- Keep existing category list behavior for pages that expose child categories instead of events.
- Cover parser, API, and frontend behavior with tests before implementation.

**Non-Goals:**

- Parse odds prices or bookmakers for each event in this change.
- Parse every possible OddsPortal sport-specific event layout if it diverges from the football competition markup.
- Follow the H2H link and scrape the destination page.
- Change existing `/api/menu`, `/api/menu/{sport}`, or lower-level category response semantics.

## Decisions

1. Add a dedicated event-row data shape instead of overloading `MenuCategory`.

   Event rows need team fields and start time. Encoding those as pseudo categories would blur the API contract and force the frontend to infer semantics from category fields. A dedicated shape keeps the fourth-level category parser intact and lets the frontend choose an event table only when the API returns event data.

2. Prefer a separate event-list API path or typed payload for fourth-level pages.

   The implementation should keep category APIs backward compatible. The practical route can be either `GET /api/events/{sport}/{category}/{league}` or a typed response from the existing fourth-level handler, but it must not remove fields existing callers expect. The implementation plan will choose the least invasive option after reading current handler structure.

3. Use parser-first extraction with strict URL filtering.

   The scraper should only accept links belonging to the target competition page or OddsPortal H2H/event destinations. It should extract visible team labels and nearby time text from the HTML around the row, then normalize `Mexico - South Africa`, `Mexico vs South Africa`, or split team nodes into the display label `Mexico VS South Africa`.

4. Frontend rendering remains table-based.

   The event table should use the existing `stats-bar`, `pagination`, and `list-table` components. Event pages should show columns for matchup, start time, and link. The link target is the parsed OddsPortal URL, opened as an external destination.

## Risks / Trade-offs

- OddsPortal markup is dynamic or obfuscated -> Mitigation: write parser tests from representative static snippets and keep extraction tolerant around class names, relying primarily on href structure and text content.
- Some competition pages may contain both event links and child category links -> Mitigation: event rows take precedence only when valid event rows are parsed; otherwise fall back to existing category rendering.
- Start time may be relative, localized, or missing -> Mitigation: preserve the display text exactly when an absolute timestamp cannot be normalized, and render an empty/unknown value only when no nearby time exists.
- API shape ambiguity can break frontend callers -> Mitigation: add explicit tests for existing category routes and new event rendering before changing implementation.

## Migration Plan

1. Add tests for event extraction, API response shape, and frontend event-row rendering.
2. Implement the smallest compatible backend API/data model change.
3. Update `public/menu.html` to render event tables only for event payloads.
4. Update documentation and session/change logs.
5. Verify with `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.

Rollback is straightforward: remove the new event API/rendering path and the fourth-level page falls back to existing category-list behavior.

## Open Questions

- The implementation phase should verify the current OddsPortal HTML shape from cached or live data before finalizing the parser selectors.
- If the page exposes multiple market tabs under a match, this change will use the event/H2H destination link only and leave market-specific parsing for a later change.
