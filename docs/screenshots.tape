# The README's screenshots, taken with vhs
# (https://github.com/charmbracelet/vhs). From the repository root:
#
#   cargo build --release && vhs docs/screenshots.tape
#
# Like docs/demo.tape, it runs on a copy of examples/demo-board.json in a
# temporary project, with its own config and state directories.
#
# vhs takes a screenshot on the frame after the Screenshot command, so
# each one is followed by a short sleep.

# vhs always records a GIF; this one is thrown away.
Output target/screenshots.gif

Set Shell "bash"
Set FontSize 15
Set Width 1280
Set Height 720
Set Padding 16
Set WindowBar Colorful
Set WindowBarSize 36
Set BorderRadius 10
Set Margin 20
Set MarginFill "#11111b"
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

Hide
Type "export PATH=$PWD/target/release:$PATH XDG_CONFIG_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d)"
Enter
Type "mkdir -p $XDG_CONFIG_HOME/tui-kanban $XDG_STATE_HOME/tui-kanban && touch $XDG_STATE_HOME/tui-kanban/tip-dismissed"
Enter
Type `echo 'theme = "mocha"' > $XDG_CONFIG_HOME/tui-kanban/config.toml`
Enter
Type "project=$(mktemp -d)/website && mkdir -p $project && cp examples/demo-board.json $project/.tui-kanban.json"
Enter
Type "cd $project && clear && tui-kanban"
Enter
Sleep 2s
Show

# The board.
Sleep 500ms
Screenshot docs/screenshots/board.png
Sleep 500ms

# A card's details: Markdown, with a checklist.
Hide
Enter
Tab
Tab
Sleep 800ms
Show
Screenshot docs/screenshots/detail.png
Sleep 500ms

# Search with filter terms.
Hide
Escape
Type "/in:progress card"
Sleep 800ms
Show
Screenshot docs/screenshots/search.png
Sleep 500ms

# All tasks, grouped by column.
Hide
Escape
Type "v"
Sleep 800ms
Show
Screenshot docs/screenshots/all-tasks.png
Sleep 500ms

# The command palette.
Hide
Type "v:move"
Sleep 800ms
Show
Screenshot docs/screenshots/palette.png
Sleep 500ms

# Which-key: every key you can press here.
Hide
Escape
Type " "
Sleep 800ms
Show
Screenshot docs/screenshots/which-key.png
Sleep 500ms

# Columns: In Progress at its limit, Done collapsed, the rail focused.
Hide
Escape
Type "1jjL"
Sleep 300ms
Type "3z"
Tab
Type "k"
Sleep 800ms
Show
Screenshot docs/screenshots/columns.png
Sleep 500ms

# The light theme, Latte, on a fresh copy of the board.
Hide
Type "q"
Sleep 500ms
Type "cp $OLDPWD/examples/demo-board.json .tui-kanban.json && clear && tui-kanban --theme latte"
Enter
Sleep 2s
Show
Screenshot docs/screenshots/latte.png
Sleep 500ms

Hide
Type "q"
