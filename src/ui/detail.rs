//! The detail drawer: the whole task, on `panel` with an accent edge.
//!
//! It is drawn at full width into its own buffer and slides in from the
//! right, so its text never reflows while it opens.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};
use uuid::Uuid;

use super::geometry;
use super::text::{relative_time, short_date, truncate_text};
use super::{AnimationKind, faint, fg, muted, progress, short_id};
use crate::app::Model;
use crate::clock::Clock;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    drawer: Rect,
    model: &Model,
    task: Uuid,
    scroll: u16,
    clock: Clock,
) {
    let drawer = drawer.intersection(frame.area());
    if drawer.is_empty() {
        return;
    }
    let mut buffer = Buffer::empty(Rect::new(0, 0, drawer.width, drawer.height));
    draw(&mut buffer, model, task, scroll, clock);

    let shown =
        (f32::from(drawer.width) * progress(model, AnimationKind::Drawer, clock)).round() as u16;
    let left = drawer.right() - shown.min(drawer.width);
    let target = frame.buffer_mut();
    for y in 0..drawer.height {
        for x in 0..shown.min(drawer.width) {
            target[(left + x, drawer.y + y)] = buffer[(x, y)].clone();
        }
    }
}

fn draw(buffer: &mut Buffer, model: &Model, task_id: Uuid, scroll: u16, clock: Clock) {
    let theme = &model.ui.theme;
    let area = buffer.area;
    Block::default()
        .style(Style::default().bg(theme.panel))
        .render(area, buffer);
    for y in area.top()..area.bottom() {
        buffer[(area.x, y)]
            .set_symbol("▎")
            .set_style(fg(theme.accent));
    }

    let Some(task) = model.board.task(task_id) else {
        line(
            buffer,
            Rect::new(3, 1, area.width.saturating_sub(6), 1),
            Line::from(Span::styled("This task no longer exists.", muted(model))),
        );
        return;
    };
    let layout = geometry::detail(task, area);
    for (row, title) in layout.title.iter().enumerate() {
        line(
            buffer,
            Rect::new(
                layout.title_area.x,
                layout.title_area.y + row as u16,
                layout.title_area.width,
                1,
            ),
            Line::from(Span::styled(
                title.clone(),
                fg(theme.text).add_modifier(Modifier::BOLD),
            )),
        );
    }

    let column = model
        .board
        .task_location(task_id)
        .map(|(column, _)| model.board.columns[column].name.as_str())
        .unwrap_or("Unknown column");
    let meta = [
        format!("{column} · #{}", short_id(task.id)),
        format!(
            "updated {} · created {}",
            relative_time(task.updated_at, clock.wall_millis),
            short_date(task.created_at, clock.wall_millis),
        ),
    ];
    for (row, text) in meta.iter().enumerate().take(layout.meta.height as usize) {
        line(
            buffer,
            Rect::new(
                layout.meta.x,
                layout.meta.y + row as u16,
                layout.meta.width,
                1,
            ),
            Line::from(Span::styled(
                truncate_text(text, layout.meta.width as usize),
                faint(model),
            )),
        );
    }

    let body = layout.body;
    if layout.description.is_empty() {
        line(
            buffer,
            body,
            Line::from(Span::styled("No description", faint(model))),
        );
        return;
    }
    let scroll = scroll.min(layout.max_scroll());
    let lines: Vec<Line> = layout
        .description
        .iter()
        .skip(scroll as usize)
        .take(body.height as usize)
        .map(|text| Line::from(Span::styled(text.clone(), fg(theme.text))))
        .collect();
    Paragraph::new(lines).render(body, buffer);
    if layout.max_scroll() > 0 {
        let mut state = ScrollbarState::new(layout.max_scroll() as usize + 1)
            .position(scroll as usize)
            .viewport_content_length(body.height as usize);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .track_style(faint(model))
            .thumb_symbol("┃")
            .thumb_style(fg(theme.text_muted))
            .render(layout.scrollbar, buffer, &mut state);
    }
}

fn line(buffer: &mut Buffer, area: Rect, line: Line<'_>) {
    let area = area.intersection(buffer.area);
    if !area.is_empty() {
        Paragraph::new(line).render(Rect { height: 1, ..area }, buffer);
    }
}
