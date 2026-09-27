//! The column rail on the left, at the Regular and Wide breakpoints.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::lanes::count_label;
use super::text::{self, truncate_text};
use super::{faint, fg, fill, muted, put};
use crate::app::{FocusRegion, Model};

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    if area.height < 2 {
        return;
    }
    put(
        frame,
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(3),
        Line::from(Span::styled(
            "Columns",
            faint(model).add_modifier(Modifier::BOLD),
        )),
    );
    if model.board.columns.is_empty() {
        put(
            frame,
            area.x + 2,
            area.y + 3,
            area.width.saturating_sub(3),
            Line::from(Span::styled("No columns", faint(model))),
        );
        return;
    }

    let rail_focused = model.ui.focus == FocusRegion::Rail;
    // The entries end one cell before the rail's edge, so the counts
    // don't touch the lanes.
    let width = area.width.saturating_sub(1);
    for (index, column) in model.board.columns.iter().enumerate() {
        let y = area.y + 3 + index as u16;
        if y >= area.bottom() {
            break;
        }
        let active = index == model.ui.active_column;
        if active && rail_focused {
            fill(
                frame,
                Rect::new(area.x, y, width, 1),
                Style::default()
                    .bg(theme.selection)
                    .add_modifier(theme.selected_modifier),
            );
        }
        if active {
            put(
                frame,
                area.x + 1,
                y,
                1,
                Line::from(Span::styled("▌", fg(theme.lane(index)))),
            );
        }
        let count = count_label(model, index);
        let count_width = text::width(&count) as u16;
        let count_x = (area.x + width).saturating_sub(count_width + 1);
        put(
            frame,
            count_x,
            y,
            count_width,
            Line::from(Span::styled(count, faint(model))),
        );
        let name_x = area.x + 3;
        let name_room = count_x.saturating_sub(name_x + 1);
        let name_style = if active {
            fg(theme.text).add_modifier(Modifier::BOLD)
        } else {
            muted(model)
        };
        put(
            frame,
            name_x,
            y,
            name_room,
            Line::from(Span::styled(
                truncate_text(&column.name, name_room as usize),
                name_style,
            )),
        );
    }
}
