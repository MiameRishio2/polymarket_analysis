## MODIFIED Requirements

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
