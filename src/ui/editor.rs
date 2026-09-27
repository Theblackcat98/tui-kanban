//! The task editor overlay: a title and a description field.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;

use super::bars::hint_line;
use super::text::{input_view, truncate_text};
use super::{AnimationKind, centered_rect, fade_in, faint, fg, fill, overlay_block, progress, put};
use crate::app::{EditorField, EditorState, Model, TextInput};
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    editor: &EditorState,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let modal = centered_rect(area, 64, 11);
    frame.render_widget(Clear, modal);
    let title = if editor.task_id.is_some() {
        "Edit task"
    } else {
        "New task"
    };
    let block = overlay_block(model, title, theme.border);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let inner = Rect::new(
        inner.x + 2,
        inner.y + 1,
        inner.width.saturating_sub(4),
        inner.height.saturating_sub(2),
    );
    if inner.height >= 7 {
        field(
            frame,
            Rect::new(inner.x, inner.y, inner.width, 2),
            model,
            "Title",
            "What needs doing?",
            &editor.title,
            editor.field == EditorField::Title,
        );
        field(
            frame,
            Rect::new(inner.x, inner.y + 3, inner.width, 2),
            model,
            "Description",
            "Add details (optional)",
            &editor.description,
            editor.field == EditorField::Description,
        );
    }
    let message = match &editor.error {
        Some(error) => Line::from(Span::styled(
            truncate_text(error, inner.width as usize),
            fg(theme.danger).add_modifier(Modifier::BOLD),
        )),
        None => hint_line(model, Context::Editor, inner.width as usize),
    };
    put(
        frame,
        inner.x,
        inner.bottom().saturating_sub(1),
        inner.width,
        message,
    );
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}

/// A label above a one-line input on `surface`. The active field has an
/// accent bar and the cursor.
fn field(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    label: &str,
    placeholder: &str,
    input: &TextInput,
    active: bool,
) {
    let theme = &model.ui.theme;
    let label_style = if active {
        fg(theme.accent).add_modifier(Modifier::BOLD | theme.active_modifier)
    } else {
        faint(model)
    };
    put(
        frame,
        area.x,
        area.y,
        area.width,
        Line::from(Span::styled(label.to_owned(), label_style)),
    );
    let row = Rect::new(area.x, area.y + 1, area.width, 1);
    fill(frame, row, Style::default().bg(theme.surface));
    if active {
        put(
            frame,
            row.x,
            row.y,
            1,
            Line::from(Span::styled("▎", fg(theme.accent))),
        );
    }
    let text_width = row.width.saturating_sub(3);
    let x = row.x + 2;
    if input.value.is_empty() {
        put(
            frame,
            x,
            row.y,
            text_width,
            Line::from(Span::styled(placeholder.to_owned(), faint(model))),
        );
        if active {
            frame.set_cursor_position((x, row.y));
        }
        return;
    }
    let (visible, cursor) = input_view(&input.value, input.cursor, text_width as usize);
    put(
        frame,
        x,
        row.y,
        text_width,
        Line::from(Span::styled(visible, fg(theme.text))),
    );
    if active {
        frame.set_cursor_position((x + cursor as u16, row.y));
    }
}
