# Visual design

This is the spec for how tui-kanban looks. The snapshot tests in
`tests/snapshots/` are its executable form: any visual change shows up
there as a reviewable diff.

Goal: calm and readable at 80×24, and still good at 200×50.

## Principles

- **Fewer boxes.** Lanes are separated by space, not borders. Cards are
  tiles, not bordered rectangles. Only overlays (help, the editor,
  dialogs) keep a border, because they float above other content.
- **Hierarchy from colour and weight only.** Titles are bold body text;
  descriptions are muted; metadata is faint. Accent colours mark state
  and identity (focus, selection, a lane's colour), never decoration.
- **Exactly one thing has focus**, and it is always visible.
- **Motion is short and purposeful**, and can be turned off.

## Colour roles

Colours are named by role, never by palette colour. `src/theme.rs` maps
each role to a colour for each theme: the four Catppuccin flavours
(Latte, Frappé, Macchiato, Mocha), `ansi` for 16-colour terminals, and
themes users write in TOML. `--theme auto` (the default) picks Latte on
light terminals and Mocha on dark ones. On terminals without true colour
the theme is mapped to the 256-colour palette, or replaced by `ansi`.

| Role | Used for | Dark flavours | Latte |
|---|---|---|---|
| `bg` | The page background | base | base |
| `panel` | The rail, the bars, the detail drawer, overlays | mantle | mantle |
| `surface` | Card tiles and input fields | surface0 | crust |
| `selection` | The selected card tile | surface1 | surface0 |
| `border` | Overlay borders | surface2 | surface1 |
| `text` | Titles and body text | text | text |
| `text_muted` | Descriptions, unfocused lane names | subtext1 | subtext1 |
| `text_faint` | Metadata, counts, hints, placeholders | overlay2 | subtext0 |
| `accent` | The focused control, the mode pill, overlay titles | mauve | mauve |
| `focus` | The focused lane's underline and the rail marker | lavender | lavender |
| `danger` / `success` / `warning` / `info` | Errors, "saved", warnings, info toasts | red / green / peach / sapphire | the same |
| `lanes[]` | Each lane's accent: its underline and its cards' bars | sapphire, peach, green, mauve, … | the same |

A column's accent is its own `color` from the board file (a palette name
such as `"teal"`, or `"#rrggbb"`) when it has one, and otherwise one of
`lanes[]` chosen from the column's id, so it doesn't change when columns
are reordered.

A test checks every flavour's contrast: at least 4.5:1 for `text` and
`text_muted` on the backgrounds they are drawn on, and 3:1 for
`text_faint`.

With `NO_COLOR`, every role is the terminal's default colour and state is
shown with attributes instead: **reversed** for the selection, **bold**
for focus, **dim** for muted and faint text, **underlined** for the
active input.

## Spacing

Everything sits on a one-cell grid.

- **Page**: a one-row bar at the top and a one-row status line at the
  bottom. The lane area has a one-cell margin on each side.
- **Lanes** are separated by a two-cell gutter.
- **Cards** are separated by one blank row. Inside a card there is one
  cell of padding after the accent bar and one at the right edge.
- **All tasks** sections are separated by one blank row.

## Breakpoints

Worked out once per frame from the terminal width (`src/layout.rs`), and
shared with the key handlers so what is drawn and what keys do agree.

| Breakpoint | Width | Layout |
|---|---|---|
| Compact | < 80 | One lane at a time, with a tab strip naming the others. No rail. The detail drawer covers the whole width. |
| Regular | 80–139 | The rail, plus as many lanes as fit at 24 cells or more; the rest scroll sideways, with `‹ N more` / `N more ›` above them. The detail drawer slides in over the right side. |
| Wide | ≥ 140 | As Regular, but the detail drawer is pinned beside the lanes instead of covering them. |

Below 40×12 the app shows a "terminal too small" screen and only quits.

## Components

### Top bar

`Board name   Board · All tasks` on the left, with the current view in
the accent colour. While searching, the search box sits on the right
and grows with the query; an applied filter shows as `/ query · N
matches`.

### Status line

`[MODE] ● saved  hints…`. The mode pill names the current context
(BOARD, ALL TASKS, RAIL, SEARCH, DETAIL, EDIT, HELP, DELETE). The save
icon is `●` when saved and `✕` when the last save failed. Hints come
from the command table, most important first, cut to whole hints that
fit, and hints for commands that would do nothing (such as "edit" with
no task selected) are left out. A toast replaces the hints while it
shows, so it is never cut off, in `success`, `info` or `danger`; error
toasts stay until the next key press.

