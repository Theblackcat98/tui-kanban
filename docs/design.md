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
the accent colour.

### Filter bar

While a search is active, a row below the top bar holds it, with the
match count on the right (`3 of 12`). While typing it is the search box,
across the whole row; once applied (Enter), each term is a chip
(`in:progress ×`) and Backspace removes the last one.

Text matches a title fuzzily (letters in order, ignoring case and
accents) or a description as written; `in:column`, `#tag`,
`updated:<7d` / `>2w` / `today` and `created:…` filter by column,
hashtag and time, and fzf's `'exact`, `^prefix`, `suffix$` and `!not`
work too. The matched letters are highlighted on cards in bold
`accent` (bold and underlined without colour). While typing, ↓/↑ (or
Ctrl+N/Ctrl+P) move through the matches across all lanes; after
applying, Ctrl+N/Ctrl+P do.

### Status line

`[MODE] ● saved  hints…`. The mode pill names the current context
(BOARD, ALL TASKS, RAIL, SEARCH, DETAIL, EDIT, HELP, DELETE, CONFLICT,
QUIT). The save status is `● saved` (`success`), `◌ saving`
(`text_muted`) while a change waits to be written, `✕ not saved`
(`danger`) when the last save failed, or `✕ changed on disk` (`warning`)
when another program changed the file (see [Saving](#saving)). Hints come
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
  and the task count in `text_faint` (`2/5` while filtering). A column
  with a work-in-progress limit counts against it (`3/4`); the count
  turns `warning` at the limit and `danger` past it (bold, and bold
  reversed, without colour). The rail and the tab strip colour their
  counts the same way.
- Underline: `▔` across the lane in the lane's accent, dimmed unless the
  lane is focused.
- Cards scroll by whole cards, never partially. `↑ 2 more` / `↓ 3 more`
  show what is hidden. Each lane remembers its own scroll position.
- Empty: `No tasks yet`, plus `n to add one` in the focused lane.
- **Collapsed** (`z`): the lane is a three-cell strip with its count,
  its underline and its name written downwards. Its cards are hidden in
  the Board view, so they can't be selected or matched by a search; h/l
  still stop on it, and `z` expands it again. All tasks shows every
  column. Adding a task to a collapsed lane expands it. At the Compact
  breakpoint a collapsed lane says `Collapsed · z to expand`.
- At the Compact breakpoint the header is a tab strip instead:
  `‹ Backlog 3 · In Progress 2 · Done 5 ›`, with the active column in
  its lane colour and arrows when columns are off-screen.

### Card

- A tile on `surface`, with a `▎` bar in the lane's accent.
- Title: bold, wrapped to at most two lines, ending in `…` if cut.
- Description: its first line with content in `text_muted`, without
  its Markdown (`- [ ] tag` reads `☐ tag`), cut with `…`. Left out when
  empty.
- Metadata: `◷ 5m ago` in `text_faint`; dates older than a week read
  `Sep 3`, or as the config file's `date_format` says. A description
  with a checklist adds `✓ 2/5`, in `success` once every item is
  ticked.
- Height fits the content: two to four rows.
- **Selected** (and the cards have focus): the tile turns `selection`
  and the bar becomes a bolder `▌`.

### Rail

The column list on `panel`. The active column has a `▌` marker in its
lane colour; the count is right-aligned with a gap. When the rail has
focus, the active entry is on `selection`.

Columns are managed from the rail, and every change can be undone:

| Key | |
|---|---|
| `a` | Add a column after the active one (a one-line prompt) |
| `r` | Rename the column; its id, and so its colour, stays |
| `d` | Delete the column. The dialog says where its tasks go, the column before it at first: h/l choose another column, or deleting them too |
| `J` / `K` | Move the column down / up (right / left on the board) |
| `c` | Pick its colour from a menu of the theme's accents, or "Automatic" |
| `w` | Set its work-in-progress limit; empty or 0 removes it |
| `z` | Collapse or expand its lane (in the Board view too) |

Moving a task into a column that is already at its limit (with H/L,
the "move to…" menu or the mouse) asks first: `y` moves it anyway.

### Detail drawer

On `panel`, with an accent edge. The title wraps in full (up to a third
of the height); two lines of metadata follow (`Backlog · #1a2b3c4d`,
then `updated 2d ago · created Sep 3`), then the description as
lightweight Markdown, scrollable (j/k a line, PgUp/PgDn a page, stopping
at the end) and with a scrollbar when it overflows. Its hints are in the
status line.

The Markdown is line by line: `#` headings in bold `accent`; `•` bullets,
numbered lists and `☐` / `☑` checklist items, with wrapped lines hanging
under their text; `│` quotes in italic; fenced code on `surface`, kept
as written; and inline **bold**, *italic* and `code`. Anything else is
plain text with its indentation kept.

When the description has a checklist, one item is focused (on
`selection`): Tab / Shift+Tab move between items and Space ticks or
unticks it, as an undoable change. `E` edits the description in
`$EDITOR`.

### Overlays

