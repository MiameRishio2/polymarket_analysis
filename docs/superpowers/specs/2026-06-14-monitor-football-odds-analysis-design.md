---
comet_change: monitor-football-odds-analysis
role: technical-design
canonical_spec: openspec
---

# Monitor Football Odds Analysis Design

## Summary

Starting monitoring for a scheduled football match will run a best-effort odds collection pass and persist a latest comparison snapshot. `/analysis` will render those snapshots as compact visual cards.

## Backend

- Extend scheduler storage with a separate latest snapshot table.
- Add an odds analysis module for source collection and probability conversion.
- Use the OddsPortal AJAX `.dat` workflow as the primary OddsPortal data path.
- Use Gamma API lookup from the linked Polymarket slug as the Polymarket data path.
- Add `GET /api/analysis/odds` for the dashboard.

## Probability Rules

OddsPortal football rows may contain `[home, draw, away]`. The collector converts decimal odds to implied probabilities, discards draw, and normalizes home vs away only. Two-outcome rows are normalized directly.

Polymarket prices are normalized to home/away probabilities when two usable outcomes can be associated with the scheduled match teams.

## UI

The analysis page uses the existing static HTML approach. It fetches `/api/analysis/odds`, then renders one card per monitored match with source status, percentages, source links, and a simple comparison bar.

## Testing

Tests cover:

- OddsPortal 1/X/2 to two-way probability conversion.
- Parsing a minimal OddsPortal `oddsdata.back` JSON fixture.
- Snapshot persistence and latest listing.
- Analysis API response shape through the Axum router.
