## Why

The fourth-level menu page can already parse OddsPortal competition event rows, but the "链接" column still behaves like a plain text URL / whole-row link. For a match such as Mexico vs South Africa, the useful workflow is to open either the OddsPortal H2H page or the matching Polymarket market.

The user wants the connected links to remain under the existing link column, but as explicit buttons: one for OddsPortal and one for Polymarket. The Polymarket link should be discovered by slug search rather than hard-coded from the menu page.

## What Changes

- Add a Polymarket URL field to event rows.
- Build deterministic Polymarket slug candidates from OddsPortal event data and search Polymarket by slug using the local Rust integration.
- For World Cup events, support the Polymarket sports path shape shown by the user, for example `fifwc-mex-rsa-2026-06-11`.
- Render the event table "链接" column as two buttons:
  - `OddsPortal`: enabled when the event has an OddsPortal URL.
  - `Polymarket`: enabled only when slug search finds a matching Polymarket market URL.
- When no Polymarket match is found, show a disabled Polymarket button instead of guessing or linking to a broad listing page.

## Capabilities

### Modified Capabilities

- `event-external-market-links`: Event rows expose structured external links for OddsPortal and Polymarket.
- `menu-third-level-page`: Fourth-level event tables render link buttons in the existing link column.

## Impact

- `src/menu/events.rs`: Enrich event rows with optional Polymarket links and slug search helpers.
- `src/menu/handlers.rs`: Keep event API routes stable while returning the expanded event row shape.
- `public/menu.html`: Replace event URL text with two external-link buttons.
- `tests/menu_third_level_test.rs`: Cover Polymarket slug candidate generation / disabled fallback in serialized event rows.
- `tests/menu_page_config_test.js`: Cover event link button rendering.

## Non-Goals

- Do not scrape odds prices or bookmaker lines.
- Do not trade, sign, or place orders through Polymarket CLOB.
- Do not show a broad Polymarket search/listing link when no exact market is found.
- Do not redesign the menu page outside the event table link column.
