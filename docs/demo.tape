# The README's demo, recorded with vhs
# (https://github.com/charmbracelet/vhs). From the repository root:
#
#   cargo build --release && vhs docs/demo.tape
#
# It runs on a copy of examples/demo-board.json in a temporary project,
# with its own config and state directories, so nothing of yours is
# touched and the first-run tip doesn't show.

Output docs/demo.gif

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
Set TypingSpeed 70ms
Set Framerate 24

Hide
Type "export PATH=$PWD/target/release:$PATH XDG_CONFIG_HOME=$(mktemp -d) XDG_STATE_HOME=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d)"
Enter
Type "mkdir -p $XDG_CONFIG_HOME/tui-kanban $XDG_STATE_HOME/tui-kanban && touch $XDG_STATE_HOME/tui-kanban/tip-dismissed"
Enter
Type `echo 'theme = "mocha"' > $XDG_CONFIG_HOME/tui-kanban/config.toml`
Enter
Type "project=$(mktemp -d)/website && mkdir -p $project/src && cp examples/demo-board.json $project/.tui-kanban.json"
Enter
Type "cd $project/src && PS1='website/src $ ' && clear"
Enter
Show

# Started anywhere in a project, it finds the project's board.
Type "tui-kanban"
Sleep 400ms
Enter
Sleep 2s

# Move through the cards, and move one along.
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms
Type "L"
Sleep 1.2s
# In Progress is now at its limit of 3, so the next move asks first.
Type "h"
Sleep 600ms
Type "L"
Sleep 1.5s
Type "n"
Sleep 800ms

# Open a card: its description is Markdown, with a checklist.
Enter
Sleep 1.2s
Tab
Sleep 400ms
Tab
Sleep 400ms
Space
Sleep 1s
Escape
Sleep 600ms

# Search with filter terms.
Type "/"
Sleep 300ms
Type "in:progress card"
Sleep 1.2s
Escape
Sleep 600ms

# Every task, grouped by column.
Type "v"
Sleep 1.5s
Type "v"
Sleep 600ms

# The command palette: collapse Done out of the way.
Type "3"
Sleep 400ms
Type ":"
Sleep 600ms
Type "collapse"
Sleep 1s
Enter
Sleep 1.2s

# Space shows every key you can press.
Type " "
Sleep 2s
Escape
Sleep 500ms

# End on the board, not the shell.
Hide
Type "q"
