# Design

## Approach

Monitoring remains owned by the existing `scheduled_matches` table and `/api/scheduler/:id/monitoring` endpoint. When the endpoint receives `monitoring_started: true`, the handler first toggles the scheduler state, then runs a best-effort odds collection pass for that match.

Odds snapshots are stored separately from scheduled matches so collection errors do not corrupt scheduler state. Each snapshot records source probabilities, source URLs, status/error fields, and the capture time.

## OddsPortal Collection

OddsPortal collection follows the `oddsportal-ajax-odds` skill:

1. Fetch the public match page.
2. Discover `/match-event/...dat` feed URLs from the page or derive a likely feed URL from the h2h URL/hash.
3. Decode the encrypted feed envelope with the known AES/PBKDF2 key pairs.
4. Parse `d.oddsdata.back.*.odds` bookmaker rows.
5. Treat `[home, draw, away]` as 1/X/2. Convert decimal odds to implied probabilities, ignore draw, and normalize home vs away.

If live feed discovery or decoding fails, the snapshot records an OddsPortal error instead of blocking monitoring.

## Polymarket Collection

Polymarket collection is best-effort. The linked Polymarket URL is normalized to a slug and queried through Gamma API endpoints. The collector extracts two-outcome prices when available and normalizes them into home/away probabilities. If the URL is a search fallback or the API shape is not usable, the snapshot records a Polymarket error.

## Analysis UI

`/analysis` fetches a new analysis API and renders monitored match cards. Each card shows match metadata, source status, home/away percentages, and a small horizontal comparison bar for Polymarket vs OddsPortal.

## Failure Handling

Starting monitoring succeeds even if one or both odds sources fail. Errors are saved with the snapshot so the analysis page can show what is missing.
