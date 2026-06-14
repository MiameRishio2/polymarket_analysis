---
change: monitor-football-odds-analysis
design-doc: docs/superpowers/specs/2026-06-14-monitor-football-odds-analysis-design.md
base-ref: 93a3ca79c59a12b1760d2822da85ab3ffbac59ac
---

# Monitor Football Odds Analysis Plan

1. Add tests for OddsPortal two-way probability conversion and oddsdata JSON parsing.
2. Add tests for storing and listing latest odds snapshots.
3. Implement odds analysis models, storage schema, and collectors.
4. Wire scheduler monitoring start to run best-effort collection.
5. Add `/api/analysis/odds`.
6. Replace `/analysis` placeholder with snapshot cards.
7. Run targeted tests, then the broader test suite.
