//! The quick-add prompt: one line that adds tasks to the end of a lane,
//! and stays open for the next one.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;

use super::bars::hint_line;
use super::text::{input_view, truncate_text};
use super::{AnimationKind, centered_rect, fade_in, faint, fg, fill, overlay_block, progress, put};
use crate::app::{Model, TextInput};
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    column: usize,
    input: &TextInput,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let modal = centered_rect(area, 56, 6);
    frame.render_widget(Clear, modal);
    let name = model
        .board
        .columns
        .get(column)
        .map_or("", |column| column.name.as_str());
    let title = format!("Add to {}", truncate_text(name, 30));
    let block = overlay_block(model, &title, super::lane_color(model, column));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let inner = Rect::new(
        inner.x + 1,
        inner.y + 1,
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(1),
    );
    let row = Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height));
    fill(
        frame,
        row,
        Style::default()
            .bg(theme.surface)
            .add_modifier(theme.active_modifier),
    );
    put(
        frame,
        row.x,
        row.y,
        1,
        Line::from(Span::styled("▎", fg(theme.accent))),
    );
    let text_x = row.x + 2;
    let text_width = row.width.saturating_sub(3);
    if input.value.is_empty() {
        put(
            frame,
            text_x,
            row.y,
            text_width,
            Line::from(Span::styled("Task title", faint(model))),
        );
        frame.set_cursor_position((text_x, row.y));
    } else {
        let (visible, cursor) = input_view(&input.value, input.cursor, text_width as usize);
        put(
            frame,
            text_x,
            row.y,
            text_width,
            Line::from(Span::styled(visible, fg(theme.text))),
        );
        frame.set_cursor_position((text_x + cursor as u16, row.y));
    }
    if inner.height >= 3 {
        put(
            frame,
            inner.x,
            inner.y + 2,
            inner.width,
            hint_line(model, Context::QuickAdd, inner.width as usize),
        );
    }
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
