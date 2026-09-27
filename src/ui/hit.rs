//! What is under the mouse, worked out from the same geometry the
//! renderer draws with, so a click always lands on what is on screen.

use ratatui::layout::{Position, Rect};

use super::bars;
use super::geometry::{self, Page, Placed, lane_parts};
use crate::app::{DropTarget, Hit, Marker, Model, Screen, ViewMode};
use crate::layout::{self, CARD_GAP, LANE_GUTTER};

fn page(model: &Model) -> Option<Page> {
    let (width, height) = model.ui.viewport;
    layout::fits(width, height).then(|| geometry::page(model, Rect::new(0, 0, width, height)))
}

/// What is at (x, y). With an overlay open, only the status line and
/// the detail drawer (and the board beside it) respond.
pub(crate) fn hit(model: &Model, x: u16, y: u16) -> Option<Hit> {
    let page = page(model)?;
    let position = Position::new(x, y);
    if page.status.contains(position) {
        return bars::status_hints(model, page.status)
            .into_iter()
            .find(|(area, _)| area.contains(position))
            .map(|(_, id)| Hit::Hint(id));
    }
    match model.ui.screens.last() {
        Some(Screen::Detail { .. }) => {
            if page.drawer.is_some_and(|drawer| drawer.contains(position)) {
                return Some(Hit::Drawer);
            }
        }
        Some(_) => return None,
        None => {}
    }
    if page.top.contains(position) {
        return bars::view_tabs(model, page.top)
            .into_iter()
            .find(|(area, _)| area.contains(position))
            .map(|(_, view)| Hit::View(view));
    }
    if let Some(rail) = page.rail
        && rail.contains(position)
    {
        return rail_entry(model, rail, y).map(|column| Hit::Rail { column });
    }
    match model.ui.view {
        ViewMode::Board => board_hit(model, &page, position),
        ViewMode::AllTasks => all_tasks_hit(model, page.main, position),
    }
}

/// The column of the rail entry on row `y`.
fn rail_entry(model: &Model, rail: Rect, y: u16) -> Option<usize> {
    let column = y.checked_sub(rail.y + 3)? as usize;
    (column < model.board.columns.len()).then_some(column)
}

fn board_hit(model: &Model, page: &Page, position: Position) -> Option<Hit> {
    let overflow = geometry::lane_overflow_row(page);
    if overflow.contains(position) && page.breakpoint.shows_several_lanes() {
        let range = geometry::lane_range(model, page);
        let middle = overflow.x + overflow.width / 2;
        if range.start > 0 && position.x < middle {
            return Some(Hit::MoreLeft);
        }
        if range.end < model.board.columns.len() && position.x >= middle {
            return Some(Hit::MoreRight);
        }
        return None;
    }
    let (column, area) = geometry::lanes(model, page)
        .into_iter()
        .find(|(_, area)| area.contains(position))?;
    if model.board.columns[column].collapsed && page.breakpoint.shows_several_lanes() {
        return Some(Hit::Strip { column });
    }
    let tasks = &model.board.columns[column].tasks;
    let card = geometry::lane_cards(model, column, area)
        .cards
        .into_iter()
        .find(|(_, card)| card.contains(position));
    Some(match card {
        Some((index, _)) => Hit::Card {
            id: tasks[index].id,
        },
        None => Hit::Lane { column },
    })
}

fn all_tasks_hit(model: &Model, main: Rect, position: Position) -> Option<Hit> {
    let list = geometry::all_tasks(model, main);
    let (placed, _, _) = geometry::place_all_tasks(model, &list);
    placed.into_iter().find_map(|item| match item {
        Placed::Header { column, area } if area.contains(position) => Some(Hit::Lane { column }),
        Placed::Card { column, task, area } if area.contains(position) => Some(Hit::Card {
            id: model.board.columns[column].tasks[task].id,
        }),
        _ => None,
    })
}

