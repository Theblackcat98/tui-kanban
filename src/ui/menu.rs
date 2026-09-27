//! Menus: "move to…", every column numbered with the task's current one
//! marked, and a column's colours.
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
use ratatui::style::{Color, Modifier, Style};
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

/// One entry in a menu.
struct Row {
    /// Shown before the name: a number, or a colour swatch.
    lead: Span<'static>,
    name: String,
    /// The colour of the highlight's edge.
    accent: Color,
    current: bool,
}

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    task: Uuid,
    selected: usize,
    clock: Clock,
) {
    let current = model.board.task_location(task).map(|(column, _)| column);
    let rows: Vec<Row> = model
        .board
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| Row {
            lead: Span::styled(
                if index < 9 {
                    format!("{} ", index + 1)
                } else {
                    "  ".to_owned()
                },
                faint(model),
            ),
            name: column.name.clone(),
            accent: super::lane_color(model, index),
            current: current == Some(index),
        })
        .collect();
    let title = model
        .board
        .task(task)
        .map(|task| format!("Move \"{}\"", truncate_text(&task.title, 24)))
        .unwrap_or_else(|| "Move to".to_owned());
    render_menu(
        frame,
        area,
        model,
        &title,
        &rows,
        selected,
        Context::MoveTo,
        clock,
    );
}

/// The colours a column can have: automatic (chosen from its id), then
/// each accent the theme names, each with a swatch.
pub(crate) fn render_colors(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    column: usize,
    selected: usize,
    clock: Clock,
) {
    let Some(data) = model.board.columns.get(column) else {
        return;
    };
    let theme = &model.ui.theme;
    let automatic = theme.column_color(&data.id, None);
    let current = data.color.as_deref().map(str::trim);
    let mut rows = vec![Row {
        lead: Span::styled("■ ", fg(automatic)),
        name: "Automatic".to_owned(),
        accent: automatic,
        current: current.is_none(),
    }];
    for name in theme.accent_names() {
        let color = theme.column_color(&data.id, Some(name));
        rows.push(Row {
            lead: Span::styled("■ ", fg(color)),
            name: name.to_owned(),
            accent: color,
            current: current.is_some_and(|current| current.eq_ignore_ascii_case(name)),
        });
    }
    let title = format!("Colour for {}", truncate_text(&data.name, 24));
    render_menu(
        frame,
        area,
        model,
        &title,
        &rows,
        selected,
        Context::Colors,
        clock,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_menu(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    title: &str,
    rows: &[Row],
    selected: usize,
    context: Context,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let name_width = rows
        .iter()
        .map(|row| text::width(&row.name))
        .max()
        .unwrap_or(0);
    // Border and padding, then "▌9 " + name + "  current".
    let width = (name_width as u16 + 16).clamp(40, 60);
    let height = rows.len() as u16 + 5;
    let modal = centered_rect(area, width, height);
    frame.render_widget(Clear, modal);
    let block = overlay_block(model, title, theme.border);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let list = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    // Keep the highlighted entry in view on short terminals.
    let first = selected.saturating_sub(list.height.saturating_sub(1) as usize);
    for (row, (index, entry)) in rows.iter().enumerate().skip(first).enumerate() {
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
                Line::from(Span::styled("▌", fg(entry.accent))),
            );
        }
        let marker = if entry.current { "  current" } else { "" };
        let name_room = (list.width as usize)
            .saturating_sub(3 + marker.len() + 1)
            .min(name_width);
        let name = truncate_text(&entry.name, name_room);
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
                entry.lead.clone(),
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
        hint_line(model, context, inner.width.saturating_sub(2) as usize),
    );
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
