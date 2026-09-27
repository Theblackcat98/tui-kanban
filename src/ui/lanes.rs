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

use super::geometry::{self, LaneParts, Page, lane_heights, lane_offset, lane_parts, visible_end};
use super::text::{self, truncate_text};
use super::{blend_color, card, faint, fg, muted, put};
use crate::app::{FocusRegion, Model};
use crate::clock::Clock;
use crate::layout::CARD_GAP;

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
    for (column, area) in geometry::lanes(model, page) {
        render_lane(frame, area, model, column, clock);
    }
    let range = geometry::lane_range(model, page);
    let row = geometry::lane_overflow_row(page);
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

/// A column's task count, as "3", or "2/5" while filtering.
pub(crate) fn count_label(model: &Model, column: usize) -> String {
    let total = model.board.columns[column].tasks.len();
    if model.ui.search.query.is_empty() {
        total.to_string()
    } else {
        format!("{}/{total}", model.visible_task_indices(column).len())
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
        Span::styled(format!("  {count}"), faint(model)),
    ])
}

/// The `▔` underline in the lane's colour, dimmed unless it is focused.
pub(crate) fn underline(model: &Model, column: usize, focused: bool, width: u16) -> Line<'static> {
    let theme = &model.ui.theme;
    let lane = theme.lane(column);
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

fn render_lane(frame: &mut Frame<'_>, area: Rect, model: &Model, column: usize, clock: Clock) {
    let parts = lane_parts(area);
    let focused = model.ui.focus == FocusRegion::Cards && model.ui.active_column == column;
    put(
        frame,
        parts.header.x,
        parts.header.y,
        parts.header.width * parts.header.height,
        header_line(model, column, focused, area.width),
    );
    put(
        frame,
        parts.underline.x,
        parts.underline.y,
        parts.underline.width * parts.underline.height,
        underline(model, column, focused, area.width),
    );

    let visible = model.visible_task_indices(column);
    if visible.is_empty() {
        render_empty(frame, &parts, model, focused);
        return;
    }
    let heights = lane_heights(model, column, area.width);
    let offset = lane_offset(model, column, area);
    let end = visible_end(&heights, parts.cards.height, offset);
    let selected = model.selected_task_id();
    let tasks = &model.board.columns[column].tasks;
    let mut y = parts.cards.y;
    for index in offset..end {
        let task = &tasks[visible[index]];
        let height = heights[index].min(parts.cards.bottom().saturating_sub(y));
        card::render(
            frame,
            Rect::new(area.x, y, area.width, height),
            model,
            column,
            task,
            focused && selected == Some(task.id),
            clock,
        );
        y = y.saturating_add(heights[index] + CARD_GAP);
    }
    more(frame, parts.above, model, "↑", offset);
    more(frame, parts.below, model, "↓", visible.len() - end);
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

fn render_empty(frame: &mut Frame<'_>, parts: &LaneParts, model: &Model, focused: bool) {
    let filtering = !model.ui.search.query.is_empty();
    let mut lines = vec![if filtering {
        "No matches"
    } else {
        "No tasks yet"
    }];
    if focused && !filtering {
        lines.extend(["n to add one", "? for all keys"]);
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
