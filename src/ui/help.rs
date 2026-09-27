//! The help overlay, generated from the command table.

use super::{AnimationKind, centered_rect, fade_in, faint, fg, muted, overlay_block, progress};
use crate::app::Model;
use crate::clock::Clock;
use crate::command::{COMMANDS, Group};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
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
fn help_column(model: &Model, sections: &[Section]) -> (Vec<Line<'static>>, u16) {
    let heading = fg(model.ui.theme.accent).add_modifier(Modifier::BOLD);
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
                Span::styled(format!("  {keys}{padding}"), fg(model.ui.theme.text)),
                Span::styled(*label, muted(model)),
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

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, scroll: u16, clock: Clock) {
    let sections = sections();
    let available_width = area.width.saturating_sub(2);
    let (left, right) = sections.split_at(balanced_split(&sections));
    let two_columns = available_width >= column_width(left) + column_width(right) + COLUMN_GAP + 4;
    let columns = if two_columns {
        vec![help_column(model, left), help_column(model, right)]
    } else {
        vec![help_column(model, &sections)]
    };
    let content_height = columns
        .iter()
        .map(|(lines, _)| lines.len())
        .max()
        .unwrap_or(0) as u16;
    let content_width = columns.iter().map(|(_, width)| width).sum::<u16>()
        + COLUMN_GAP * (columns.len() as u16 - 1);
    // Border, a blank line and the footer hint around the content.
    let width = (content_width + 4).min(available_width);
    let height = (content_height + 4).min(area.height.saturating_sub(2));
    let modal = centered_rect(area, width, height);
    frame.render_widget(Clear, modal);
    let block = overlay_block(model, "Keyboard shortcuts", model.ui.theme.border);
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
    let scroll = scroll.min(max_scroll);
    let mut x = body.x;
    for (lines, width) in columns {
        let width = width.min(body.right().saturating_sub(x));
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)),
            Rect::new(x, body.y, width, body.height),
        );
        x = x.saturating_add(width + COLUMN_GAP);
    }

    let hint = if scroll < max_scroll {
        "↓ more   j/k scroll   Esc close"
    } else if max_scroll > 0 {
        "j/k scroll   Esc close"
    } else {
        "Press any key to return"
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, faint(model)))),
        Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        ),
    );
    fade_in(
        frame.buffer_mut(),
        modal,
        model.ui.theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
