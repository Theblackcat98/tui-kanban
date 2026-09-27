//! The config file, `~/.config/tui-kanban/config.toml`: the theme,
//! animations, the mouse, the columns of new boards, the date format and
//! key bindings. Every setting is optional, and command-line options win
//! over it. `tui-kanban config --print-default` prints a commented copy
//! with the defaults.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use crate::command::{self, CommandId, Key};

/// The columns of a new board, unless the config file says otherwise.
pub const DEFAULT_COLUMNS: [&str; 3] = ["Backlog", "In Progress", "Done"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// A theme name or path, as for `--theme`.
    pub theme: Option<String>,
    pub animations: bool,
    pub mouse: bool,
    /// The columns of a new board.
    pub columns: Vec<String>,
    pub date_format: DateFormat,
    /// Commands whose keys are changed, and their new keys.
    pub keys: Vec<(CommandId, Vec<Key>)>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            animations: true,
            mouse: true,
            columns: DEFAULT_COLUMNS.map(str::to_owned).to_vec(),
            date_format: DateFormat::default(),
            keys: Vec::new(),
        }
    }
}

/// How dates older than a week are shown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DateFormat {
    /// "Sep 3", or "Sep 3, 2025" when the year isn't this year's.
    #[default]
    Auto,
    /// A pattern of text and `%Y` (2026), `%y` (26), `%m` (09), `%d`
    /// (03), `%e` (3), `%b` (Sep), `%B` (September) and `%%`.
    Pattern(String),
}

impl DateFormat {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text == "auto" {
            return Ok(Self::Auto);
        }
        let mut characters = text.chars();
        while let Some(character) = characters.next() {
            if character == '%' {
                match characters.next() {
                    Some('Y' | 'y' | 'm' | 'd' | 'e' | 'b' | 'B' | '%') => {}
                    Some(other) => return Err(format!("date_format: unknown %{other}")),
                    None => return Err("date_format: a lone % at the end".to_owned()),
                }
            }
        }
        Ok(Self::Pattern(text.to_owned()))
    }
}

/// The file as written. Unknown settings are errors, so a typo doesn't
/// silently do nothing.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    theme: Option<String>,
    animations: Option<bool>,
    mouse: Option<bool>,
    columns: Option<Vec<String>>,
    date_format: Option<String>,
    #[serde(default)]
    keys: BTreeMap<String, Keys>,
}

/// One key, or a list of them.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Keys {
    One(String),
    Many(Vec<String>),
}

impl Config {
    /// Reads the config file at `path`; a missing file means the defaults.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| format!("could not read {}", path.display()));
            }
        };
        Self::parse(&text).with_context(|| format!("invalid config file {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let raw: Raw = toml::from_str(text)?;
        let defaults = Self::default();
        let columns = match raw.columns {
            Some(columns) => {
                if columns.is_empty() || columns.iter().any(|name| name.trim().is_empty()) {
                    bail!(
                        "columns: a new board needs at least one column, and names can't be empty"
                    );
                }
                columns
            }
            None => defaults.columns,
        };
        let date_format = match raw.date_format {
            Some(text) => DateFormat::parse(&text).map_err(anyhow::Error::msg)?,
            None => DateFormat::Auto,
        };
        let mut keys = Vec::new();
        for (name, value) in raw.keys {
            let Some(id) = CommandId::from_name(&name) else {
                bail!("keys: there is no command called {name}");
            };
            let texts = match value {
                Keys::One(text) => vec![text],
                Keys::Many(texts) => texts,
            };
            let parsed = texts
                .iter()
                .map(|text| Key::parse(text))
                .collect::<Result<Vec<Key>, String>>()
                .map_err(|error| anyhow::anyhow!("keys: {name}: {error}"))?;
            keys.push((id, parsed));
        }
        let config = Self {
            theme: raw.theme,
            animations: raw.animations.unwrap_or(defaults.animations),
            mouse: raw.mouse.unwrap_or(defaults.mouse),
            columns,
            date_format,
            keys,
        };
        // Check the keys against the whole table now, so a clash is
        // reported when the file is read.
        config.command_table()?;
        Ok(config)
    }

    /// The command table with this file's keys.
    pub fn command_table(&self) -> Result<&'static [command::Command]> {
        if self.keys.is_empty() {
            return Ok(command::COMMANDS);
        }
        command::remap(&self.keys).map_err(|error| anyhow::anyhow!("keys: {error}"))
    }
}

