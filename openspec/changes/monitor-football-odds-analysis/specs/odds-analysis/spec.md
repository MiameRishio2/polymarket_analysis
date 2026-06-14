# odds-analysis

## ADDED Requirements

### Requirement: Monitoring start collects latest odds

When a scheduled match is switched to `monitoring_started: true`, the system SHALL attempt to collect current odds from the match's OddsPortal and Polymarket links and persist a latest analysis snapshot.

#### Scenario: Collection errors do not block monitoring

- **WHEN** a source cannot be fetched or parsed
- **THEN** the scheduler monitoring flag is still updated
- **AND** the latest snapshot records a source-specific error

### Requirement: OddsPortal draw probability is ignored

OddsPortal football 1/X/2 odds SHALL be converted into two-way home/away probabilities by ignoring the draw outcome and normalizing only home and away implied probabilities.

#### Scenario: Three-way odds become two-way probabilities

- **GIVEN** home odds `2.00`, draw odds `3.00`, and away odds `4.00`
- **WHEN** the OddsPortal probabilities are calculated
- **THEN** home probability is `66.67%`
- **AND** away probability is `33.33%`
- **AND** draw probability is not displayed as a competing outcome

### Requirement: Analysis page shows monitored snapshots

The `/analysis` page SHALL show latest monitored match odds snapshots as visual comparison cards.

#### Scenario: Monitored snapshots are visible

- **GIVEN** a monitored match has a latest odds snapshot
- **WHEN** the user opens `/analysis`
- **THEN** the page fetches analysis data from the backend
- **AND** displays Polymarket and OddsPortal home/away probabilities with source status

### Requirement: Analysis page refreshes odds every second

The `/analysis` page SHALL request an odds collection refresh every second while the page is open, and the backend SHALL prevent overlapping collection runs.

#### Scenario: Live refresh updates the analysis view

- **GIVEN** a monitored match is visible on `/analysis`
- **WHEN** the page remains open
- **THEN** the page posts to the analysis collection endpoint once per second
- **AND** the endpoint returns the current stored snapshots without waiting for slow external sources
- **AND** updates the displayed latest snapshots and OddsPortal history after each successful refresh response

### Requirement: OddsPortal all-market history is visible

For monitored matches with an OddsPortal link, the system SHALL persist all available OddsPortal AJAX market odds rows and expose them to `/analysis` as match-scoped history.

#### Scenario: All OddsPortal market odds render as a horizontal time series

- **GIVEN** OddsPortal returns multiple markets and bookmaker rows for a monitored match
- **WHEN** the user opens `/analysis`
- **THEN** the page displays each market, bookmaker, and outcome odds line
- **AND** the odds are plotted horizontally over recent collection timestamps
