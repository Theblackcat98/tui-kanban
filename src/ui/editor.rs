//! The task editor overlay: a one-line title and a multi-line
//! description.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use tui_textarea::TextArea;

use super::bars::hint_line;
use super::text::truncate_text;
use super::{AnimationKind, centered_rect, fade_in, faint, fg, fill, overlay_block, progress, put};
use crate::app::{EditorField, EditorState, Model};
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
    // The description gets as many rows as fit, from 3 to 10.
    let description_rows = area.height.saturating_sub(16).clamp(3, 10);
    let modal = centered_rect(area, 72, description_rows + 10);
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
    // Label, one row, gap, label, then the description down to the gap
    // above the footer.
    if inner.height >= 7 {
        let description_height = inner.height - 6;
        field(
            frame,
            Rect::new(inner.x, inner.y, inner.width, 2),
            model,
            "Title",
            &editor.title,
            editor.field == EditorField::Title,
        );
        field(
            frame,
            Rect::new(inner.x, inner.y + 3, inner.width, description_height + 1),
            model,
            "Description",
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

/// A label above a text area on `surface`. The active field has an accent
/// bar and the cursor.
fn field(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    label: &str,
    text: &TextArea<'_>,
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
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    )
    .intersection(frame.area());
    if body.is_empty() {
        return;
    }
    fill(frame, body, Style::default().bg(theme.surface));
    if active {
        for y in body.top()..body.bottom() {
            put(
                frame,
                body.x,
                y,
                1,
                Line::from(Span::styled("▎", fg(theme.accent))),
            );
        }
    }
    let text_area = Rect::new(
        body.x + 2,
        body.y,
        body.width.saturating_sub(3),
        body.height,
    );
    frame.render_widget(text, text_area);
    if active && let Some(position) = text.rendered_cursor_position() {
        frame.set_cursor_position(position);
    }
}
