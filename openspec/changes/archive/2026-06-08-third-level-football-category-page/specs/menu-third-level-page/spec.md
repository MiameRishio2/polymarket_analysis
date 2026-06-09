## ADDED Requirements

### Requirement: Third-level menu page
The system SHALL serve a local third-level menu page at `/menu/{sport}/{category}` using the existing static menu page shell.

#### Scenario: Open football Argentina page
- **WHEN** the browser requests `/menu/football/argentina`
- **THEN** the system returns the menu page HTML

### Requirement: Third-level menu API
The system SHALL expose third-level category data at `/api/menu/{sport}/{category}` and refresh it at `/api/menu/{sport}/{category}/refresh`.

#### Scenario: Fetch football Argentina children
- **WHEN** the client requests `/api/menu/football/argentina`
- **THEN** the response contains `ok: true` and `data.sport` identifies the third-level path

#### Scenario: Refresh football Argentina children
- **WHEN** the client posts to `/api/menu/football/argentina/refresh`
- **THEN** the system fetches data from OddsPortal and stores it under a third-level cache key

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