/// A commented config file with every setting at its default, and every
/// command's keys.
pub fn default_file() -> String {
    let mut text = String::from(
        r#"# tui-kanban's config file: ~/.config/tui-kanban/config.toml
# ($XDG_CONFIG_HOME/tui-kanban/config.toml; %APPDATA%\tui-kanban on
# Windows). Every setting is optional; these are the defaults.
# Command-line options win over this file.

# The colour theme: auto (Latte on light terminals, Mocha on dark ones),
# latte, frappe, macchiato, mocha, ansi, the name of a theme in
# ~/.config/tui-kanban/themes, or a path to a .toml theme file.
# theme = "auto"

# Animations. --no-animation, or a non-empty REDUCE_MOTION, turns them
# off too.
# animations = true

# Use the mouse. Capturing it stops the terminal's own text selection
# (most terminals still select with Shift held); --no-mouse turns it off
# for one run.
# mouse = true

# The columns of a new board.
# columns = ["Backlog", "In Progress", "Done"]

# How dates older than a week are shown: "auto" ("Sep 3", with the year
# when it isn't this year), or a pattern of text and %Y (2026), %y (26),
# %m (09), %d (03), %e (3), %b (Sep), %B (September) and %%.
# date_format = "auto"

# Keys, by command. A key is a character ("n", "N", "?"), or a name
# (enter, esc, tab, shift+tab, space, backspace, del, up, down, left,
# right, home, end, pgup, pgdn, f1 to f12), with ctrl+ or alt+ in front
# if needed, or g and a key for a chord ("g g"). A list binds several
# keys, and [] leaves a command to the palette. Two commands can't share
# a key where both apply. Digits, g + letter, g and Space are fixed.
[keys]
"#,
    );
    for command in command::COMMANDS {
        if !command.id.remappable() || command.keys.is_empty() {
            continue;
        }
        let keys: Vec<String> = command
            .keys
            .iter()
            .map(|key| format!("{:?}", key.config_name()))
            .collect();
        text.push_str(&format!(
            "# {} = [{}]  # {}\n",
            command.id.name(),
            keys.join(", "),
            command.label
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn settings_are_read() {
        let config = Config::parse(
            r#"
            theme = "latte"
            animations = false
            mouse = false
            columns = ["To do", "Done"]
            date_format = "%Y-%m-%d"

            [keys]
            new_task = "+"
            search = ["/", "ctrl+f"]
            "#,
        )
        .unwrap();
        assert_eq!(config.theme.as_deref(), Some("latte"));
        assert!(!config.animations && !config.mouse);
        assert_eq!(config.columns, ["To do", "Done"]);
        assert_eq!(
            config.date_format,
            DateFormat::Pattern("%Y-%m-%d".to_owned())
        );
        assert_eq!(config.keys.len(), 2);
        let (_, search) = config
            .keys
            .iter()
            .find(|(id, _)| *id == CommandId::Search)
            .unwrap();
        assert_eq!(search[1].code, KeyCode::Char('f'));
        assert_eq!(search[1].modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn mistakes_are_reported() {
        for (text, expected) in [
            ("colour = 1", "unknown field"),
            ("columns = []", "at least one column"),
            ("date_format = \"%Q\"", "%Q"),
            ("[keys]\nfly = \"f\"", "no command called fly"),
            ("[keys]\nnew_task = \"hyper+n\"", "unknown key"),
            ("[keys]\nsearch = \"n\"", "used by both"),
        ] {
            let error = format!("{:#}", Config::parse(text).unwrap_err());
            assert!(error.contains(expected), "{text}: {error}");
        }
    }

    #[test]
    fn the_default_file_reads_back_as_the_defaults() {
        let text = default_file();
        assert_eq!(Config::parse(&text).unwrap(), Config::default());
        // With every setting uncommented, too.
        let uncommented = text
            .lines()
            .filter_map(|line| match line.strip_prefix("# ") {
                // A setting, without the comment after a key binding.
                Some(setting) if setting.contains(" = ") && !setting.starts_with('(') => {
                    Some(setting.split("  # ").next().unwrap_or(setting))
                }
                Some(_) => None,
                None => Some(line),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let config = Config::parse(&format!("{uncommented}\n")).unwrap_or_else(|error| {
            panic!("{error:#}\n{uncommented}");
        });
        assert_eq!(config.theme.as_deref(), Some("auto"));
        assert_eq!(config.columns, DEFAULT_COLUMNS);
        assert!(!config.keys.is_empty());
    }
}
