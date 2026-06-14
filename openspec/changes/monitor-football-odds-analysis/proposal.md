# monitor-football-odds-analysis

## Problem

The World Cup menu page can save matches to the scheduler and toggle monitoring, but starting monitoring does not yet calculate comparable win probabilities from Polymarket and OddsPortal. The `/analysis` page is still a placeholder, so monitored matches cannot be reviewed after collection starts.

## Goals

- Start odds collection when a scheduled match is switched to monitoring.
- Use the OddsPortal AJAX `.dat` match-event feed workflow to collect 1/X/2 bookmaker odds.
- Convert OddsPortal 1/X/2 odds into two-way home/away win probabilities by ignoring draw and normalizing home vs away only.
- Collect best-effort Polymarket probabilities from the linked Polymarket event/market data.
- Persist the latest comparison snapshot per monitored match.
- Show monitored match odds comparisons on `/analysis` as compact chart-style cards.

## Non-Goals

- Continuous background polling or cron scheduling.
- Arbitrage execution.
- Long-term time-series charting.
- Replacing the existing scheduler match discovery flow.

## Scope

This change is limited to scheduler-triggered collection, latest snapshot storage/API, OddsPortal/Polymarket probability calculation, and the `/analysis` dashboard display.
