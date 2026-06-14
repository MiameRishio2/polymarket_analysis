## ADDED Requirements

### Requirement: Non-disruptive analysis refresh
The analysis page SHALL avoid replacing interactive controls during automatic background refreshes.

#### Scenario: User is choosing a monitored item
- **WHEN** the user is interacting with the monitored-item selector and an automatic background refresh interval fires
- **THEN** the page keeps the current selector DOM stable and does not interrupt the user's selection

#### Scenario: Manual refresh still reloads data
- **WHEN** the user clicks the manual refresh button
- **THEN** the page reloads analysis data even if a recent interaction occurred

### Requirement: Preserve selected monitored item
The analysis page SHALL preserve the selected monitored item across refreshes when that item is still present in the API response.

#### Scenario: Selected item remains available
- **WHEN** refreshed analysis data includes the previously selected monitored item
- **THEN** the page continues rendering that item instead of falling back to the first item

#### Scenario: Selected item disappears
- **WHEN** refreshed analysis data no longer includes the previously selected monitored item
- **THEN** the page falls back to the first available monitored item

### Requirement: Filter all-market chart series
The all-market OddsPortal history chart SHALL allow users to hide and show individual win-probability series.

#### Scenario: Hide one series
- **WHEN** the user disables a series from the chart legend
- **THEN** that series is omitted from the combined SVG chart while other enabled series remain visible

#### Scenario: Show hidden series
- **WHEN** the user re-enables a previously hidden series from the chart legend
- **THEN** that series appears in the combined SVG chart again

#### Scenario: All series hidden
- **WHEN** the user hides every available series
- **THEN** the chart area shows an empty filtered state and keeps legend controls available
