//! The All tasks view: a section for each column with a grid of cards,
//! scrolled by whole rows so cards are never cut off.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::geometry::{self, HEADER_HEIGHT, Placed};
use super::lanes::{header_line, more, underline};
use super::text::truncate_text;
use super::{card, faint, fg, put};
use crate::app::{FocusRegion, Model};
use crate::clock::Clock;

pub(crate) fn render(frame: &mut Frame<'_>, main: Rect, model: &Model, clock: Clock) {
    let list = geometry::all_tasks(model, main);
    if list.items.is_empty() {
        render_empty(frame, list.list, model);
        return;
    }
    let selected = model
        .selected_task_id()
        .filter(|_| model.ui.focus == FocusRegion::Cards);
    let (placed, hidden_above, hidden_below) = geometry::place_all_tasks(model, &list);
    for item in placed {
        match item {
            Placed::Header { column, area } => {
                put(
                    frame,
                    area.x,
                    area.y,
                    area.width,
                    header_line(model, column, true, area.width),
                );
                if area.height >= HEADER_HEIGHT {
                    put(
                        frame,
                        area.x,
                        area.y + 1,
                        area.width,
                        underline(model, column, true, area.width),
                    );
                }
            }
            Placed::Card { column, task, area } => {
                let task = &model.board.columns[column].tasks[task];
                card::render(
                    frame,
                    area,
                    model,
                    column,
                    task,
                    selected == Some(task.id),
                    clock,
                );
            }
        }
    }
    more(frame, list.above, model, "↑", hidden_above);
    more(frame, list.below, model, "↓", hidden_below);
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
