//! The delete confirmation dialog.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use uuid::Uuid;

use super::bars::hint_line;
use super::text::truncate_text;
use super::{AnimationKind, centered_rect, fade_in, fg, muted, overlay_block, progress, put};
use crate::app::Model;
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, task: Uuid, clock: Clock) {
    let theme = &model.ui.theme;
    let modal = centered_rect(area, 52, 8);
    frame.render_widget(Clear, modal);
    let block = overlay_block(model, "Delete task", theme.danger);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let inner = Rect::new(
        inner.x + 2,
        inner.y + 1,
        inner.width.saturating_sub(4),
        inner.height.saturating_sub(2),
    );
    let title = model
        .board
        .task(task)
        .map(|task| task.title.as_str())
        .unwrap_or("this task");
    let question = format!(
        "Delete \"{}\"?",
        truncate_text(title, (inner.width as usize).saturating_sub(10))
    );
    let lines = [
        Line::from(Span::styled(
            question,
            fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("You can undo this with u.", muted(model))),
        Line::default(),
        hint_line(model, Context::Confirm, inner.width as usize),
    ];
    for (row, line) in lines.into_iter().enumerate() {
        if row as u16 >= inner.height {
            break;
        }
        put(frame, inner.x, inner.y + row as u16, inner.width, line);
    }
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
