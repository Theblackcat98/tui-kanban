//! The Board view: a lane for each column, separated by space.
//!
//! ```text
//! Backlog  3
//! ▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔
//! ↑ 1 more
//! ▎ Write the README
//! ```

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::geometry::{self, LaneParts, Page, lane_parts};
use super::text::{self, truncate_text};
use super::{blend_color, card, faint, fg, muted, put};
use crate::app::{FocusRegion, Model};
use crate::clock::Clock;
use crate::command::{CommandId, key_label};
use crate::domain::Wip;
use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn render(frame: &mut Frame<'_>, page: &Page, model: &Model, clock: Clock) {
    if model.board.columns.is_empty() {
        let area = page.main;
        put(
            frame,
            area.x + 2,
            area.y + 1,
            area.width.saturating_sub(4),
            Line::from(Span::styled("This board has no columns.", muted(model))),
        );
        return;
    }
    // At the Compact breakpoint the one lane's header is a tab strip that
    // also names the other columns.
    let tabs = !page.breakpoint.shows_several_lanes();
    for (column, area) in geometry::lanes(model, page) {
        if !tabs && model.board.columns[column].collapsed {
            render_strip(frame, area, model, column);
            continue;
        }
        render_lane(frame, area, model, column, !tabs, clock);
        if tabs {
            render_tabs(frame, lane_parts(area).header, model);
        }
    }
    if tabs {
        return;
    }
    let row = geometry::lane_overflow_row(page);
    let range = geometry::lane_range(model, page);
    if range.start > 0 {
        put(
            frame,
            row.x,
            row.y,
            row.width,
            Line::from(Span::styled(
                format!("‹ {} more", range.start),
                faint(model),
            )),
        );
    }
    let after = model.board.columns.len() - range.end;
    if after > 0 {
        let text = format!("{after} more ›");
        let width = text::width(&text) as u16;
        put(
            frame,
            row.right().saturating_sub(width),
            row.y,
            width,
            Line::from(Span::styled(text, faint(model))),
        );
    }
}

/// The longest a column name gets in the tab strip.
const TAB_NAME_WIDTH: usize = 16;

/// The tab strip that heads the single lane at the Compact breakpoint,
/// naming the columns around the active one:
///
/// ```text
/// ‹ Backlog 3 · In Progress 2 · Done 5 ›
/// ```
fn render_tabs(frame: &mut Frame<'_>, row: Rect, model: &Model) {
    if row.is_empty() {
        return;
    }
    let tabs = tab_labels(model);
    let (start, end) = tab_window(&tabs, model.ui.active_column, row.width as usize);
    let mut spans = Vec::new();
    if start > 0 {
        spans.push(Span::styled("‹ ", faint(model)));
    }
    for (index, (name, count)) in tabs.iter().enumerate().take(end).skip(start) {
        if index > start {
            spans.push(Span::styled(" · ", faint(model)));
        }
        if index == model.ui.active_column {
            let theme = &model.ui.theme;
            let style = fg(super::lane_color(model, index))
                .add_modifier(Modifier::BOLD | theme.selected_modifier);
            spans.push(Span::styled(format!("{name} {count}"), style));
        } else {
            spans.push(Span::styled(name.clone(), muted(model)));
            spans.push(Span::styled(format!(" {count}"), count_style(model, index)));
        }
    }
    if end < tabs.len() {
        spans.push(Span::styled(" ›", faint(model)));
    }
    put(frame, row.x, row.y, row.width, Line::from(spans));
}

fn tab_labels(model: &Model) -> Vec<(String, String)> {
    model
        .board
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            (
                truncate_text(&column.name, TAB_NAME_WIDTH),
                count_label(model, index),
            )
        })
        .collect()
}

