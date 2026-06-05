---
name: oddsportal-ajax-odds
description: Use when implementing or repairing fast OddsPortal odds collection through AJAX/dat feeds, including match-event URL discovery, encrypted feed decoding, gzip handling, and oddsdata JSON parsing for two-way esports or three-way sports markets.
---

# OddsPortal AJAX Odds

Use this skill when a task needs fast OddsPortal odds collection without scraping rendered tables.

## Core Workflow

1. Start from the public OddsPortal match URL.
2. Prefer the frontend AJAX/dat feed over browser-rendered HTML.
3. Decode the feed exactly as the current frontend bundle does.
4. Parse structured JSON odds first, then fall back to legacy HTML parsing only if needed.
5. Verify with a saved real feed fixture and at least one targeted unit test.

## Feed URL Pattern

For match pages with a hash like:

```text
https://www.oddsportal.com/esports/h2h/home-slug/eventId/away-slug/#versionId:home-away;scope
```

OddsPortal may request a feed shaped like:

```text
/match-event/1-{sportId}-{eventId}-{betId}-{scopeId}-{bookiehash}.dat?geo={geo}&lang={lang}&_={timestamp}
```

Observed Dota 2 example:

```text
/match-event/1-36-MR57uPVs-3-2-yj1dd.dat?_=...
```

Useful constants from the observed flow:

```text
sportId=36       esports
betId=3          home-away market
scopeId=2        home-away scope from hash
bookiehash=yj1dd default aggregate hash observed in OddsPortal bundle output
```

If guessing the URL is brittle, fetch the page or `/ajax-user-data/h2h/...` script and extract embedded `"url"` values containing `/match-event/`. Absolutize relative URLs and append a timestamp for trailing `_=` query strings.

## Feed Decoding

The response body is base64 text containing an encrypted payload and IV. The decoded envelope is:

```text
{ciphertextBase64}:{ivBase64}
```

Decode process:

1. Base64-decode the envelope.
2. Split on `:`.
3. Base64-decode ciphertext and IV.
4. Derive AES key with PBKDF2-HMAC-SHA256, 1000 iterations.
5. Decrypt AES-CBC.
6. Try PKCS#7 padding first, then no-padding as fallback.
7. If decrypted bytes start with gzip magic `1f 8b`, gzip-decompress them.
8. Convert UTF-8 and trim trailing bytes after the final `}` if OddsPortal appends noise.

Known frontend key material can rotate. Try current and recently known pairs in order:

```text
password: J*8sQ!p$7aD_fR2yW@gHn*3bVp#sAdLd_k
salt:     5b9a8f2c3e6d1a4b7c8e9d0f1a2b3c4d

password: %RtR8AB&nWsh=AQC+v!=pgAe@dSQG3kQ
salt:     orieC_jQQWRmhkPvR6u2kzXeTube6aYupiOddsPortal
```

When decoding breaks, inspect the latest OddsPortal JS bundle and search for `requestBasePreMatch`, `.dat`, `PBKDF2`, `AES`, `CryptoJS`, `salt`, or password-like constants.

## JSON Parsing

Prefer parsing:

```text
d.oddsdata.back.{market}.odds
```

Each bookmaker ID maps to an array of odds. For two-outcome esports markets:

```text
[home, away] -> home=odds[0], draw=0.0, away=odds[1]
```

For three-outcome sports markets:

```text
[home, draw, away] -> home=odds[0], draw=odds[1], away=odds[2]
```

Bookmaker names can often be derived from `bs` betslip URLs. Slug example:

```text
/bookmakers/stake-com/... -> Stake Com
```

Fallback to `Bookmaker {id}` when no readable slug exists.

## Validation

Use both fixture and live-like checks:

- Decode a saved encrypted feed and assert recognizable JSON fields such as `eventData`, `requestPreMatch`, or `oddsdata`.
- Parse a minimal `d.oddsdata.back` JSON fixture with two-outcome odds.
- For a saved real Dota feed, assert multiple bookmaker rows and inspect the first few names/odds.

Keep live network tests ignored or optional so regular test runs stay deterministic.
