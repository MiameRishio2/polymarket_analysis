## ADDED Requirements

### Requirement: Hide Placeholder Bookmaker Labels
The analysis page SHALL NOT render OddsPortal market-history series whose bookmaker label is only a generated placeholder.

#### Scenario: Missing bookmaker name with unknown ID
- **GIVEN** an OddsPortal history row has no readable `bookmaker_name`
- **AND** its `bookmaker_id` is not in the client-side known bookmaker map
- **WHEN** the analysis page builds market-history series
- **THEN** no series is created for that row

#### Scenario: Persisted placeholder bookmaker name
- **GIVEN** an OddsPortal history row has `bookmaker_name` matching `Bookmaker <number>`
- **WHEN** the analysis page builds market-history series
- **THEN** no series is created for that row

#### Scenario: Known bookmaker ID fallback
- **GIVEN** an OddsPortal history row has no readable `bookmaker_name`
- **AND** its `bookmaker_id` is in the client-side known bookmaker map
- **WHEN** the analysis page builds market-history series
- **THEN** the series uses the mapped readable bookmaker name

#### Scenario: Legacy placeholder for confirmed OddsPortal bookmaker
- **GIVEN** an OddsPortal history row has `bookmaker_name` matching `Bookmaker <number>`
- **AND** its `bookmaker_id` maps to a confirmed OddsPortal bookmaker such as 22bet, BetFury, BetInAsia, or Bets.io
- **WHEN** the analysis page builds market-history series
- **THEN** the series uses the mapped readable bookmaker name
