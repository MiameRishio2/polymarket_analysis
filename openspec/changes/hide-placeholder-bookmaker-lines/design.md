## Overview

The root cause is in the analysis page's bookmaker label normalization. `bookmakerName(value, fallbackId)` always returns a display string, falling back to `Bookmaker {id}` when no readable name exists. Historical rows may also already contain `Bookmaker {id}` from older collected snapshots, so the renderer needs to reject both newly generated and persisted placeholders.

## Solution

- Treat a readable bookmaker name as one that is non-empty, not numeric-only, and not matching `Bookmaker <number>`.
- Keep the existing small ID map for known bookmaker IDs so rows with missing names but known IDs can still render as real names.
- Extend the ID map with bookmaker IDs confirmed from the decoded OddsPortal `bs` betslip metadata for the monitored Germany vs Curacao feed, including `500 -> 22bet`, `911 -> BetFury`, `575 -> BetInAsia`, and `909 -> Bets.io`.
- Return `null` for unknown IDs instead of generating `Bookmaker {id}`.
- In `buildSeries`, skip rows whose normalized bookmaker name is `null`.

## Root Cause Check

After the change, searching `public/analysis.html` should show no generated `Bookmaker ${id}` fallback in the history path. The regression test should show that placeholder rows produce no series.

## Non-Goals

- Do not change OddsPortal feed parsing or stored snapshot format.
- Do not backfill existing history rows.
