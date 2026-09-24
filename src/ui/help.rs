use super::{centered_rect, render_clear, surface_style};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use std::time::Instant;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, app: &App, now: Instant) {
    let full_width = 58.min(area.width.saturating_sub(2));
    let full_height = 19.min(area.height.saturating_sub(2));
    let progress = app
        .animations
        .progress(AnimationKind::Modal, now)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let width = ((full_width as f32 * progress).round() as u16).min(area.width);
    let height = ((full_height as f32 * progress).round() as u16).min(area.height);
    if width < 20 || height < 8 {
        return;
    }
    let modal = centered_rect(area, width, height);
    render_clear(frame, modal);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .style(surface_style(app))
        .title(Span::styled(
            " Keyboard shortcuts ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let content = Text::from(vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(app.theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  h / l or ← / →   select column"),
        Line::from("  j / k or ↑ / ↓   select card"),
        Line::from("  H / L            move selected card"),
        Line::from(""),
        Line::from(Span::styled(
            "Tasks",
            Style::default()
                .fg(app.theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  n                new task"),
        Line::from("  e                edit task"),
        Line::from("  d                delete task"),
        Line::from("  Enter            open details"),
        Line::from("  /                search"),
        Line::from(""),
        Line::from(Span::styled(
            "General",
            Style::default()
                .fg(app.theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ?                this help"),
        Line::from("  Esc              close or cancel"),
        Line::from("  q / Ctrl+C       quit"),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to return",
            Style::default().fg(app.theme.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(content)
            .style(Style::default().fg(app.theme.text).bg(app.theme.surface))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

pub(crate) fn render_confirm(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    task_id: uuid::Uuid,
    now: Instant,
) {
    let full_width = 48.min(area.width.saturating_sub(2));
    let full_height = 9.min(area.height.saturating_sub(2));
    let progress = app
        .animations
        .progress(AnimationKind::Modal, now)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let width = ((full_width as f32 * progress).round() as u16).min(area.width);
    let height = ((full_height as f32 * progress).round() as u16).min(area.height);
    if width < 20 || height < 6 {
        return;
    }
    let modal = centered_rect(area, width, height);
    render_clear(frame, modal);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.error))
        .style(surface_style(app))
        .title(Span::styled(
            " Delete task ",
            Style::default()
                .fg(app.theme.error)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let title = app
        .board
        .task(task_id)
        .map(|task| task.title.as_str())
        .unwrap_or("this task");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" Delete {title}?"),
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                " y confirm  •  n cancel",
                Style::default().fg(app.theme.muted),
            )),
        ])
        .style(Style::default().fg(app.theme.text).bg(app.theme.surface))
        .wrap(Wrap { trim: true }),
        inner,
    );
}