Help, the editor, the dialogs, the "move to…" menu and the
quick-add prompt float centred on `panel` with a rounded `border` (the
dialogs' borders are `danger`, the quick-add prompt's is the lane's
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

### Command palette

`:` or Ctrl+K opens a centred overlay listing every command available
on the screen it was opened from, then "Go to lane: …" for each column
and "Go to task: …" for each task (with its column, faint). Each row has
its shortcut on the right. Typing ranks the rows by fuzzy match, with
the matched letters highlighted; with nothing typed, commands recently
run from the palette come first, then actions before movement. ↓/↑ or
Ctrl+N/Ctrl+P choose, Enter runs, Esc or Ctrl+C closes. Going to a task
that a search hides clears the search.

### Which-key

Space shows a panel above the status line listing every key available
on the current screen; the next key does what it would have done
anyway. Holding `g` for 300 ms shows the keys that can follow it: `g`
for the first card, and each lane's initial. The panel sizes each
column to its content, and cuts labels only when there isn't room for
enough columns to show every key.

### Tip bar

Until it is dismissed, a one-row bar above the status line says `Press ? for
keys, : for commands · Esc to dismiss`. Opening help or pressing
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
| `:` / Ctrl+K | The command palette |
| `Space` | Which-key: every key for the current screen |
| `/` | Search; Ctrl+N / Ctrl+P jump between matches |

`b` switches boards (see [Boards](#boards)).

Moving faster: `1`–`9` jump to a lane, `g g` / `G` to the first or last
card, `g` + a letter to the next lane with that initial.

Changing tasks: `H`/`L` move a task to the previous or next lane, `m`
opens a "move to…" menu, `J`/`K` reorder within a lane, `n` / `N` add a
task below / above the selected one, `a` quick-adds to the end of the
lane from a one-line prompt that stays open for the next task, `y`
duplicates, and `u` / `U` undo and redo.

## Boards

Without `--board`, tui-kanban looks for `.tui-kanban.json` in the
current directory and each one above it, the way git finds `.git`,
stopping at the git root (or, outside a repository, at the home
directory). So running it anywhere in a project opens that project's
board. When there is none, a **Welcome** dialog (with an `accent`
border) asks: `c` creates a board here, named after the directory, and
`p` opens the personal board in the data directory
(`~/.local/share/tui-kanban/boards/personal.json`). Nothing is written
until one is chosen, and `q` quits.

`--board` takes a path, or a name: a recent board with that name (in
the file) or file name, or else a personal board of that name in the
data directory, created on its first change.

`b` opens the **board switcher**, a menu of the board open now (`●`),
the recently opened boards that still exist, and the personal board,
each with its task count and where it is (`~` for home). Enter saves
this board and opens the other one, with nothing selected and nothing
to undo; if this board can't be saved, it stays open and says so.
Recent boards are kept in the state directory.

## The config file

`~/.config/tui-kanban/config.toml` (`$XDG_CONFIG_HOME` is respected;
`%APPDATA%` on Windows) sets the theme, animations, the mouse, the
columns of new boards, the date format (`"auto"`, or a pattern with
`%Y %y %m %d %e %b %B`) and keys. Command-line options win over it.
`tui-kanban config` prints where it is, and `tui-kanban config
--print-default` prints a commented copy with every default, including
every command's keys, generated from the command table.

Keys are rebound by command name, as in `new_task = "+"` or
`search = ["/", "ctrl+f"]`. The file is checked when it is read: an
unknown setting or command, a key that can't be read, or two commands
sharing a key where both apply is an error that names the problem, and
tui-kanban doesn't start. Digits, `g` + letter, `g` and Space are fixed.
Help, hints, which-key and the palette all show the keys in use.

## Mouse

The keyboard comes first, but the mouse works where it is tried. What
is under the pointer is worked out from the same geometry the screen is
drawn with (`src/ui/hit.rs`), so a click always lands on what is shown.

- **Click** a card to select it, a lane header or empty lane space to
  focus that lane, a rail entry to focus the rail on it, a view name in
  the top bar to switch views, and `‹ N more` / `N more ›` to scroll the
  lanes. A hint in the status line runs its command. With the detail
  drawer open, clicking another card shows it there.
- **Double-click** a card to open it, a rail entry to focus its cards,
  and a collapsed lane to expand it.
- **The wheel** moves the selection in the active lane and in All tasks
  (which scrolls to follow it), scrolls other lanes without taking the
  selection, scrolls the drawer's description and help, and moves
  through menus and the palette.
- **Drag** a card to move it. While dragging, an `accent` line between
  cards (or a bar beside a card in All tasks) shows where it will land;
  dropping on a rail entry or a collapsed lane puts it at the end of
  that column. A full column asks first, as with the keys.

While an overlay other than the drawer is open, only the status line
responds. Capturing the mouse stops the terminal selecting text (most
terminals still do with Shift held); `--no-mouse`, or `mouse = false` in
the config file, leaves the mouse to the terminal.

## Saving

Changes are written in the background, 150 ms after the first one, so a
burst of changes is one save and a slow disk never holds up a key. The
file is replaced atomically.

- **A failed save** keeps the change and is retried after 1 s, then
  2 s, 4 s and so on up to 30 s. The error shows once, as a toast that
  stays until the next key; the status stays `✕ not saved` until a
  retry works.
- **Another program changes the file** (an editor, `git pull`, a second
  tui-kanban): when nothing here is unsaved, the new version is loaded,
  keeping the selection, with a "Reloaded" toast. `u` brings back the
  board as it was. When something here is unsaved, nothing is written
  and the **Changed on disk** dialog asks: `r` loads the file (`u`
  brings your version back), `o` overwrites it with yours, `Esc` decides
  later (the next change or quitting asks again). Every save checks the
  file first, so this works even where the file can't be watched.
- **Quitting** writes anything still waiting. If that fails, **Quit**
  asks before throwing the changes away: `y` or a second Ctrl+C quits,
  `n` stays.

The file is pretty-printed JSON with fields in a fixed order, so each
task is one block and diffs stay small. Fields tui-kanban doesn't know,
such as ones added by hand or by a newer version, are kept and written
back after the known ones, sorted by name. A file from an older version
is copied to `<file>.bak-v<version>` before it is upgraded; one from a
newer version isn't opened.

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
