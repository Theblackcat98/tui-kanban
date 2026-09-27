use super::muted_style;
use super::{centered_rect, render_clear, surface_style};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::App;
use crate::clock::Clock;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("v", "toggle Board / All tasks"),
            ("Tab", "focus rail / cards"),
            ("h / l or ← / →", "select column"),
            ("j / k or ↑ / ↓", "select card"),
            ("Home / End", "first / last card"),
            ("PageUp / PageDn", "move by five cards"),
        ],
    ),
    (
        "Tasks",
        &[
            ("n", "new task"),
            ("e", "edit task"),
            ("d", "delete task"),
            ("H / L", "move task to column"),
            ("Enter", "open details"),
            ("/", "search"),
        ],
    ),
    (
        "Details",
        &[
            ("PageUp / PageDn", "scroll description"),
            ("H / L", "move task from drawer"),
        ],
    ),
    (
        "General",
        &[
            ("?", "this help"),
            ("Esc", "close or cancel"),
            ("q / Ctrl+C", "quit"),
        ],
    ),
];

const KEY_WIDTH: usize = 17;
const COLUMN_WIDTH: u16 = 44;
const COLUMN_GAP: u16 = 2;

/// The help lines as one column, or as two columns side by side.
fn help_columns(app: &App, columns: usize) -> Vec<Vec<Line<'static>>> {
    let heading = Style::default()
        .fg(app.theme.accent_alt)
        .add_modifier(Modifier::BOLD);
    let split = if columns == 2 {
        balanced_split()
    } else {
        SECTIONS.len()
    };
    let (left, right) = SECTIONS.split_at(split);
    [left, right]
        .into_iter()
        .filter(|sections| !sections.is_empty())
        .map(|sections| {
            let mut lines = Vec::new();
            for (index, (title, keys)) in sections.iter().enumerate() {
                if index > 0 {
                    lines.push(Line::default());
                }
                lines.push(Line::from(Span::styled(*title, heading)));
                for (key, description) in *keys {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {key:<KEY_WIDTH$}"),
                            Style::default().fg(app.theme.text),
                        ),
                        Span::styled(*description, Style::default().fg(app.theme.text)),
                    ]));
                }
            }
            lines
        })
        .collect()
}

/// Where to split the sections into two columns so the taller column is
/// as short as possible.
fn balanced_split() -> usize {
    let height = |sections: &[(&str, &[(&str, &str)])]| {
        sections
            .iter()
            .map(|(_, keys)| keys.len() + 2)
            .sum::<usize>()
    };
    (1..SECTIONS.len())
        .min_by_key(|&split| height(&SECTIONS[..split]).max(height(&SECTIONS[split..])))
        .unwrap_or(SECTIONS.len())
}

/// The number of lines the help overlay can scroll through at most.
pub(crate) fn line_count() -> usize {
    SECTIONS
        .iter()
        .map(|(_, keys)| keys.len() + 2)
        .sum::<usize>()
        - 1
}

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, app: &App, clock: Clock) {
    let available_width = area.width.saturating_sub(2);
    let two_columns = available_width >= COLUMN_WIDTH * 2 + COLUMN_GAP + 4;
    let columns = help_columns(app, if two_columns { 2 } else { 1 });
    let content_height = columns.iter().map(Vec::len).max().unwrap_or(0) as u16;
    let content_width = if two_columns {
        COLUMN_WIDTH * 2 + COLUMN_GAP
    } else {
        COLUMN_WIDTH
    };
    // Border, a blank line and the footer hint around the content.
    let full_width = (content_width + 4).min(available_width);
    let full_height = (content_height + 4).min(area.height.saturating_sub(2));
    let progress = app
        .animations
        .progress(AnimationKind::Modal, clock.instant)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let width = ((full_width as f32 * progress).round() as u16).min(area.width);
    let height = ((full_height as f32 * progress).round() as u16).min(area.height);
    if width < 20 || height < 5 {
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
    let inner = Rect::new(
        inner.x.saturating_add(1),
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    let body = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let max_scroll = content_height.saturating_sub(body.height);
    let scroll = app.help_scroll.min(max_scroll);
    let column_width = if two_columns {
        (body.width.saturating_sub(COLUMN_GAP)) / 2
    } else {
        body.width
    };
    for (index, lines) in columns.into_iter().enumerate() {
        let x = body.x + (column_width + COLUMN_GAP) * index as u16;
        frame.render_widget(
            Paragraph::new(lines)
                .style(surface_style(app))
                .scroll((scroll, 0)),
            Rect::new(x, body.y, column_width, body.height),
        );
    }

    let hint = if scroll < max_scroll {
        "↓ more  •  j/k scroll  •  esc close"
    } else if max_scroll > 0 {
        "j/k scroll  •  esc close"
    } else {
        "Press any key to return"
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, muted_style(app)))).style(surface_style(app)),
        Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        ),
    );
}

pub(crate) fn render_confirm(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    task_id: uuid::Uuid,
    clock: Clock,
) {
    let full_width = 48.min(area.width.saturating_sub(2));
    let full_height = 9.min(area.height.saturating_sub(2));
    let progress = app
        .animations
        .progress(AnimationKind::Modal, clock.instant)
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
            Line::from(Span::styled(" y confirm  •  n cancel", muted_style(app))),
        ])
        .style(Style::default().fg(app.theme.text).bg(app.theme.surface))
        .wrap(Wrap { trim: true }),
        inner,
    );
}
