//! The which-key panel: after Space, or after holding `g` for a moment,
//! a panel above the status line lists the keys that can follow.
//!
//! ```text
//! ── g ───────────────────────────────────────────────
//!  g  first card         b  Backlog        d  Done
//!  i  In Progress
//! ```

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::text::{self, truncate_text};
use super::{faint, fg, fill, muted, put};
use crate::app::Model;
use crate::clock::Clock;
use crate::command::{self, Context};

/// How long `g` is held before its panel appears, so it doesn't flash up
/// for people who type `g g` quickly.
pub const DELAY: Duration = Duration::from_millis(300);

/// Whether the panel shows now.
pub(crate) fn visible(model: &Model, clock: Clock) -> bool {
    model
        .ui
        .pending
        .is_some_and(|pending| pending.prefix == ' ' || clock.instant >= pending.since + DELAY)
}

/// The keys that can follow the pending prefix, as (keys, label).
fn items(model: &Model, context: Context, prefix: char) -> Vec<(String, String)> {
    if prefix == ' ' {
        return command::available(context)
            .filter(|command| {
                command.listed
                    && !command.is_paired_into_another()
                    && command.id != command::CommandId::Leader
                    && model.command_enabled(command.id)
            })
            .map(|command| (command.hint_keys(), command.help_label().to_owned()))
            .collect();
    }
    let mut items: Vec<(String, String)> = command::available(context)
        .flat_map(|command| {
            command
                .keys
                .iter()
                .filter(|key| key.prefix == Some(prefix))
                .map(move |key| {
                    let bare = command::Key {
                        prefix: None,
                        ..*key
                    };
                    (bare.label(), command.label.to_owned())
                })
        })
        .collect();
    // `g` + a lane's initial: the first lane for each letter.
    if prefix == 'g' {
        let mut seen = vec!['g'];
        for column in &model.board.columns {
            let Some(initial) = column.name.chars().next().map(|c| c.to_ascii_lowercase()) else {
                continue;
            };
            if initial.is_alphanumeric() && !seen.contains(&initial) {
                seen.push(initial);
                items.push((initial.to_string(), column.name.clone()));
            }
        }
    }
    items
}

/// Draws the panel across `body`, just above `bottom` (the status line),
/// if it shows.
pub(crate) fn render(frame: &mut Frame<'_>, body: Rect, bottom: u16, model: &Model, clock: Clock) {
    let Some(pending) = model.ui.pending else {
        return;
    };
    if !visible(model, clock) {
        return;
    }
    let theme = &model.ui.theme;
    let items = items(model, model.context(), pending.prefix);
    // Rows between the top bar and the status line, less the title.
    let max_rows = body.height.saturating_sub(5).max(1) as usize;
    let layout = columns(&items, body.width.saturating_sub(2) as usize, max_rows);
    let rows = items.len().div_ceil(layout.len()).max(1);
    // Title row, the items, and a row of padding, leaving the top and
    // status bars.
    let height = ((rows + 2) as u16)
        .min(body.height.saturating_sub(3))
        .max(2);
    let area = Rect::new(body.x, bottom.saturating_sub(height), body.width, height);
    fill(frame, area, Style::default().bg(theme.panel));

    let title = if pending.prefix == ' ' {
        " Space ".to_owned()
    } else {
        format!(" {} ", pending.prefix)
    };
    let rule = "─".repeat(area.width as usize);
    put(
        frame,
        area.x,
        area.y,
        area.width,
        Line::from(Span::styled(rule, fg(theme.border))),
    );
    put(
        frame,
        area.x + 2,
        area.y,
        text::width(&title) as u16,
        Line::from(Span::styled(
            title,
            fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
    );
    let hint = " Esc cancel ";
    put(
        frame,
        area.right().saturating_sub(text::width(hint) as u16 + 2),
        area.y,
        text::width(hint) as u16,
        Line::from(Span::styled(hint, faint(model))),
    );

    let visible_rows = (area.height - 1) as usize;
    let mut x = area.x + 1;
    for (column, (keys_width, label_width)) in layout.iter().enumerate() {
        for row in 0..rows.min(visible_rows) {
            let Some((keys, label)) = items.get(column * rows + row) else {
                break;
            };
            let padding = " ".repeat(keys_width - text::width(keys) + 2);
            put(
                frame,
                x,
                area.y + 1 + row as u16,
                (keys_width + label_width + 3) as u16,
                Line::from(vec![
                    Span::styled(
                        format!(" {keys}"),
                        fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(padding),
                    Span::styled(truncate_text(label, *label_width), muted(model)),
                ]),
            );
        }
        x += (keys_width + label_width + 3 + COLUMN_GAP) as u16;
    }
}

/// The widest a label gets before it is cut.
const LABEL_WIDTH: usize = 30;
const COLUMN_GAP: usize = 2;

/// The most columns (items run down each column in turn) that fit in
/// `width`, as each column's (keys width, label width). When the items
/// need more than `max_rows` rows in the columns that fit, labels are cut
/// to make room for more columns.
fn columns(items: &[(String, String)], width: usize, max_rows: usize) -> Vec<(usize, usize)> {
    let layout = |count: usize, label_cap: usize| -> Vec<(usize, usize)> {
        let rows = items.len().div_ceil(count).max(1);
        items
            .chunks(rows)
            .map(|column| {
                let keys = column.iter().map(|(keys, _)| text::width(keys)).max();
                let labels = column.iter().map(|(_, label)| text::width(label)).max();
                (keys.unwrap_or(0), labels.unwrap_or(0).min(label_cap))
            })
            .collect()
    };
    let total = |layout: &[(usize, usize)]| -> usize {
        layout
            .iter()
            .map(|(keys, label)| keys + label + 3 + COLUMN_GAP)
            .sum()
    };
    let fewest = items.len().div_ceil(max_rows).max(1);
    for count in (fewest..=items.len().clamp(fewest, 6)).rev() {
        let candidate = layout(count, LABEL_WIDTH);
        if total(&candidate) <= width {
            return candidate;
        }
    }
    // Cut the labels so the fewest columns that show everything fit.
    let mut candidate = layout(fewest, LABEL_WIDTH);
    let keys: usize = candidate
        .iter()
        .map(|(keys, _)| keys + 3 + COLUMN_GAP)
        .sum();
    let label_cap = width.saturating_sub(keys) / fewest;
    for (_, label) in &mut candidate {
        *label = (*label).min(label_cap);
    }
    candidate
}
