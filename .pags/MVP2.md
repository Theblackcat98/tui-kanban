# TUI Kanban MVP2

MVP2 keeps the keyboard-first, local-first character of the MVP while making the board easier to scan and navigate. The main product change is a workspace with two complementary views:

- **Board**: the workflow view, organized by columns.
- **All tasks**: a card-centric overview grouped by column.

The existing right-side task detail drawer remains a contextual inspector. A separate, quiet left navigation rail will organize columns only. The two panels have distinct purposes and should not be conflated.

## Product principles

- Prefer hierarchy, spacing, and typography over additional borders or chrome.
- Keep every workflow reachable from the keyboard.
- Preserve task identity by UUID across search, movement, view changes, and column navigation.
- Make the smallest useful amount of information visible in a card, with more detail available in the drawer.
- Keep board data and UI preferences separate. View state is transient; task and column data remains in the board document.

## Phase 0: Foundation

- Add `ViewMode` with `Board` and `AllTasks` states.
- Add explicit focus state for the column rail and card canvas.
- Represent a selected task by UUID and derive its current location when needed.
- Add a board-wide task query that returns task IDs and column locations.
- Extract shared card layout and text-width helpers.
- Consolidate keyboard command definitions so the footer, help overlay, and handlers stay synchronized.
- Expand rendering coverage for wide, narrow, empty, long-text, and monochrome layouts.

## Phase 1: Visual reset

### Column rail

The rail contains columns only:

```text
COLUMNS

  Backlog        12
  In Progress     4
  Done            8
```

- Use one active-column accent marker and stronger text.
- Align counts without placing a border around every row.
- Keep the rail visible on wide terminals and hide or collapse it on narrow terminals.
- Move focus between the rail and the card canvas with `Tab`.
- Keep view switching in the header or a `v` shortcut rather than mixing it into the column list.

### Cards

Use one shared comfortable card renderer in both Board and All tasks:

- Title with one or two wrapped lines.
- One or two description lines.
- Column/status and relative updated time.
- Space for priority, tags, due dates, and pinned state in later phases.
- A subtle selected surface plus an accent edge; do not rely only on a saturated background.
- No filler blank row.
- Terminal-cell-aware truncation and wrapping.

### All tasks

- Group cards under each column with a count.
- Flow cards in rows on wide terminals and in a vertical stream on narrow terminals.
- Apply search across every group.
- Preserve the selected task while filtering and moving.
- Support the same `Enter`, `e`, `d`, and `H`/`L` actions as the Board view.
- Provide a view-specific scroll position and selected-card indicator.

### Detail drawer

Keep the existing right-side drawer and improve its information hierarchy:

- Title as the primary heading.
- Column/status as the secondary heading.
- Scrollable description body.
- Created, updated, and short-ID metadata.
- A compact, anchored action footer for edit, move, delete, and close.
- Use spacing and one divider instead of nested cards or extra borders.

## Phase 2: Workflow control and speed

### Workflow control

- Reorder tasks within a column.
- Add, rename, delete, and reorder columns.
- Add work-in-progress limits.
- Move a task to an arbitrary column.
- Add bounded undo and redo.
- Sort and filter by column, creation time, and update time.

### Power-user speed

- Add a command palette.
- Add `g` / go-to navigation for columns and tasks.
- Add quick capture from anywhere.
- Add reusable task templates.
- Add keyboard-only duplicate, archive, and snooze actions.
- Drive the footer, help screen, and command palette from one command registry.

## Phase 3: Personal planning

Add the following after introducing a backward-compatible schema migration:

- Priority
- Tags
- Due dates
- Pinned or starred tasks
- Today, Overdue, Recently Updated, and Needs Attention views
- Saved filters
- Optional checklists or recurring tasks

## Phase 4: History and portability

- Activity history and an undo timeline.
- Archive and snooze instead of permanent deletion.
- Markdown and JSONL import/export.
- Automatic backups before migrations.
- Board health indicators such as stale tasks and WIP pressure.
- Multiple boards and workspace switching.
- Optional integrations after the core workflow is stable.

## Initial key model

- `v`: toggle Board and All tasks.
- `Tab`: move focus between the column rail and cards.
- `Enter`: open the task detail drawer.
- `e`: edit the selected task.
- `d`: delete the selected task.
- `H` / `L`: move the selected task between columns.
- `/`: search.
- `Esc`: close the drawer or clear search.
- `?`: show help.
- `q` / `Ctrl+C`: quit.

## MVP2 acceptance criteria

- Board and All tasks expose the same task set.
- The All tasks view is grouped by column and remains usable on narrow terminals.
- The detail drawer opens from either view and has a clear information hierarchy.
- The column rail selects columns without confusing rail focus with task selection.
- Selection survives search, column changes, and task movement by using task UUIDs.
- Existing version-1 board JSON continues to load without requiring a migration.
- Wide, narrow, long-content, empty-state, and `NO_COLOR` rendering cases are covered.
- Keyboard help, footer hints, and handlers use one consistent command model.

## Current implementation

The first MVP2 vertical slice is implemented in this checkout:

- Board and All tasks view switching with `v`.
- Rail and card focus switching with `Tab`.
- Stable UUID-backed task selection across columns, views, and search.
- Grouped All tasks cards with a shared comfortable card renderer.
- Columns-only left navigation rail on wide terminals.
- Improved right-side task detail drawer with created/updated metadata, task actions, and description scrolling.
- Updated footer and help text for the new navigation model.
- Rendering and interaction tests for the new state and layout paths.

Phase 2 and later remain the next implementation milestones after this visual reset is accepted.
