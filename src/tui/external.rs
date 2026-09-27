//! Editing text in the user's own editor (`$VISUAL`, then `$EDITOR`),
//! through a temporary Markdown file.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::process::Command;

/// The editor to run, as a program and its arguments: `$VISUAL`, then
/// `$EDITOR` (split at spaces, so `code --wait` works), then `vi`, or
/// `notepad` on Windows.
pub fn editor_command(visual: Option<OsString>, editor: Option<OsString>) -> Vec<String> {
    let chosen = [visual, editor]
        .into_iter()
        .flatten()
        .map(|value| value.to_string_lossy().trim().to_owned())
        .find(|value| !value.is_empty());
    match chosen {
        Some(command) => command.split_whitespace().map(str::to_owned).collect(),
        None if cfg!(windows) => vec!["notepad".to_owned()],
        None => vec!["vi".to_owned()],
    }
}

/// Opens `text` in the user's editor and returns what they saved. The
/// terminal must be restored to normal mode first.
pub fn edit(text: &str) -> Result<String, String> {
    let command = editor_command(env::var_os("VISUAL"), env::var_os("EDITOR"));
    edit_with(&command, text)
}

fn edit_with(command: &[String], text: &str) -> Result<String, String> {
    let path = env::temp_dir().join(format!("tui-kanban-{}.md", uuid::Uuid::new_v4()));
    fs::write(&path, text).map_err(|error| format!("could not write a temporary file: {error}"))?;
    let status = Command::new(&command[0])
        .args(&command[1..])
        .arg(&path)
        .status();
    let result = match status {
        Ok(status) if status.success() => fs::read_to_string(&path)
            .map_err(|error| format!("could not read the edited file: {error}")),
        Ok(status) => Err(format!("{} exited with {status}", command[0])),
        Err(error) => Err(format!("could not run {}: {error}", command[0])),
    };
    let _ = fs::remove_file(&path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_wins_then_editor_then_a_default() {
        let os = |value: &str| Some(OsString::from(value));
        assert_eq!(editor_command(os("hx"), os("vim")), ["hx"]);
        assert_eq!(editor_command(None, os("code --wait")), ["code", "--wait"]);
        assert_eq!(editor_command(os("  "), os("nano")), ["nano"]);
        let fallback = editor_command(None, None);
        assert!(fallback == ["vi"] || fallback == ["notepad"]);
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_file_comes_back() {
        let command = |script: &str| vec!["sh".to_owned(), "-c".to_owned(), script.to_owned()];
        // The file's path is the script's $0.
        let edited = edit_with(
            &command(r#"printf '%s done' "$(cat "$0")" > "$0""#),
            "draft",
        );
        assert_eq!(edited.as_deref(), Ok("draft done"));
        let failed = edit_with(&command("exit 3"), "draft");
        assert!(failed.unwrap_err().starts_with("sh exited with"));
    }
}
