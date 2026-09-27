# tui-kanban

A calm, keyboard-first Kanban board for your terminal. Each project
keeps its board in a small JSON file next to its code, and running
`tui-kanban` anywhere in the project opens it, the way git finds `.git`.

![A demo: moving a card, a work-in-progress limit, a card's checklist, search, All tasks, the command palette and collapsing a lane](docs/demo.gif)

- **Two views**: lanes for the workflow, or every task grouped by column.
- **Fast to drive**: vim-style keys, a command palette, a which-key
  panel, and undo and redo for every change.
- **Markdown descriptions** with checklists you tick without leaving the
  board, edited in a built-in editor or your own `$EDITOR`.
- **Fuzzy search with filter terms** such as `in:progress`, `#tag` and
  `updated:<7d`.
- **Columns managed in the app**, with colours, work-in-progress limits
  and lanes that fold away.
- **Mouse support**: click, scroll and drag cards between lanes.
- **Safe with your data**: saved in the background and atomically, and
  never silently overwritten when another program changes the file.
- **Themes**: the four Catppuccin flavours, matched to your terminal's
  background, 16 colours, your own, or none with `NO_COLOR`.

## Install

With a [Rust toolchain](https://rustup.rs), 1.88 or newer:

```sh
cargo install --git https://github.com/Theblackcat98/tui-kanban
```

or, from a clone, `cargo install --path .`. Release binaries and a
crates.io release are planned.

## Quick start

```sh
cd ~/src/my-project
tui-kanban
```

The first time, there's no board yet: press `c` to create one in the
current directory (it's named after the directory, and saved as
`.tui-kanban.json`), or `p` to use your personal board instead. From
then on, `tui-kanban` finds it from anywhere in the project, up to the
git root.

Then `n` adds a task, `Enter` opens it, `H` and `L` move it between
lanes, and `q` quits. Press `?` for the keys on the screen you're on,
`:` to search every command, or `Space` for a panel of the keys you can
press next.

To try it on a sample board first:

```sh
cp examples/demo-board.json /tmp/demo.json && tui-kanban --board /tmp/demo.json
```

## A tour

### The board

![The Board view: a column rail on the left, and Backlog, In Progress and Done lanes of cards](docs/screenshots/board.png)

Each column is a lane, and each card shows its title, the first line of
its description, when it last changed, and checklist progress (`✓ 2/4`).
`h j k l` or the arrow keys move around, `1`–`9` jump to a lane, `H`/`L`
move a card to the next lane and `J`/`K` reorder it. The status line
shows what mode you're in, whether everything is saved, and the keys
that make sense right now.

On narrow terminals the board shows one lane at a time with a tab strip
naming the others; lanes that don't fit scroll sideways.

### Details and checklists

![The detail drawer open beside the board, showing a Markdown description with a checklist](docs/screenshots/detail.png)

`Enter` opens a card in the drawer. Descriptions are Markdown: headings,
lists, quotes, code and checklists. `Tab` moves between checklist items
and `Space` ticks them. `e` edits the card, and `E` opens the
description in your own `$EDITOR`.

### Search

![Searching for "in:progress card": one match, with the other lanes empty](docs/screenshots/search.png)

`/` filters every lane as you type. Text matches titles fuzzily, and
terms narrow it down:

| Term | Matches |
|---|---|
| `in:progress` | tasks in a column whose name contains "progress" |
| `#bug` | tasks with the hashtag `#bug` |
| `updated:<7d`, `created:>2w`, `updated:today` | tasks by when they changed or were made |
| `'exact`, `^prefix`, `suffix$`, `!not` | as in fzf |

`↓`/`↑` step through the matches, `Enter` keeps the filter while you
work, and `Esc` clears it.

### All tasks

![The All tasks view: every task in a grid, grouped under each column's name](docs/screenshots/all-tasks.png)

`v` switches to every task at once, grouped by column, for a quick
overview. The same keys work on the cards here.

### The command palette and which-key

![The command palette, with "move" typed and five matching commands](docs/screenshots/palette.png)

`:` or `Ctrl+K` lists every command available where you are, plus "go
to lane" and "go to task" for each one, with the keys for each command
on the right. Commands you run from it come first next time.

![The which-key panel along the bottom of the board, listing every key](docs/screenshots/which-key.png)

`Space` shows every key you can press on the current screen; the next
key does what it normally does.

### Columns and work-in-progress limits

![The rail focused on In Progress, which is at its limit of 3, with Done collapsed to a narrow strip](docs/screenshots/columns.png)

`Tab` focuses the column rail. There, `a` adds a column, `r` renames
it, `d` deletes it (asking where its tasks should go), `J`/`K` reorder,
`c` picks a colour and `w` sets a work-in-progress limit. A column at
its limit shows its count (`3/3`) in orange, and red past it, and
moving another task in asks first. `z` folds a lane into a narrow strip,
say to keep Done out of the way.

### Boards

`b` switches between the boards you've opened recently, and your
personal board. `--board` opens one by path, or by name.

### Mouse

Click a card to select it and double-click to open it; click a lane, a
rail entry or a view name to go there, and a hint in the status line to
run it. The wheel scrolls lanes, the drawer and menus. Drag a card to
move it: a line shows where it will land. Capturing the mouse stops the
terminal's own text selection (most terminals still select with `Shift`
held); `--no-mouse`, or `mouse = false` in the config file, turns it
off.

### Themes

![The same board in the light Catppuccin Latte theme](docs/screenshots/latte.png)

By default tui-kanban asks the terminal whether its background is light
or dark and picks Catppuccin Latte or Mocha to match. `--theme` picks
one of the four Catppuccin flavours, `ansi` (the terminal's own 16
colours), or a theme of your own; see [docs/themes.md](docs/themes.md).
A non-empty `NO_COLOR` turns colour off, showing state with bold,
reversed and underlined text instead.

## Keys

The essentials:

| Key | |
|---|---|
| `h j k l` / arrows | Move between lanes and cards |
| `Enter` | Open the selected card |
| `n` / `a` | New card (with a description) / quick add to the lane |
| `e` / `E` | Edit the card / its description in `$EDITOR` |
| `H` `L` / `J` `K` / `m` | Move the card to another lane / up or down / to any lane |
| `d` / `y` | Delete / duplicate the card |
| `u` / `U` | Undo / redo |
| `/` | Search; `Esc` clears it |
| `v` | Switch between Board and All tasks |
| `Tab` | Focus the column rail, to manage columns |
| `z` | Collapse or expand a lane |
| `b` | Switch boards |
| `?` / `:` / `Space` | Help / command palette / which-key |
| `q` | Quit |

<details>
<summary>Every key, generated from the command table</summary>

<!-- keys:start -->

#### Navigation

| Keys | | Where |
|---|---|---|
| `h/l  ←/→` | previous / next column | Board |
| `j/k  ↓/↑` | next / previous card | Board |
| `h/l  ←/→` | card to the left / right | All tasks |
| `j/k  ↓/↑` | row down / up | All tasks |
| `[/]` | previous / next column group | All tasks |
| `PgUp/PgDn` | up / down five cards or rows | Board, All tasks |
| `Home/End  g g/G` | first / last card | Board, All tasks |
| `j/k  ↓/↑  l/h  →/←` | next / previous column | Column rail |
| `Home/End` | first / last column | Column rail |
| `1–9` | lane by its number | Board, All tasks, Column rail, Move to |
| `g + letter` | lane by its initial | Board, All tasks |
| `Enter` | focus cards | Column rail |
| `Tab / Shift+Tab` | focus rail / cards | Board, All tasks, Column rail |
| `v` | Board / All tasks | Board, All tasks, Column rail |
| `b` | switch board | Board, All tasks, Column rail |

#### Tasks

| Keys | | Where |
|---|---|---|
| `Enter` | open details | Board, All tasks |
| `n/N` | new task below / above | Board, All tasks, Column rail |
| `a` | quick add to this lane | Board, All tasks |
| `e` | edit task | Board, All tasks, Details |
| `y` | duplicate task | Board, All tasks, Details |
| `m` | move to… | Board, All tasks, Details |
| `E` | edit description in $EDITOR | Board, All tasks, Details |
| `d` | delete task | Board, All tasks, Details |
| `H/L` | move task left / right | Board, All tasks, Details |
| `J/K` | move task down / up | Board, All tasks, Details |
| `u/U  Ctrl+Z/Ctrl+R` | undo / redo | Board, All tasks, Column rail, Details |
| `/` | search | Board, All tasks, Column rail |
| `Esc` | back / clear search | Board, All tasks, Column rail, Search |
| `Ctrl+N/Ctrl+P` | next / previous match | Board, All tasks |
| `Backspace` | remove the last filter term | Board, All tasks, Column rail |

#### Columns

| Keys | | Where |
|---|---|---|
| `a` | add a column | Column rail |
| `r` | rename column | Column rail |
| `d` | delete column | Column rail |
| `J/K` | move column down / up | Column rail |
| `c` | column colour | Column rail |
| `w` | work-in-progress limit | Column rail |
| `z` | collapse / expand lane | Board, Column rail |

#### Details

| Keys | | Where |
|---|---|---|
| `PgUp/PgDn` | scroll up / down a page | Details |
| `j/k  ↓/↑` | scroll down / up a line | Details |
| `Space` | tick / untick checklist item | Details |
| `Tab/Shift+Tab` | next / previous checklist item | Details |
| `Esc / q` | close details | Details |

#### Editor, search and dialogs

| Keys | | Where |
|---|---|---|
| `Enter` | apply | Search |
| `↓/↑  Ctrl+N/Ctrl+P` | next / previous match | Search |
| `Tab / Shift+Tab` | switch field | Editor |
| `Ctrl+S / Enter` | save (Enter in the title) | Editor |
| `Esc / Ctrl+C` | cancel | Editor |
| `y` | confirm | Delete |
| `n / Esc / q` | cancel | Delete |
| `Ctrl+O` | description in $EDITOR | Editor |
| `↓/↑  Ctrl+N/Ctrl+P` | next / previous | Command palette |
| `Enter` | run | Command palette |
| `Esc / Ctrl+C` | close | Command palette |
| `y` | confirm | Discard changes |
| `n / Esc / q` | keep editing | Discard changes |
| `r` | load it (u brings yours back) | Changed on disk |
| `o` | overwrite it with yours | Changed on disk |
| `Esc` | decide later | Changed on disk |
| `y` | without saving | Quit |
| `n / Esc` | stay | Quit |
| `j/k  ↓/↑` | next / previous | Move to, Column colour, Boards |
| `Enter` | pick | Move to |
| `Esc / q` | close | Move to, Column colour, Boards |
| `Enter` | open | Boards |
| `c` | create a board here | No board here |
| `p` | open your personal board | No board here |
| `Enter` | set | Column colour |
| `Enter` | save | Column |
| `Esc / Ctrl+C` | cancel | Column |
| `y` | confirm | Delete column |
| `h/l  ←/→` | where its tasks go | Delete column |
| `n / Esc / q` | cancel | Delete column |
| `y` | move anyway | Work-in-progress limit |
| `n / Esc / q` | don't move | Work-in-progress limit |
| `Enter` | add | Quick add |
| `Esc / Ctrl+C` | done | Quick add |

#### General

| Keys | | Where |
|---|---|---|
| `j/k  ↓/↑` | scroll down / up | Help |
| `Esc / q / ?` | close | Help |
| `?` | this help | Board, All tasks, Column rail, Details |
| `: / Ctrl+K` | command palette | Board, All tasks, Column rail, Details |
| `Space` | show the keys you can press | Board, All tasks, Column rail |
| `q` | quit | Board, All tasks, Column rail, No board here |
| `Ctrl+C` | quit (anywhere) | anywhere |

<!-- keys:end -->

</details>

Keys can be changed in the config file.

## Options

| Option | |
|---|---|
| `--board <PATH OR NAME>` | Open a board file, or a board by name: a recent board with that name, or a personal board of that name in the data directory |
| `--theme <NAME>` | `auto` (the default), `latte`, `frappe`, `macchiato`, `mocha`, `ansi`, or your own |
| `--no-animation` | Turn off animations; a non-empty `REDUCE_MOTION` does the same |
| `--no-mouse` | Leave the mouse to the terminal, so text can be selected as usual |
| `tui-kanban config` | Print where the config file is; add `--print-default` for a commented one with every default |

## Configuration

`~/.config/tui-kanban/config.toml` (`$XDG_CONFIG_HOME` is respected;
`%APPDATA%` on Windows) is optional, and command-line options win over
it. `tui-kanban config --print-default` prints a commented copy with
every setting and every command's keys.

```toml
theme = "latte"
animations = true
mouse = true
# The columns of a new board.
columns = ["Ideas", "Doing", "Done"]
# "auto" ("Sep 3"), or a pattern with %Y %y %m %d %e %b %B.
date_format = "%Y-%m-%d"

[keys]
new_task = "+"
search = ["/", "ctrl+f"]
```

The file is checked when tui-kanban starts: a misspelt setting or
command, or two commands sharing a key where both apply, is reported
rather than ignored.

## The board file

A board is one pretty-printed JSON file, so it diffs well and can be
committed with the project (or added to `.gitignore`):

```json
{
  "schema_version": 1,
  "name": "Website",
  "columns": [
    {
      "id": "in-progress",
      "name": "In Progress",
      "color": "peach",
      "wip_limit": 3,
      "tasks": [
        {
          "id": "0d4c1d2e-5b9a-4c55-9c1e-4a1b8f3e2a10",
          "title": "Write the README",
          "description": "- [x] Install\n- [ ] Keys",
          "created_at": 1787820000000,
          "updated_at": 1787906400000
        }
      ]
    }
  ]
}
```

- Column `color` (a Catppuccin colour name or `#rrggbb`), `wip_limit`
  and `collapsed` are optional. Without a colour, one is picked from the
  column's id, so it stays the same when columns move.
- Times are Unix milliseconds. Descriptions are Markdown.
- Fields tui-kanban doesn't know are kept and written back. A file from
  an older version is copied to `<file>.bak-v<version>` before it is
  upgraded; one from a newer version isn't opened.
- Saving replaces the file atomically. If another program changes it,
  tui-kanban loads the new version, or, with unsaved changes of its own,
  asks which to keep.

Where files live:

| | |
|---|---|
| A project's board | `.tui-kanban.json`, in the project |
| Personal and named boards | `~/.local/share/tui-kanban/boards/` (`$XDG_DATA_HOME`) |
| Config and themes | `~/.config/tui-kanban/` (`$XDG_CONFIG_HOME`) |
| Recent boards, the dismissed tip | `~/.local/state/tui-kanban/` (`$XDG_STATE_HOME`) |

## Development

```sh
cargo test                         # unit, key-sequence and snapshot tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

- The look is specified in [docs/design.md](docs/design.md); the
  snapshot tests in `tests/snapshots/` are its executable form. After a
  deliberate visual change, review the new snapshots with
  `cargo insta review` (or `INSTA_UPDATE=always cargo test`).
- The key reference above comes from the command table in
  `src/command.rs`; `UPDATE_README=1 cargo test --test readme`
  regenerates it.
- The demo and screenshots are recorded with
  [vhs](https://github.com/charmbracelet/vhs), each from a tape that
  runs on a copy of the demo board: `cargo build --release`, then
  `vhs docs/demo.tape` and `vhs docs/screenshots.tape`.
- The minimum supported Rust version is 1.88, checked in CI.

## License

[MIT](LICENSE)
