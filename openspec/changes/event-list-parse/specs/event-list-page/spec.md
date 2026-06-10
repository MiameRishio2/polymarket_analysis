## ADDED Requirements

### Requirement: Event list API
The system SHALL expose parsed event rows for an OddsPortal competition page without changing existing category menu API contracts.

#### Scenario: Fetch World Championship events
- **WHEN** the client requests event rows for `/football/world/world-championship-2026/`
- **THEN** the response contains `ok: true` and event data with matchup, start time, and OddsPortal link fields

#### Scenario: Empty event extraction fallback
- **WHEN** the target competition page contains no valid event rows
- **THEN** the event response returns an empty event list without persisting an error-shaped category cache entry

### Requirement: Event row extraction
The scraper SHALL extract match/event rows from a football competition page, including home team, away team, display matchup, start time text, and target OddsPortal URL.

#### Scenario: Extract Mexico South Africa row
- **WHEN** the OddsPortal HTML contains an event row for Mexico and South Africa with a link to `/football/h2h/mexico-O6iHcNkd/south-africa-W2ijYvlr/#h4EoUB7T:1X2;2`
- **THEN** the scraper returns an event with display matchup `Mexico VS South Africa`, both team names, the row start time, and that link

#### Scenario: Ignore unrelated links
- **WHEN** the OddsPortal HTML contains navigation, results, standings, archive, or unrelated sport links
- **THEN** those links are excluded from event rows

### Requirement: Event list caching
The system SHALL cache event-list data separately from category menu data.

#### Scenario: Event cache does not overwrite category cache
- **WHEN** event rows are saved for `football/world/world-championship-2026`
- **THEN** existing `menu_football_world_world-championship-2026` category cache data is not overwritten with incompatible event JSON
