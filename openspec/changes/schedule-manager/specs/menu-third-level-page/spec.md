## ADDED Requirements

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
