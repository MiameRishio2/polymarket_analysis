## ADDED Requirements

### Requirement: Fourth-level event table rendering
The frontend SHALL render fourth-level competition pages as an event table when event-list data is available.

#### Scenario: Render Mexico South Africa event row
- **WHEN** `/menu/football/world/world-championship-2026` receives an event row for Mexico and South Africa
- **THEN** the list table displays `Mexico VS South Africa`, the event start time, and the parsed OddsPortal link

#### Scenario: Preserve category fallback
- **WHEN** `/menu/{sport}/{category}/{league}` receives category data instead of event data
- **THEN** the page keeps rendering the existing type, name, and URL category table

### Requirement: Event table pagination
The frontend SHALL paginate event rows with the same 10 items per page rule used by menu tables.

#### Scenario: Paginate event rows
- **WHEN** a fourth-level event page has more than 10 parsed events
- **THEN** only 10 event rows are shown on the first page and pagination controls allow navigation to later rows
