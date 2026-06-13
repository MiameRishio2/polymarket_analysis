## ADDED Requirements

### Requirement: Competition event ended marker
Competition event rows returned for `/menu/{sport}/{category}/{league}` SHALL include an `ended` boolean that identifies whether the scheduled event start time is before the current server time.

#### Scenario: Future event is open
- **WHEN** an event row has a future `start_time`
- **THEN** the API response contains `ended: false` for that event

#### Scenario: Past event is ended
- **WHEN** an event row has a past `start_time`
- **THEN** the API response contains `ended: true` for that event

#### Scenario: Unknown event time is open
- **WHEN** an event row has an empty or unparseable `start_time`
- **THEN** the API response contains `ended: false` for that event

### Requirement: Competition event status display
The fourth-level menu event table SHALL display each event row's ended/open status without removing OddsPortal, Polymarket, or scheduler actions.

#### Scenario: Render ended event row
- **WHEN** the browser renders an event row with `ended: true`
- **THEN** the row displays an ended status marker

#### Scenario: Render open event row
- **WHEN** the browser renders an event row with `ended: false`
- **THEN** the row displays an open status marker

#### Scenario: Preserve event actions
- **WHEN** the browser renders event status markers
- **THEN** OddsPortal links, Polymarket links, and scheduler actions remain available according to the row data

### Requirement: Parent navigation for nested menu pages
Nested menu pages SHALL expose a parent navigation link that targets the immediate parent menu path, while the root `/menu` page SHALL not render a parent link.

#### Scenario: Root menu has no parent link
- **WHEN** the browser renders `/menu`
- **THEN** no parent navigation link is shown

#### Scenario: Sport menu parent link
- **WHEN** the browser renders `/menu/football`
- **THEN** the parent navigation link targets `/menu`

#### Scenario: Competition menu parent link
- **WHEN** the browser renders `/menu/football/world/world-championship-2026/`
- **THEN** the parent navigation link targets `/menu/football/world`
