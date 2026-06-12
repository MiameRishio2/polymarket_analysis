## Purpose

Define scheduler storage, API, and menu UI behavior for matches selected from competition event pages.

## Requirements

### Requirement: Scheduler SQLite storage
The system SHALL persist scheduled matches in SQLite using the same database file as the menu cache.

#### Scenario: Initialize scheduler storage
- **WHEN** the application starts with an existing or new menu database
- **THEN** the scheduler table exists without removing existing menu cache rows

#### Scenario: Upsert scheduled match
- **WHEN** the same event is added to the scheduler more than once
- **THEN** the system stores one scheduler row and updates its mutable fields

### Requirement: Scheduler match metadata
The system SHALL store enough metadata for each scheduled match to identify, display, and monitor it.

#### Scenario: Store event row as scheduled match
- **WHEN** a client schedules an event row from a competition page
- **THEN** the persisted row includes matchup, home team when available, away team when available, start time, OddsPortal URL, Polymarket URL when available, monitoring state, added timestamp, and updated timestamp

### Requirement: Scheduler monitor toggle
The system SHALL expose whether monitoring has started for each scheduled match and allow that state to be changed.

#### Scenario: Add match with monitoring disabled
- **WHEN** a client adds a match without specifying monitor state
- **THEN** the scheduler entry has `monitoring_started` set to false

#### Scenario: Toggle monitor state
- **WHEN** a client updates a scheduler entry monitor state
- **THEN** the persisted scheduler entry reflects the new `monitoring_started` value

### Requirement: Scheduler API
The system SHALL expose scheduler APIs under the existing web server.

#### Scenario: List scheduled matches
- **WHEN** a client requests the scheduler list API
- **THEN** the response contains all scheduler entries ordered by monitoring state, start time, and matchup

#### Scenario: Add scheduled match
- **WHEN** a client posts a valid scheduled match payload
- **THEN** the response contains the persisted scheduler entry

#### Scenario: Delete scheduled match
- **WHEN** a client deletes a scheduler entry by id
- **THEN** subsequent scheduler list responses do not include that entry

### Requirement: Root menu scheduler display
The `/menu` page SHALL display all scheduler entries.

#### Scenario: Open root menu
- **WHEN** the browser renders `/menu`
- **THEN** the page shows a scheduler section populated from the scheduler list API

#### Scenario: Empty scheduler
- **WHEN** the scheduler list API returns no entries
- **THEN** the root menu page shows an empty scheduler state without hiding the sports list