/// The columns shown as lanes, as the renderer lays them out now.
pub(crate) fn lane_range(model: &Model) -> std::ops::Range<usize> {
    page(model).map_or(0..0, |page| geometry::lane_range(model, &page))
}

/// Where a card dragged to (x, y) would be dropped: before or after the
/// card under the pointer, at the end of a lane, or at the end of a
/// column named in the rail or collapsed to a strip.
pub(crate) fn drop_target(model: &Model, x: u16, y: u16) -> Option<DropTarget> {
    let page = page(model)?;
    let position = Position::new(x, y);
    if let Some(rail) = page.rail
        && rail.contains(position)
    {
        let column = rail_entry(model, rail, y)?;
        return Some(DropTarget {
            column,
            index: None,
            marker: Marker::Bar(Rect::new(rail.x + 1, y, 1, 1)),
        });
    }
    if !page.main.contains(position) {
        return None;
    }
    match model.ui.view {
        ViewMode::Board => board_drop(model, &page, position),
        ViewMode::AllTasks => all_tasks_drop(model, page.main, position),
    }
}

fn board_drop(model: &Model, page: &Page, position: Position) -> Option<DropTarget> {
    // The lane under the pointer, counting the gutter after it, so there
    // is no dead space between lanes.
    let (column, area) = geometry::lanes(model, page)
        .into_iter()
        .find(|(_, area)| position.x >= area.x && position.x < area.right() + LANE_GUTTER)?;
    let parts = lane_parts(area);
    let line = |y: u16| {
        let y = y.clamp(parts.underline.y + 1, area.bottom().saturating_sub(1));
        Marker::Line(Rect::new(area.x, y, area.width, 1))
    };
    if model.lane_hidden(column) {
        return Some(DropTarget {
            column,
            index: None,
            marker: line(parts.cards.y),
        });
    }
    let lane = geometry::lane_cards(model, column, area);
    // Before the first card whose middle is below the pointer.
    if let Some((index, card)) = lane
        .cards
        .iter()
        .find(|(_, card)| position.y < card.y + card.height / 2)
    {
        return Some(DropTarget {
            column,
            index: Some(*index),
            marker: line(card.y.saturating_sub(1)),
        });
    }
    // After the last card on screen: the end, unless more are hidden
    // below it.
    Some(match lane.cards.last() {
        Some((index, card)) => DropTarget {
            column,
            index: (lane.hidden_below > 0).then_some(index + 1),
            marker: line(card.bottom()),
        },
        None => DropTarget {
            column,
            index: None,
            marker: line(parts.cards.y),
        },
    })
}

fn all_tasks_drop(model: &Model, main: Rect, position: Position) -> Option<DropTarget> {
    let list = geometry::all_tasks(model, main);
    let (placed, _, _) = geometry::place_all_tasks(model, &list);
    let bar = |x: u16, card: Rect| Marker::Bar(Rect::new(x, card.y, 1, card.height));
    // The row under the pointer, counting the blank row below it.
    let in_row = |area: Rect| position.y >= area.y && position.y < area.bottom() + CARD_GAP;
    let mut last_in_row = None;
    for item in &placed {
        match *item {
            Placed::Header { column, area } if in_row(area) => {
                let first = model.visible_task_indices(column).first().copied();
                return Some(DropTarget {
                    column,
                    index: first,
                    marker: Marker::Line(Rect::new(area.x, area.bottom(), area.width, 1)),
                });
            }
            Placed::Card { column, task, area } if in_row(area) => {
                if position.x < area.x + area.width / 2 {
                    return Some(DropTarget {
                        column,
                        index: Some(task),
                        marker: bar(area.x.saturating_sub(1), area),
                    });
                }
                last_in_row = Some((column, task, area));
            }
            _ => {}
        }
    }
    // Right of a card's middle: after the last card passed in that row.
    let (column, task, area) = last_in_row?;
    Some(DropTarget {
        column,
        index: Some(task + 1),
        marker: bar(area.right(), area),
    })
}
