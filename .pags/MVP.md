# TUI Kanban MVP

The MVP is a keyboard-first Rust application using Ratatui, Crossterm, and Catppuccin Mocha.

## Scope

- Three fixed columns: `Backlog`, `In Progress`, and `Done`
- Tasks with UUID, title, description, and timestamps
- Create, edit, delete, move, search, and detail-view actions
- Help overlay, confirmation dialogs, and save/error status
- No priorities, tags, due dates, custom columns, or undo initially
- Current-directory storage at `.tui-kanban.json`
- `--board <path>` override and `--no-animation` option

## Structure

```text
src/
  main.rs
  app.rs
  domain/
    board.rs
    task.rs
  storage/
    json_store.rs
  tui/
    event.rs
    terminal.rs
  ui/
    dashboard.rs
    task_editor.rs
    detail.rs
    help.rs
  theme.rs
  animation.rs
```

## Architecture

- Keep board mutations in a domain layer independent of rendering.
- Use an application state machine for dashboard, editor, detail, help, and confirmation modes.
- Use a single Crossterm event loop with dynamic polling:
  - Render on input or state changes.
  - Render animation frames at approximately 30 FPS only while effects are active.
- Keep render functions independent so they can be tested with Ratatui's `TestBackend`.
- Centralize all colors in `theme.rs`; widgets use semantic roles instead of raw palette values.

## Persistence

Use a versioned JSON document containing a board name, columns, and nested tasks. Autosave after every successful mutation. Serialize to a same-directory temporary file, sync it, then atomically replace the board file. Preserve the existing file if parsing or writing fails. Return readable errors for malformed JSON, invalid task data, and permission failures.

## Key bindings

- `h` / `l` or arrow keys: select column
- `j` / `k` or up/down: select card
- `n`: create task
- `e`: edit task
- `d`: delete task
- `H` / `L`: move selected task left/right
- `Enter`: open task details
- `/`: search
- `?`: help
- `Esc`: close the current overlay or clear search
- `q` / `Ctrl+C`: quit

## Visual design

- Catppuccin Mocha palette with semantic roles for background, surface, text, muted text, accents, selection, errors, and success.
- Rounded borders, clear spacing, column counts, selected-column emphasis, card selection highlighting, and compact card descriptions.
- Responsive layout: wide terminals show all columns; narrow terminals show the active column with navigation hints.
- Use only widely supported terminal characters and avoid Nerd Font dependencies.
- Respect `NO_COLOR` by falling back to terminal default colors.
- Expressive but short animations:
  - Task movement: eased highlight and settling effect
  - Detail drawer: slide and fade
  - Modals: brief expansion and fade
  - Toasts: slide, fade, and auto-dismiss
  - Selection: smooth color transition
- `--no-animation` disables all motion without changing application behavior.

## Implementation order

1. Create the Rust package and add compatible dependencies.
2. Implement domain types and board operations.
3. Implement JSON loading, validation, and atomic saving.
4. Add terminal setup, event handling, and application state transitions.
5. Build the dashboard, task editor, details drawer, help overlay, and status bar.
6. Add Catppuccin Mocha styling and responsive layout rules.
7. Add the animation engine and motion effects.
8. Add unit, persistence, keybinding, and `TestBackend` snapshot tests.
9. Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets`.