/// The tabs that fit in `width` cells: the active one, then its
/// neighbours, alternating right and left, while they fit. Room is kept
/// for the arrows at each end.
fn tab_window(tabs: &[(String, String)], active: usize, width: usize) -> (usize, usize) {
    if tabs.is_empty() {
        return (0, 0);
    }
    let tab_width = |index: usize| {
        let (name, count) = &tabs[index];
        text::width(name) + 1 + count.len()
    };
    let active = active.min(tabs.len() - 1);
    let room = width.saturating_sub(4);
    let (mut start, mut end) = (active, active + 1);
    let mut used = tab_width(active);
    loop {
        let mut grew = false;
        for right in [true, false] {
            let candidate = if right {
                (end < tabs.len()).then_some(end)
            } else {
                start.checked_sub(1)
            };
            let Some(index) = candidate else { continue };
            let extra = 3 + tab_width(index);
            if used + extra <= room {
                used += extra;
                if right {
                    end += 1;
                } else {
                    start -= 1;
                }
                grew = true;
            }
        }
        if !grew {
            return (start, end);
        }
    }
}

/// A column's task count: "3", "3/4" against a work-in-progress limit of
/// 4, or "2/5" (matches of all) while filtering.
pub(crate) fn count_label(model: &Model, column: usize) -> String {
    let data = &model.board.columns[column];
    let total = data.tasks.len();
    if !model.ui.search.query.is_empty() {
        format!("{}/{total}", model.matching_count(column))
    } else if let Some(limit) = data.limit() {
        format!("{total}/{limit}")
    } else {
        total.to_string()
    }
}

/// How a count is drawn: faint, or `warning` once the column is at its
/// work-in-progress limit and `danger` past it (bold and reversed without
/// colour).
pub(crate) fn count_style(model: &Model, column: usize) -> Style {
    let theme = &model.ui.theme;
    let wip = model.board.columns[column].wip();
    match wip {
        Wip::Under => faint(model),
        _ if theme.is_monochrome() => Style::default().add_modifier(if wip == Wip::Over {
            Modifier::BOLD | Modifier::REVERSED
        } else {
            Modifier::BOLD
        }),
        Wip::Full => fg(theme.warning).add_modifier(Modifier::BOLD),
        Wip::Over => fg(theme.danger).add_modifier(Modifier::BOLD),
    }
}

/// A lane or section header: the name, bold, and the count.
pub(crate) fn header_line(
    model: &Model,
    column: usize,
    focused: bool,
    width: u16,
) -> Line<'static> {
    let theme = &model.ui.theme;
    let count = count_label(model, column);
    let name_room = (width as usize).saturating_sub(count.len() + 2);
    let name_style = if focused {
        fg(theme.text)
    } else {
        muted(model)
    };
    Line::from(vec![
        Span::styled(
            truncate_text(&model.board.columns[column].name, name_room),
            name_style.add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {count}"), count_style(model, column)),
    ])
}

/// The `▔` underline in the lane's colour, dimmed unless it is focused.
pub(crate) fn underline(model: &Model, column: usize, focused: bool, width: u16) -> Line<'static> {
    let theme = &model.ui.theme;
    let lane = super::lane_color(model, column);
    let style = if theme.is_monochrome() {
        if focused {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        }
    } else if focused {
        fg(lane)
    } else {
        fg(blend_color(theme.bg, lane, 0.45))
    };
    Line::from(Span::styled("▔".repeat(width as usize), style))
}

fn render_lane(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    column: usize,
    header: bool,
    clock: Clock,
) {
    let parts = lane_parts(area);
    let focused = model.ui.focus == FocusRegion::Cards && model.ui.active_column == column;
    if header {
        put(
            frame,
            parts.header.x,
            parts.header.y,
            parts.header.width * parts.header.height,
            header_line(model, column, focused, area.width),
        );
    }
    put(
        frame,
        parts.underline.x,
        parts.underline.y,
        parts.underline.width * parts.underline.height,
        underline(model, column, focused, area.width),
    );

    if model.visible_task_indices(column).is_empty() {
        render_empty(frame, &parts, model, column, focused);
        return;
    }
    let lane = geometry::lane_cards(model, column, area);
    let selected = model.selected_task_id();
    let tasks = &model.board.columns[column].tasks;
    for (index, card_area) in lane.cards {
        let task = &tasks[index];
        card::render(
            frame,
            card_area,
            model,
            column,
            task,
            focused && selected == Some(task.id),
            clock,
        );
    }
    more(frame, parts.above, model, "↑", lane.hidden_above);
    more(frame, parts.below, model, "↓", lane.hidden_below);
}

