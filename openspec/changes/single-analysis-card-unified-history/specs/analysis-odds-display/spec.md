## ADDED Requirements

### Requirement: Single visible analysis schedule item
The analysis page SHALL render at most one monitored odds schedule item from the current odds snapshot response and allow the user to choose which item is visible when multiple items are available.

#### Scenario: Multiple monitored snapshots returned
- **WHEN** the current odds snapshot API returns multiple monitored match items
- **THEN** the analysis page renders one selected item
- **AND** the page provides a manual selector containing the available monitored items
- **AND** the summary counts reflect the visible item list

#### Scenario: User selects a monitored item
- **WHEN** the user selects a different monitored item
- **THEN** the analysis page renders that selected item
- **AND** the page loads history only for that selected item

### Requirement: Unified OddsPortal history chart
The analysis page SHALL render all visible OddsPortal market-history series in one shared time-series chart.

#### Scenario: Multiple history series exist
- **WHEN** a visible analysis item has multiple OddsPortal market-history series
- **THEN** the page displays one combined chart for those series
- **AND** the chart includes a legend identifying each market, bookmaker, and outcome line
