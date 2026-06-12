## ADDED Requirements

### Requirement: Event rows expose external market links
The system SHALL expose structured external destinations for parsed event rows while preserving the existing OddsPortal `url` field.

#### Scenario: Event row includes OddsPortal and Polymarket destinations
- **WHEN** the event API returns a World Cup event row with a matching Polymarket slug
- **THEN** the row contains the OddsPortal H2H URL in `url`
- **AND** the row contains the matching Polymarket sports URL in `polymarket_url`

#### Scenario: Missing Polymarket match is represented explicitly
- **WHEN** Polymarket slug lookup returns no exact match for an OddsPortal event
- **THEN** the event row still appears with its OddsPortal URL
- **AND** `polymarket_url` is absent or null

### Requirement: World Cup Polymarket slug lookup
The system SHALL derive World Cup Polymarket slug candidates from OddsPortal event teams and start date, then search Polymarket by exact slug.

#### Scenario: Mexico South Africa World Cup slug
- **WHEN** an event row has home team `Mexico`, away team `South Africa`, and date `2026-06-11`
- **THEN** the slug candidate list includes `fifwc-mex-rsa-2026-06-11`

#### Scenario: Lookup failures are non-fatal
- **WHEN** Polymarket lookup fails, times out, or returns malformed data
- **THEN** the event API still returns the OddsPortal event row

### Requirement: Event table renders link buttons
The frontend SHALL render event row external destinations as buttons in the existing "链接" column.

#### Scenario: Both links are available
- **WHEN** an event row has `url` and `polymarket_url`
- **THEN** the "链接" column shows enabled `OddsPortal` and `Polymarket` buttons

#### Scenario: Polymarket is unavailable
- **WHEN** an event row has no `polymarket_url`
- **THEN** the "链接" column shows an enabled `OddsPortal` button
- **AND** shows a disabled `Polymarket` button
