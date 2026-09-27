//! Where tui-kanban keeps files outside a project's board: the config file
//! and themes in the config directory, personal boards in the data
//! directory, and small bits of state (a dismissed tip, recent boards) in
//! the state directory.

use std::env;
use std::path::PathBuf;

fn non_empty(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// `$XDG_CONFIG_HOME`, `%APPDATA%` on Windows, or `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = non_empty("XDG_CONFIG_HOME") {
        return Some(dir);
    }
    if cfg!(windows) {
        return non_empty("APPDATA");
    }
    non_empty("HOME").map(|home| home.join(".config"))
}

/// `$XDG_STATE_HOME/tui-kanban`, `%LOCALAPPDATA%\tui-kanban` on Windows,
/// or `~/.local/state/tui-kanban`.
pub fn state_dir() -> Option<PathBuf> {
    let base = if let Some(dir) = non_empty("XDG_STATE_HOME") {
        Some(dir)
    } else if cfg!(windows) {
        non_empty("LOCALAPPDATA")
    } else {
        non_empty("HOME").map(|home| home.join(".local").join("state"))
    };
    base.map(|dir| dir.join("tui-kanban"))
}

/// `$XDG_DATA_HOME/tui-kanban`, `%APPDATA%\tui-kanban` on Windows, or
/// `~/.local/share/tui-kanban`: where personal boards live.
pub fn data_dir() -> Option<PathBuf> {
    let base = if let Some(dir) = non_empty("XDG_DATA_HOME") {
        Some(dir)
    } else if cfg!(windows) {
        non_empty("APPDATA")
    } else {
        non_empty("HOME").map(|home| home.join(".local").join("share"))
    };
    base.map(|dir| dir.join("tui-kanban"))
}

/// `~/.config/tui-kanban/config.toml`, or its equivalent.
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("tui-kanban").join("config.toml"))
}

/// The home directory, where searching upwards for a board stops.
pub fn home_dir() -> Option<PathBuf> {
    non_empty(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
}
