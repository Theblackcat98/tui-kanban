use super::{render_clear, short_id, surface_style};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use std::time::Instant;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    task_id: uuid::Uuid,
    now: Instant,
) {
    let full_width = if area.width < 56 {
        area.width
    } else {
        (area.width * 2 / 5).clamp(32, 54)
    };
    let progress = app
        .animations
        .progress(AnimationKind::Drawer, now)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let width = ((full_width as f32 * progress).round() as u16).min(area.width);
    if width == 0 {
        return;
    }
    let drawer = Rect::new(
        area.x + area.width.saturating_sub(width),
        area.y,
        width,
        area.height,
    );
    render_clear(frame, drawer);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(surface_style(app))
        .title(Span::styled(
            " Task details ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(drawer);
    frame.render_widget(block, drawer);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(task) = app.board.task(task_id) else {
        frame.render_widget(
            Paragraph::new("This task no longer exists.")
                .style(Style::default().fg(app.theme.muted).bg(app.theme.surface)),
            inner,
        );
        return;
    };
    let column_name = app
        .board
        .columns
        .iter()
        .find_map(|column| {
            column
                .tasks
                .iter()
                .position(|item| item.id == task_id)
                .map(|_| column.name.as_str())
        })
        .unwrap_or("Unknown column");
    let sections = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", task.title),
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        )))
        .style(surface_style(app))
        .wrap(Wrap { trim: true }),
        sections[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}  •  {}", column_name, short_id(task.id)),
            Style::default().fg(app.theme.accent_alt),
        )))
        .style(surface_style(app)),
        sections[1],
    );
    let description = if task.description.trim().is_empty() {
        "No description".to_owned()
    } else {
        task.description.clone()
    };
    let description_style = Style::default()
        .fg(if task.description.trim().is_empty() {
            app.theme.muted
        } else {
            app.theme.text
        })
        .bg(app.theme.surface);
    frame.render_widget(
        Paragraph::new(description)
            .style(description_style)
            .wrap(Wrap { trim: true }),
        sections[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " e edit  •  d delete  •  esc close",
            Style::default().fg(app.theme.muted),
        )))
        .style(surface_style(app)),
        sections[3],
    );
}
