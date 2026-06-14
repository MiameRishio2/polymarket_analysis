## 1. Refresh Interaction

- [x] 1.1 Add frontend state to detect active or recent user interaction with analysis controls.
- [x] 1.2 Make automatic background refreshes skip or defer DOM replacement while interaction protection is active.
- [x] 1.3 Preserve the selected monitored item across refreshed API responses when its stable key remains available.

## 2. Chart Series Filtering

- [x] 2.1 Add stable per-series keys for all-market history lines.
- [x] 2.2 Render legend controls that toggle individual series visibility.
- [x] 2.3 Filter hidden series out of the combined SVG chart while keeping controls available.
- [x] 2.4 Show a clear empty filtered state when all series are hidden.

## 3. Verification

- [x] 3.1 Add focused JavaScript regression tests for refresh interaction protection and selected-item preservation.
- [x] 3.2 Add focused JavaScript regression tests for chart series hide/show rendering.
- [x] 3.3 Run the relevant frontend/page tests and fix regressions.
