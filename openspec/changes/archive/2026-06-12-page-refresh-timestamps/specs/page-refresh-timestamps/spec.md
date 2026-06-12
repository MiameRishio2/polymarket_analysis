## ADDED Requirements

### Requirement: Persist page refresh timestamp
The system SHALL persist a dedicated refresh timestamp for each cached menu or event page in SQLite.

#### Scenario: New cache row stores refresh time
- **WHEN** the system saves refreshed category or event data for a cache key
- **THEN** the SQLite row for that cache key contains a non-empty `refreshed_at` timestamp representing the refresh time

#### Scenario: Existing cache row without refresh time
- **WHEN** the application opens an existing SQLite database whose cache rows do not have refresh timestamps
- **THEN** the system adds the refresh timestamp field and treats existing rows as refreshed at `1970-01-01T00:00:00Z`

### Requirement: Return page refresh timestamp
The system SHALL return the persisted page refresh timestamp in API data for every menu and event page level.

#### Scenario: Fetch root menu page data
- **WHEN** the client requests `/api/menu`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch sport menu page data
- **WHEN** the client requests `/api/menu/football`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch nested category menu page data
- **WHEN** the client requests `/api/menu/football/world`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch competition menu page data
- **WHEN** the client requests `/api/menu/football/world/world-championship-2026`
- **THEN** the response data includes `refreshed_at`

#### Scenario: Fetch competition event page data
- **WHEN** the client requests `/api/events/football/world/world-championship-2026`
- **THEN** the response data includes `refreshed_at`

### Requirement: Update timestamp on refresh
The system SHALL update the persisted page refresh timestamp when a page refresh successfully stores new data.

#### Scenario: Refresh root menu page
- **WHEN** the client posts to `/api/menu/refresh` and the system stores refreshed data
- **THEN** the persisted `refreshed_at` value for the root menu cache key is updated from the previous value

#### Scenario: Refresh nested competition events
- **WHEN** the client posts to `/api/events/football/world/world-championship-2026/refresh` and the system stores refreshed event data
- **THEN** the persisted `refreshed_at` value for the event cache key is updated from the previous value

### Requirement: Display page refresh timestamp
The menu UI SHALL display the page refresh timestamp for every menu page level.

#### Scenario: Open root menu page
- **WHEN** the browser renders `/menu`
- **THEN** the page shows a refresh timestamp label populated from the API response or `1970-01-01T00:00:00Z` when unavailable

#### Scenario: Open competition page
- **WHEN** the browser renders `/menu/football/world/world-championship-2026/`
- **THEN** the page shows a refresh timestamp label populated from the event API response when event data is displayed