/// "↑ 2 more" or "↓ 3 more", when cards are hidden.
pub(crate) fn more(frame: &mut Frame<'_>, area: Rect, model: &Model, arrow: &str, hidden: usize) {
    if hidden > 0 && !area.is_empty() {
        put(
            frame,
            area.x,
            area.y,
            area.width,
            Line::from(Span::styled(format!("{arrow} {hidden} more"), faint(model))),
        );
    }
}

/// A collapsed lane, as a strip: its count, its underline, and its name
/// written downwards.
///
/// ```text
///  5
/// ▔▔▔
///
///  D
///  o
///  n
///  e
/// ```
fn render_strip(frame: &mut Frame<'_>, area: Rect, model: &Model, column: usize) {
    let parts = lane_parts(area);
    let focused = model.ui.focus == FocusRegion::Cards && model.ui.active_column == column;
    let data = &model.board.columns[column];
    let count = data.tasks.len().to_string();
    let count_width = text::width(&count) as u16;
    put(
        frame,
        area.x + area.width.saturating_sub(count_width) / 2,
        parts.header.y,
        area.width,
        Line::from(Span::styled(count, count_style(model, column))),
    );
    if parts.underline.height > 0 {
        put(
            frame,
            area.x,
            parts.underline.y,
            area.width,
            underline(model, column, focused, area.width),
        );
    }
    let theme = &model.ui.theme;
    let style = if focused {
        fg(theme.text).add_modifier(Modifier::BOLD)
    } else {
        muted(model)
    };
    let rows = parts.cards.height as usize;
    let letters: Vec<&str> = data.name.graphemes(true).collect();
    let cut = letters.len() > rows;
    for (row, letter) in letters.iter().take(rows).enumerate() {
        let letter = if cut && row + 1 == rows {
            "…"
        } else {
            letter
        };
        put(
            frame,
            area.x + 1,
            parts.cards.y + row as u16,
            area.width.saturating_sub(1),
            Line::from(Span::styled(letter.to_owned(), style)),
        );
    }
}

fn render_empty(
    frame: &mut Frame<'_>,
    parts: &LaneParts,
    model: &Model,
    column: usize,
    focused: bool,
) {
    let filtering = !model.ui.search.query.is_empty();
    let hidden = model.lane_hidden(column);
    let key = |id: CommandId, text: &str| key_label(id).map(|key| format!("{key} {text}"));
    let first = if hidden {
        "Collapsed"
    } else if filtering {
        "No matches"
    } else {
        "No tasks yet"
    };
    let mut lines = vec![first.to_owned()];
    if hidden {
        lines.extend(key(CommandId::ToggleCollapse, "to expand"));
    } else if focused && !filtering {
        lines.extend(key(CommandId::NewTask, "to add one"));
        lines.extend(key(CommandId::Help, "for all keys"));
    }
    let area = parts.cards;
    for (row, text) in lines.into_iter().enumerate() {
        if row as u16 >= area.height {
            break;
        }
        put(
            frame,
            area.x,
            area.y + row as u16,
            area.width,
            Line::from(Span::styled(text, faint(model))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(names: &[&str]) -> Vec<(String, String)> {
        names
            .iter()
            .map(|name| (name.to_string(), "1".to_owned()))
            .collect()
    }

    #[test]
    fn tab_window_centres_on_the_active_tab() {
        let tabs = tabs(&["aaaa", "bbbb", "cccc", "dddd", "eeee"]);
        // Each tab is 6 cells, plus 3 between them and 4 for the arrows.
        assert_eq!(tab_window(&tabs, 0, 100), (0, 5));
        assert_eq!(tab_window(&tabs, 2, 4 + 6 + 9 + 9), (1, 4));
        assert_eq!(tab_window(&tabs, 4, 4 + 6 + 9), (3, 5));
        assert_eq!(tab_window(&tabs, 0, 4 + 6 + 9), (0, 2));
        // The active tab is always shown, even when nothing fits.
        assert_eq!(tab_window(&tabs, 3, 0), (3, 4));
        assert_eq!(tab_window(&[], 0, 50), (0, 0));
    }
}
