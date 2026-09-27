use super::muted_style;
use super::{centered_rect, render_clear, surface_style};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::App;
use crate::clock::Clock;
use crate::command::{COMMANDS, Context, Group};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

const COLUMN_GAP: u16 = 2;

/// One help section: a group title and its (keys, label) rows, generated
/// from the command table.
struct Section {
    title: &'static str,
    rows: Vec<(String, &'static str)>,
}

fn sections() -> Vec<Section> {
    Group::ALL
        .iter()
        .map(|group| Section {
            title: group.title(),
            rows: COMMANDS
                .iter()
                .filter(|command| command.group == *group && !command.is_paired_into_another())
                .map(|command| (command.keys_label(), command.help_label()))
                .collect(),
        })
        .filter(|section| !section.rows.is_empty())
        .collect()
}

fn section_height(section: &Section) -> usize {
    section.rows.len() + 2
}

fn key_width(sections: &[Section]) -> usize {
    sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|(keys, _)| keys.width())
        .max()
        .unwrap_or(0)
        + 2
}

fn column_width(sections: &[Section]) -> u16 {
    let label_width = sections
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|(_, label)| label.width())
        .max()
        .unwrap_or(0);
    (2 + key_width(sections) + label_width) as u16
}

/// One column of help lines, sized to its own content.
fn help_column(app: &App, sections: &[Section]) -> (Vec<Line<'static>>, u16) {
    let heading = Style::default()
        .fg(app.theme.accent_alt)
        .add_modifier(Modifier::BOLD);
    let keys_width = key_width(sections);
    let mut lines = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(section.title, heading)));
        for (keys, label) in &section.rows {
            let padding = " ".repeat(keys_width.saturating_sub(keys.width()));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {keys}{padding}"),
                    Style::default().fg(app.theme.text),
                ),
                Span::styled(*label, Style::default().fg(app.theme.text)),
            ]));
        }
    }
    (lines, column_width(sections))
}

/// Where to split the sections into two columns so the taller column is
/// as short as possible.
fn balanced_split(sections: &[Section]) -> usize {
    let height = |sections: &[Section]| sections.iter().map(section_height).sum::<usize>();
    (1..sections.len())
        .min_by_key(|&split| height(&sections[..split]).max(height(&sections[split..])))
        .unwrap_or(sections.len())
}

/// The number of lines the help overlay can scroll through at most.
pub(crate) fn line_count() -> usize {
    sections().iter().map(section_height).sum::<usize>() - 1
}

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, app: &App, clock: Clock) {
    let sections = sections();
    let available_width = area.width.saturating_sub(2);
    let (left, right) = sections.split_at(balanced_split(&sections));
    let two_columns = available_width >= column_width(left) + column_width(right) + COLUMN_GAP + 4;
    let columns = if two_columns {
        vec![help_column(app, left), help_column(app, right)]
    } else {
        vec![help_column(app, &sections)]
    };
    let content_height = columns
        .iter()
        .map(|(lines, _)| lines.len())
        .max()
        .unwrap_or(0) as u16;
    let content_width = columns.iter().map(|(_, width)| width).sum::<u16>()
        + COLUMN_GAP * (columns.len() as u16 - 1);
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
    let mut x = body.x;
    for (lines, width) in columns {
        let width = width.min(body.right().saturating_sub(x));
        frame.render_widget(
            Paragraph::new(lines)
                .style(surface_style(app))
                .scroll((scroll, 0)),
            Rect::new(x, body.y, width, body.height),
        );
        x = x.saturating_add(width + COLUMN_GAP);
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
            Line::from(Span::styled(
                format!(" {}", super::hint_text(app, Context::Confirm, usize::MAX)),
                muted_style(app),
            )),
        ])
        .style(Style::default().fg(app.theme.text).bg(app.theme.surface))
        .wrap(Wrap { trim: true }),
        inner,
    );
}
