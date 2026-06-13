## Purpose

Define behavior for nested football menu pages and APIs beneath `/menu/{sport}/{category}`.

## Requirements

### Requirement: Third-level menu page
The system SHALL serve a local third-level menu page at `/menu/{sport}/{category}` using the existing static menu page shell.

#### Scenario: Open football Argentina page
- **WHEN** the browser requests `/menu/football/argentina`
- **THEN** the system returns the menu page HTML

### Requirement: Third-level menu API
The system SHALL expose third-level category data at `/api/menu/{sport}/{category}` and refresh it at `/api/menu/{sport}/{category}/refresh`, including the page refresh timestamp in each response.

#### Scenario: Fetch football Argentina children
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response contains `ok: true` and `data.sport` identifies the third-level path

#### Scenario: Refresh football Argentina children
- **WHEN** the client posts to `/api/menu/football/argentina/refresh`
- **THEN** the system fetches data from OddsPortal and stores it under a third-level cache key

#### Scenario: Third-level response includes refresh time
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response data contains `refreshed_at`

### Requirement: Third-level child link extraction
The scraper SHALL extract child links with exactly one segment under `/{sport}/{category}/`, such as `/football/argentina/primera-nacional/`.

#### Scenario: Include direct league child
- **WHEN** the OddsPortal HTML contains `href="/football/argentina/primera-nacional/"`
- **THEN** the scraper returns a row with slug `primera-nacional` and URL `/football/argentina/primera-nacional/`

#### Scenario: Exclude deeper or unrelated links
- **WHEN** the OddsPortal HTML contains paths outside `/{sport}/{category}/{child}/` or with extra path segments
- **THEN** those paths are excluded from the third-level menu data

### Requirement: Third-level frontend links
The frontend SHALL convert second-level OddsPortal category URLs to local third-level page links while displaying the original OddsPortal URL in the URL column.

#### Scenario: Render Argentina row
- **WHEN** a second-level row has URL `/football/argentina/`
- **THEN** the row link targets `/menu/football/argentina/`

### Requirement: Competition event scheduler action
Competition event rows rendered under `/menu/{sport}/{category}/{league}` SHALL include a scheduler action when event data is displayed.

#### Scenario: Render scheduler action on World Championship page
- **WHEN** the browser renders event rows for `/menu/football/world/world-championship-2026/`
- **THEN** each event row includes an action that can add the event to the scheduler

#### Scenario: Schedule event from row
- **WHEN** the user activates the scheduler action for an event row
- **THEN** the frontend posts the event metadata to the scheduler API and shows the row as scheduled after the request succeeds

#### Scenario: Preserve external links
- **WHEN** a competition event row includes OddsPortal or Polymarket links
- **THEN** the scheduler action does not remove or disable those external links

### Requirement: World Cup event Polymarket links
World Cup event rows under `/menu/football/world/world-championship-2026/` SHALL generate Polymarket public URL candidates for supported teams from both team-code and normalized country-name aliases, using adjacent date variants to tolerate display-timezone boundaries.

#### Scenario: Canada vs Bosnia & Herzegovina slug candidate
- **WHEN** an event row contains `Canada VS Bosnia & Herzegovina` with start time `12 Jun 2026, 21:00`
- **THEN** the generated Polymarket slug candidates include `fifwc-can-bih-2026-06-12`

#### Scenario: USA vs Paraguay country-name slug candidate
- **WHEN** an event row contains `USA VS Paraguay` with start time `13 Jun 2026, 21:00`
- **THEN** the generated Polymarket slug candidates include both code-based and country-name-based slug forms

#### Scenario: Adjacent display date slug candidate
- **WHEN** an event row contains `USA VS Paraguay` with start time `14 Jun 2026, 07:00`
- **THEN** the generated Polymarket slug candidates include a `2026-06-13` slug variant

#### Scenario: Validated search fallback
- **WHEN** exact slug lookups do not find a Polymarket event
- **THEN** the system may use a Polymarket search result only if it matches both teams and the event date

#### Scenario: Validated keyset event fallback
- **WHEN** Polymarket returns a keyset response containing World Cup events
- **THEN** the system may use a keyset event only if it matches both teams and the event date
