## Context

The event enrichment path only runs for `football/world/world-championship-2026`. It previously built one Polymarket slug from a hand-maintained team-code table and the event date parsed from OddsPortal display text or JSON-LD.

## Decision

Use Gamma keyset World Cup event lists as the first lookup path, accepting only a result whose searchable text contains both teams and whose slug/start/end date matches one of the OddsPortal date candidates. Keep generated slug candidates as a fallback, but query them with plain HTTP so missing slugs fail silently instead of producing noisy CLOB client 404 logs. Candidate generation still includes both team codes and normalized country-name aliases across the parsed event date and its adjacent days. This addresses USA VS Paraguay, Mexico VS South Korea, Switzerland VS Bosnia & Herzegovina, and similar cases without adding one-off match exceptions.

## Non-Goals

- Do not change time zone handling in this hotfix.
- Do not add a persistent team-code discovery cache.
- Do not change scheduler persistence or event API response shapes.

## Verification

Run focused Rust tests that cover third-level event parsing, Polymarket slug candidate generation, adjacent date candidates, validated search-result fallback, and Gamma keyset event fallback.
