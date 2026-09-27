# Themes

Pick a theme with `--theme`:

| Theme | |
|---|---|
| `auto` | The default: Latte on light terminals, Mocha on dark ones. |
| `latte`, `frappe`, `macchiato`, `mocha` | The four [Catppuccin](https://catppuccin.com) flavours. |
| `ansi` | The 16 standard colours, following the terminal's own palette. |
| a name | A theme file in `~/.config/tui-kanban/themes/<name>.toml` (`$XDG_CONFIG_HOME` is respected; `%APPDATA%` on Windows). |
| a path | A theme file anywhere, ending in `.toml`. |

A non-empty `NO_COLOR` turns colour off whatever the theme.

On terminals without true colour (`COLORTERM` isn't `truecolor` or
`24bit`), themes are mapped to the 256-colour palette when `TERM`
mentions 256 colours, and replaced by `ansi` otherwise.

## Writing a theme

A theme file starts from a built-in theme and changes some of its
colours. Colours are `"#rrggbb"` or a colour name from the base theme's
palette (for the Catppuccin flavours: `rosewater`, `flamingo`, `pink`,
`mauve`, `red`, `maroon`, `peach`, `yellow`, `green`, `teal`, `sky`,
`sapphire`, `blue`, `lavender`, `text`, `subtext1`, `subtext0`,
`overlay2`, `overlay1`, `overlay0`, `surface2`, `surface1`, `surface0`,
`base`, `mantle`, `crust`).

```toml
base = "mocha"

[colors]
bg = "#11111b"
accent = "pink"
lanes = ["blue", "#fab387", "green"]
```

The roles are described in [design.md](design.md#colour-roles): `bg`,
`panel`, `surface`, `selection`, `border`, `text`, `text_muted`,
`text_faint`, `accent`, `focus`, `danger`, `success`, `warning`, `info`,
and `lanes`, the list of lane colours used in turn.

## Column colours

Each column gets an accent from the theme's `lanes`, chosen from its id.
To pick one yourself, give the column a `color` in the board file, as a
palette name or `"#rrggbb"`:

```json
{ "id": "review", "name": "Review", "color": "teal", "tasks": [] }
```
