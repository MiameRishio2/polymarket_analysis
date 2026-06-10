---
comet_change: event-list-parse
role: technical-design
canonical_spec: openspec
---

# Event List Parse Technical Design

## Context

The menu flow currently treats OddsPortal pages as nested category lists. That works for `/menu`, `/menu/{sport}`, `/menu/{sport}/{category}`, and category-like fourth-level pages. It does not fit `https://www.oddsportal.com/football/world/world-championship-2026/`, where the useful rows are actual matches with a scheduled start time and a downstream OddsPortal H2H/event link.

The existing Rust files are also near the project limit: `src/menu/handlers.rs` and `src/menu/scraper.rs` are both around 480 lines. The event-list work needs to avoid adding significant code to those files.

## Technical Approach

Add a dedicated event-list path instead of overloading `CategoryData`.

New backend data shape:

```rust
pub struct EventRow {
    pub slug: String,
    pub home_team: String,
    pub away_team: String,
    pub matchup: String,
    pub start_time: String,
    pub url: String,
}

pub struct EventData {
    pub sport: String,
    pub events: Vec<EventRow>,
    pub last_updated: String,
    pub source: String,
}
```

Add `src/menu/events.rs` for event-specific fetching, parsing, cache-key helpers, and handlers. `src/menu/handlers.rs` should only wire the routes, keeping it under the 500-line rule as much as possible. If route wiring still pushes it over the limit, move event route construction into an event router helper.

New API:

- `GET /api/events/:sport/:category/:league`
- `POST /api/events/:sport/:category/:league/refresh`

Event cache keys use the prefix `events_`, for example `events_football_world_world-championship-2026`. Existing category cache keys such as `menu_football_world_world-championship-2026` remain unchanged and incompatible JSON is never written there.

## Parser Design

The parser should primarily identify event rows through OddsPortal event/H2H links, especially links under `/football/h2h/...` or event anchors embedded in competition rows. For the user’s target case, it must extract:

- `home_team`: `Mexico`
- `away_team`: `South Africa`
- `matchup`: `Mexico VS South Africa`
- `start_time`: the visible row time text from the page
- `url`: `/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2` or the absolute OddsPortal equivalent used by the API/frontend

Team extraction should prefer visible row text when available, falling back to H2H slug decoding. Slug decoding removes trailing OddsPortal IDs after the final hyphenated token segment when the segment contains an ID-like suffix, then title-cases hyphenated words. The parser should deduplicate by URL or by `(home_team, away_team, start_time)`.

Start time should be preserved as display text. This avoids incorrect timezone conversion while still showing the user the exact scheduling text visible on OddsPortal.

## Frontend Design

Fourth-level pages keep the same URL: `/menu/football/world/world-championship-2026`.

On fourth-level pages, `public/menu.html` should:

1. Request `/api/events/{sport}/{category}/{league}` first.
2. If the response contains `events.length > 0`, set page mode to event rows and render an event table.
3. If the event response is empty or unavailable, fall back to the existing `/api/menu/{sport}/{category}/{league}` category request.

Event table columns:

- `比赛`: `Mexico VS South Africa`
- `开始时间`: display start time
- `链接`: parsed OddsPortal URL

The table must continue using `list-table`, `stats-bar`, and `pagination`, with 10 items per page. Event row links should open the OddsPortal URL externally.

## Testing Strategy

Follow TDD strictly.

1. Rust parser red test: representative HTML with a Mexico/South Africa H2H link and nearby start time must produce one `EventRow` with the expected matchup, teams, start time, and URL.
2. Rust API/cache red test: seeded event cache at `events_football_world_world-championship-2026` is returned by `/api/events/football/world/world-championship-2026`, while seeded category cache at `menu_...` remains independent.
3. Frontend red tests: fourth-level page config exposes an event API URL, event payloads render event rows, pagination limits to 10 rows, and category fallback still renders existing category rows.
4. Final verification: `cargo test --all`, `cargo build`, and `node tests/menu_page_config_test.js`.

## Risks and Mitigations

- OddsPortal markup changes: keep parsing centered on href patterns and visible row text rather than brittle class names.
- Mixed event/category pages: event rows only win when non-empty; otherwise existing category fallback stays intact.
- File-size limits: keep event logic in `src/menu/events.rs`; only minimal route wiring belongs in existing files.
- API ambiguity: separate `/api/events` endpoint makes payload shape explicit and avoids breaking menu callers.

## Spec Patches

No additional OpenSpec spec patches are needed beyond the existing `event-list-page` and `menu-third-level-page` delta specs.
