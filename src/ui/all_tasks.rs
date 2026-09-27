//! The All tasks view: a section for each column with a grid of cards,
//! scrolled by whole rows so cards are never cut off.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::geometry::{self, HEADER_HEIGHT, Item, all_tasks_offset, visible_end};
use super::lanes::{header_line, more, underline};
use super::text::truncate_text;
use super::{card, faint, fg, put};
use crate::app::{FocusRegion, Model};
use crate::clock::Clock;
use crate::layout::CARD_GAP;

pub(crate) fn render(frame: &mut Frame<'_>, main: Rect, model: &Model, clock: Clock) {
    let list = geometry::all_tasks(model, main);
    if list.items.is_empty() {
        render_empty(frame, list.list, model);
        return;
    }
    let offset = all_tasks_offset(model, &list);
    let end = visible_end(&list.heights, list.list.height, offset);
    let columns = list.card_columns();
    let selected = model
        .selected_task_id()
        .filter(|_| model.ui.focus == FocusRegion::Cards);
    let area = list.list;
    let mut y = area.y;
    for index in offset..end {
        let height = list.heights[index].min(area.bottom().saturating_sub(y));
        match &list.items[index] {
            Item::Header { column } => {
                put(
                    frame,
                    area.x,
                    y,
                    area.width,
                    header_line(model, *column, true, area.width),
                );
                if height >= HEADER_HEIGHT {
                    put(
                        frame,
                        area.x,
                        y + 1,
                        area.width,
                        underline(model, *column, true, area.width),
                    );
                }
            }
            Item::Row { column, tasks } => {
                for (task_index, (x, width)) in tasks.iter().zip(&columns) {
                    let task = &model.board.columns[*column].tasks[*task_index];
                    card::render(
                        frame,
                        Rect::new(area.x + x, y, *width, height),
                        model,
                        *column,
                        task,
                        selected == Some(task.id),
                        clock,
                    );
                }
            }
        }
        y = y.saturating_add(list.heights[index] + CARD_GAP);
    }
    more(frame, list.above, model, "↑", list.card_count(0..offset));
    more(
        frame,
        list.below,
        model,
        "↓",
        list.card_count(end..list.items.len()),
    );
}

/// What to do next when nothing is listed.
fn render_empty(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let query = &model.ui.search.query;
    let line = if query.is_empty() {
        Line::from(Span::styled(
            "No tasks yet · n to add one · ? for all keys",
            faint(model),
        ))
    } else {
        Line::from(vec![
            Span::styled("No matches for ", faint(model)),
            Span::styled(
                format!("\"{}\"", truncate_text(query, 30)),
                fg(model.ui.theme.text),
            ),
            Span::styled(" · Esc to clear", faint(model)),
        ])
    };
    put(frame, area.x, area.y, area.width, line);
}
