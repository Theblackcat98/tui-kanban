//! The "move to…" menu: every column, numbered, with the task's current
//! one marked.
//!
//! ```text
//! ╭ Move "Write the README" ─╮
//! │ 1 Backlog      current   │
//! │▌2 In Progress            │
//! │ 3 Done                   │
//! ╰──────────────────────────╯
//! ```

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use uuid::Uuid;

use super::bars::hint_line;
use super::text::{self, truncate_text};
use super::{
    AnimationKind, centered_rect, fade_in, faint, fg, fill, muted, overlay_block, progress, put,
};
use crate::app::Model;
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    task: Uuid,
    selected: usize,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let columns = &model.board.columns;
    let current = model.board.task_location(task).map(|(column, _)| column);
    let name_width = columns
        .iter()
        .map(|column| text::width(&column.name))
        .max()
        .unwrap_or(0);
    // Border and padding, then "▌9 " + name + "  current".
    let width = (name_width as u16 + 16).clamp(40, 60);
    let height = columns.len() as u16 + 5;
    let modal = centered_rect(area, width, height);
    frame.render_widget(Clear, modal);
    let title = model
        .board
        .task(task)
        .map(|task| format!("Move \"{}\"", truncate_text(&task.title, 24)))
        .unwrap_or_else(|| "Move to".to_owned());
    let block = overlay_block(model, &title, theme.border);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let list = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    // Keep the highlighted column in view on short terminals.
    let first = selected.saturating_sub(list.height.saturating_sub(1) as usize);
    for (row, (index, column)) in columns.iter().enumerate().skip(first).enumerate() {
        if row as u16 >= list.height {
            break;
        }
        let y = list.y + row as u16;
        let highlighted = index == selected;
        if highlighted {
            fill(
                frame,
                Rect::new(list.x, y, list.width, 1),
                Style::default()
                    .bg(theme.selection)
                    .add_modifier(theme.selected_modifier),
            );
            put(
                frame,
                list.x,
                y,
                1,
                Line::from(Span::styled("▌", fg(super::lane_color(model, index)))),
            );
        }
        let number = if index < 9 {
            format!("{} ", index + 1)
        } else {
            "  ".to_owned()
        };
        let marker = if current == Some(index) {
            "  current"
        } else {
            ""
        };
        let name_room = (list.width as usize)
            .saturating_sub(3 + marker.len() + 1)
            .min(name_width);
        let name = truncate_text(&column.name, name_room);
        let padding = " ".repeat(name_room.saturating_sub(text::width(&name)));
        let name_style = if highlighted {
            fg(theme.text).add_modifier(Modifier::BOLD)
        } else {
            muted(model)
        };
        put(
            frame,
            list.x + 1,
            y,
            list.width.saturating_sub(2),
            Line::from(vec![
                Span::styled(number, faint(model)),
                Span::styled(name, name_style),
                Span::styled(padding, name_style),
                Span::styled(marker, faint(model)),
            ]),
        );
    }
    put(
        frame,
        inner.x + 1,
        inner.bottom().saturating_sub(1),
        inner.width.saturating_sub(2),
        hint_line(
            model,
            Context::MoveTo,
            inner.width.saturating_sub(2) as usize,
        ),
    );
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