### Lane

```
Backlog  3
▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔

▎ Write the README
▎ Cover install, usage, keys…
▎ ◷ 1d ago
```

- Header: the name (bold; `text` when focused, `text_muted` otherwise)
  and the task count in `text_faint` (`2/5` while filtering).
- Underline: `▔` across the lane in the lane's accent, dimmed unless the
  lane is focused.
- Cards scroll by whole cards, never partially. `↑ 2 more` / `↓ 3 more`
  show what is hidden. Each lane remembers its own scroll position.
- Empty: `No tasks yet`, plus `n to add one` in the focused lane.
- At the Compact breakpoint the header is a tab strip instead:
  `‹ Backlog 3 · In Progress 2 · Done 5 ›`, with the active column in
  its lane colour and arrows when columns are off-screen.

### Card

- A tile on `surface`, with a `▎` bar in the lane's accent.
- Title: bold, wrapped to at most two lines, ending in `…` if cut.
- Description: its first non-empty line in `text_muted`, cut with `…`.
  Left out when empty.
- Metadata: `◷ 5m ago` in `text_faint`; dates older than a week read
  `Sep 3`.
- Height fits the content: two to four rows.
- **Selected** (and the cards have focus): the tile turns `selection`
  and the bar becomes a bolder `▌`.

### Rail

The column list on `panel`. The active column has a `▌` marker in its
lane colour; the count is right-aligned with a gap. When the rail has
focus, the active entry is on `selection`.

### Detail drawer

On `panel`, with an accent edge. The title wraps in full (up to a third
of the height); two lines of metadata follow (`Backlog · #1a2b3c4d`,
then `updated 2d ago · created Sep 3`), then the description with its
line breaks and indentation kept, scrollable (j/k a line, PgUp/PgDn a
page, stopping at the end) and with a scrollbar when it overflows. Its
hints are in the status line.

### Overlays

Help, the editor, the delete dialog, the "move to…" menu and the
quick-add prompt float centred on `panel` with a rounded `border` (the
delete dialog's border is `danger`, the quick-add prompt's is the lane's
colour). Inputs are `surface` fields; the active one has an accent bar,
and empty ones show a placeholder. Every overlay's last row lists its
own keys, generated from the command table.

The editor has a one-line title that scrolls sideways and a
multi-line description that wraps at words and grows to fill the
overlay (3 to 10 rows). Enter saves from the title and starts a new line
in the description; Ctrl+S saves from either. Both fields take readline
keys (Ctrl+A/E/W/U/K, Alt+B/F), Ctrl+Z / Ctrl+R to undo and redo,
Shift+arrows to select, and pasted text in one piece.

Help lists the keys for the screen it was opened from (`Keys · Board`,
`Keys · Details`, …), grouped as in the command table.

### Tip bar

Until it is dismissed, a one-row bar above the status line says `Press ?
for the keys on any screen · Esc to dismiss`. Opening help or pressing
Esc on the board dismisses it for good (a marker file in the state
directory, `~/.local/state/tui-kanban`).

### Empty states

Empty states say what to do next: `No tasks yet · n to add one · ? for
all keys` on an empty board, and `No matches for "x" · Esc to clear`
while a filter matches nothing.

## Navigation

The same keys mean the same thing on every screen:

| Key | Meaning |
|---|---|
| `h j k l` / arrows | Move spatially: lanes and cards, the All tasks grid, menus |
| `Enter` | Open or confirm |
| `Esc` | Go back one level (close an overlay, clear a search, leave the rail); never quits |
| `q` | Quit from the board; close from inside an overlay |
| `?` | Help for the current screen |

Moving faster: `1`–`9` jump to a lane, `g g` / `G` to the first or last
card, `g` + a letter to the next lane with that initial.

Changing tasks: `H`/`L` move a task to the previous or next lane, `m`
opens a "move to…" menu, `J`/`K` reorder within a lane, `n` / `N` add a
task below / above the selected one, `a` quick-adds to the end of the
lane from a one-line prompt that stays open for the next task, `y`
duplicates, and `u` / `U` undo and redo.

## Motion

All animations are short (100–240 ms) and run at about 30 FPS.

- The selection colour eases in when the selection moves.
- A card that was just moved or saved briefly glows in its lane colour.
- Overlays fade in (their colours blend in from the background).
- The detail drawer slides in from the right at full width, so its text
  never reflows mid-animation.
- Toasts fade in.

`--no-animation`, or a non-empty `REDUCE_MOTION` environment variable,
turns all of it off.
