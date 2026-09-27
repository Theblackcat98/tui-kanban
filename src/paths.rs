//! Where tui-kanban keeps files outside the board: themes in the config
//! directory, and small bits of state (such as a dismissed tip) in the
//! state directory.

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
