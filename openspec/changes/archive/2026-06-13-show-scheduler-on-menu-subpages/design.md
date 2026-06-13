## Implementation
`public/menu.html` already loads scheduler state before rendering page content. The rendering restriction lives in a small wrapper that only prepends `renderSchedulerSection()` when the current path is the root `/menu`.

Update that wrapper so all menu page types prepend the same Scheduler section. This keeps data loading, monitor toggles, delete actions, and event-row scheduling behavior on the existing code path.

## Verification
- Add a focused JavaScript test proving a subpage render includes `.scheduler-section` and the scheduled match.
- Run the existing menu page JavaScript tests.
