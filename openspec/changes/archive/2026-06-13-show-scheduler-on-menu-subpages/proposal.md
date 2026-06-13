## Why
Scheduler entries are currently visible only on `/menu`. Users navigating directly to sport, category, or competition pages cannot see the same scheduler context without returning to the root menu page.

## What Changes
- Render the existing Scheduler section on every menu page level.
- Keep the existing scheduler actions for event rows unchanged.
- Preserve current category/event pagination and API behavior.

## Scope
- Frontend menu page rendering only.
- No scheduler storage, API, or route changes.
