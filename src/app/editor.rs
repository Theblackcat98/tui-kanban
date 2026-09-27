//! The task editor's state: a one-line title and a multi-line
//! description, each a [`TextArea`] with readline keys, undo and
//! selection.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use tui_textarea::{CursorMove, CursorRenderMode, Input, TextArea, WrapMode};
use uuid::Uuid;

use crate::domain::Task;
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorField {
    Title,
    Description,
}

/// Where a new task goes: next to `anchor` (below it, or above it when
/// `above` is set), or at the end of `column` (the start, when `above`)
/// if there is no anchor or it has gone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
    pub column: usize,
    pub anchor: Option<Uuid>,
    pub above: bool,
}

#[derive(Clone, Debug)]
pub struct EditorState {
    pub task_id: Option<Uuid>,
    /// Where a new task goes. Unused when editing.
    pub placement: Option<Placement>,
    pub title: TextArea<'static>,
    pub description: TextArea<'static>,
    pub field: EditorField,
    pub error: Option<String>,
    /// The title and description the editor opened with, to tell whether
    /// closing it would lose anything.
    original: (String, String),
}

impl EditorState {
    pub fn new(placement: Placement, theme: &Theme) -> Self {
        Self::with_text(None, Some(placement), "", "", theme)
    }

    pub fn from_task(task: &Task, theme: &Theme) -> Self {
        Self::with_text(Some(task.id), None, &task.title, &task.description, theme)
    }

    fn with_text(
        task_id: Option<Uuid>,
        placement: Option<Placement>,
        title: &str,
        description: &str,
        theme: &Theme,
    ) -> Self {
        Self {
            task_id,
            placement,
            title: text_area(title, "What needs doing?", WrapMode::None, theme),
            description: text_area(
                description,
                "Add details (optional). Markdown works.",
                WrapMode::Word,
                theme,
            ),
            field: EditorField::Title,
            error: None,
            original: (title.to_owned(), description.to_owned()),
        }
    }

    pub fn title_text(&self) -> String {
        self.title.lines().join(" ")
    }

    pub fn description_text(&self) -> String {
        self.description.lines().join("\n")
    }

    /// Whether the draft differs from what the editor opened with.
    pub fn is_dirty(&self) -> bool {
        (self.title_text(), self.description_text()) != self.original
    }

    pub fn active(&mut self) -> &mut TextArea<'static> {
        match self.field {
            EditorField::Title => &mut self.title,
            EditorField::Description => &mut self.description,
        }
    }

    /// Applies an editing key to the active field. Returns whether the
    /// text changed.
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        let multi_line = self.field == EditorField::Description;
        edit(self.active(), key, multi_line)
    }

    /// Inserts pasted text into the active field. The title is one line,
    /// so line breaks become spaces there.
    pub fn paste(&mut self, text: &str) -> bool {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text = match self.field {
            EditorField::Title => text.replace('\n', " "),
            EditorField::Description => text,
        };
        self.active().insert_str(text)
    }

    /// Replaces the description, as after editing it in `$EDITOR`.
    pub fn set_description(&mut self, description: &str) {
        let lines = description.lines().map(str::to_owned).collect();
        self.description.set_lines(lines, (0, 0));
        self.description.move_cursor(CursorMove::Bottom);
        self.description.move_cursor(CursorMove::End);
    }
}

fn text_area(text: &str, placeholder: &str, wrap: WrapMode, theme: &Theme) -> TextArea<'static> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    let mut area = TextArea::new(lines);
    area.move_cursor(CursorMove::Bottom);
    area.move_cursor(CursorMove::End);
    area.set_wrap_mode(wrap);
    area.set_style(Style::default().fg(theme.text).bg(theme.surface));
    area.set_cursor_line_style(Style::default());
    // The terminal's own cursor marks the insertion point, as in every
    // other input.
    area.set_cursor_render_mode(CursorRenderMode::Hidden);
    area.set_placeholder_text(placeholder);
    area.set_placeholder_style(
        Style::default()
            .fg(theme.text_faint)
            .add_modifier(theme.muted_modifier),
    );
    area.set_selection_style(
        Style::default()
            .bg(theme.selection)
            .add_modifier(theme.selected_modifier),
    );
    area.set_max_histories(100);
    area
}

/// Applies a key to a text area, with readline's Ctrl+U (delete to the
/// start of the line) and Ctrl+Z for undo. Line breaks are only typed
/// into multi-line fields.
fn edit(area: &mut TextArea<'static>, key: &KeyEvent, multi_line: bool) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('u') if ctrl => area.delete_line_by_head(),
        KeyCode::Char('z') if ctrl => area.undo(),
        KeyCode::Enter if !multi_line => false,
        KeyCode::Char('m' | 'j') if ctrl && !multi_line => false,
        _ => area.input(Input::from(*key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(editor: &mut EditorState, code: KeyCode, modifiers: KeyModifiers) {
        editor.handle_key(&KeyEvent::new(code, modifiers));
    }

    fn type_text(editor: &mut EditorState, text: &str) {
        for character in text.chars() {
            press(editor, KeyCode::Char(character), KeyModifiers::NONE);
        }
    }

    fn placement() -> Placement {
        Placement {
            column: 0,
            anchor: None,
            above: false,
        }
    }

    #[test]
    fn description_takes_several_lines() {
        let mut editor = EditorState::new(placement(), &Theme::mocha());
        editor.field = EditorField::Description;
        type_text(&mut editor, "one");
        press(&mut editor, KeyCode::Enter, KeyModifiers::NONE);
        type_text(&mut editor, "two");
        assert_eq!(editor.description_text(), "one\ntwo");
        // The title stays on one line, and pastes are flattened into it.
        editor.field = EditorField::Title;
        type_text(&mut editor, "a");
        press(&mut editor, KeyCode::Enter, KeyModifiers::NONE);
        press(&mut editor, KeyCode::Char('j'), KeyModifiers::CONTROL);
        editor.paste("b\nc");
        assert_eq!(editor.title_text(), "ab c");
        assert_eq!(editor.title.lines().len(), 1);
    }

    #[test]
    fn readline_keys_and_undo() {
        let mut editor = EditorState::new(placement(), &Theme::mocha());
        type_text(&mut editor, "hello brave world");
        press(&mut editor, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(editor.title_text(), "hello brave ");
        press(&mut editor, KeyCode::Char('z'), KeyModifiers::CONTROL);
        assert_eq!(editor.title_text(), "hello brave world");
        press(&mut editor, KeyCode::Char('a'), KeyModifiers::CONTROL);
        type_text(&mut editor, "> ");
        press(&mut editor, KeyCode::Char('e'), KeyModifiers::CONTROL);
        type_text(&mut editor, "!");
        assert_eq!(editor.title_text(), "> hello brave world!");
        press(&mut editor, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(editor.title_text(), "");
    }

    #[test]
    fn dirty_only_after_a_change() {
        let task = Task::new("Title", "Body\nmore", 0);
        let mut editor = EditorState::from_task(&task, &Theme::mocha());
        assert!(!editor.is_dirty());
        type_text(&mut editor, "!");
        assert!(editor.is_dirty());
        press(&mut editor, KeyCode::Backspace, KeyModifiers::NONE);
        assert!(!editor.is_dirty());
    }
}
