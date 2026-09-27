//! The mouse: clicking selects and focuses, a double click opens, the
//! wheel scrolls, and dragging a card moves it. The keyboard stays the
//! main way in; the mouse only reaches what is on screen.

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::Updater;
use crate::animation::AnimationKind;
use crate::app::model::{FocusRegion, Screen, ViewMode};
use crate::app::mouse::{DOUBLE_CLICK, Drag, Hit};
use crate::command::CommandId;
use crate::ui;

impl Updater<'_> {
    pub(super) fn mouse(&mut self, event: MouseEvent) {
        if self.model.too_small() {
            return;
        }
        let (x, y) = (event.column, event.row);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => self.mouse_down(x, y),
            MouseEventKind::Drag(MouseButton::Left) => self.mouse_drag(x, y),
            MouseEventKind::Up(MouseButton::Left) => self.mouse_up(),
            MouseEventKind::ScrollDown => self.wheel(x, y, 1),
            MouseEventKind::ScrollUp => self.wheel(x, y, -1),
            _ => {}
        }
    }

    fn mouse_down(&mut self, x: u16, y: u16) {
        let hit = ui::hit(self.model, x, y);
        let now = self.clock.instant;
        let mouse = &mut self.model.ui.mouse;
        mouse.drag = None;
        let double = hit.is_some()
            && mouse.last_click.is_some_and(|(at, previous)| {
                Some(previous) == hit && now.saturating_duration_since(at) <= DOUBLE_CLICK
            });
        // A third click starts over rather than making another double.
        mouse.last_click = if double {
            None
        } else {
            hit.map(|hit| (now, hit))
        };
        let Some(hit) = hit else {
            return;
        };
        match hit {
            Hit::Hint(id) => {
                if self.model.command_enabled(id) {
                    self.execute(id);
                }
            }
            Hit::View(view) => {
                if view != self.model.ui.view {
                    self.execute(CommandId::ToggleView);
                }
            }
            Hit::MoreLeft => {
                let range = ui::lane_range(self.model);
                self.model.ui.focus = FocusRegion::Cards;
                self.select_column(range.start.saturating_sub(1));
            }
            Hit::MoreRight => {
                let range = ui::lane_range(self.model);
                self.model.ui.focus = FocusRegion::Cards;
                self.select_column(range.end);
            }
            Hit::Rail { column } => {
                self.select_column(column);
                self.model.ui.focus = if double {
                    FocusRegion::Cards
                } else {
                    FocusRegion::Rail
                };
            }
            Hit::Lane { column } | Hit::Strip { column } => {
                self.model.ui.focus = FocusRegion::Cards;
                if column != self.model.ui.active_column {
                    self.select_column(column);
                }
                if double && matches!(hit, Hit::Strip { .. }) {
                    self.set_collapsed(column, false);
                }
            }
            Hit::Card { id } => self.click_card(id, x, y, double),
            Hit::Drawer => {}
        }
    }

    /// Selects a card. A double click opens it; with the drawer open, it
    /// shows there instead. Holding the button starts dragging it.
    fn click_card(&mut self, id: uuid::Uuid, x: u16, y: u16, double: bool) {
        self.model.ui.focus = FocusRegion::Cards;
        self.model.select_task(id);
        self.animate(AnimationKind::Selection, 100);
        if let Some(Screen::Detail { task, scroll, item }) = self.model.ui.screens.last_mut() {
            *task = id;
            *scroll = 0;
            *item = 0;
            return;
        }
        if double {
            self.execute(CommandId::OpenDetail);
        } else {
            self.model.ui.mouse.drag = Some(Drag {
                task: id,
                from: (x, y),
                moved: false,
                target: None,
            });
        }
    }

    fn mouse_drag(&mut self, x: u16, y: u16) {
        let Some(drag) = self.model.ui.mouse.drag else {
            return;
        };
        if !drag.moved && drag.from == (x, y) {
            return;
        }
        let target = ui::drop_target(self.model, x, y);
        if let Some(drag) = &mut self.model.ui.mouse.drag {
            drag.moved = true;
            drag.target = target;
        }
    }

    /// Drops a dragged card where its marker shows.
    fn mouse_up(&mut self) {
        let Some(drag) = self.model.ui.mouse.drag.take() else {
            return;
        };
        if drag.moved {
            // A click right after a drag isn't a double click.
            self.model.ui.mouse.last_click = None;
        }
        let (Some(target), true) = (drag.target, drag.moved) else {
            return;
        };
        let Some((column, index)) = self.model.board.task_location(drag.task) else {
            return;
        };
        // Dropping a card next to itself leaves it where it is.
        if target.column == column {
            let last = self.model.board.columns[column].tasks.len() - 1;
            let stays = match target.index {
                Some(target) => target == index || target == index + 1,
                None => index == last,
            };
            if stays {
                return;
            }
        }
        self.request_move(drag.task, target.column, target.index);
    }

    /// The wheel scrolls whatever is under the pointer: an overlay's list,
    /// the drawer's description, or a lane. In the active lane and in All
    /// tasks it moves the selection, which scrolls to keep it in view.
    fn wheel(&mut self, x: u16, y: u16, direction: isize) {
        match self.model.ui.screens.last() {
            Some(Screen::Help { .. }) => return self.scroll_help(3 * direction as i16),
            Some(Screen::Palette(_)) => return self.move_palette(direction),
            Some(Screen::MoveTo { .. } | Screen::Colors { .. }) => {
                return self.move_menu(direction);
            }
            Some(Screen::Detail { .. }) | None => {}
            Some(_) => return,
        }
        let column = match ui::hit(self.model, x, y) {
            Some(Hit::Drawer) => return self.scroll_detail(3 * direction as i32),
            Some(Hit::Rail { .. }) => return self.move_column(direction),
            Some(Hit::Card { id }) => match self.model.board.task_location(id) {
                Some((column, _)) => column,
                None => return,
            },
            Some(Hit::Lane { column }) => column,
            _ => return,
        };
        match self.model.ui.view {
            ViewMode::AllTasks => self.move_row(direction),
            ViewMode::Board if column == self.model.ui.active_column => {
                self.move_selection(direction)
            }
            ViewMode::Board => {
                let scroll = &mut self.model.ui.scroll;
                let offset = scroll.lane(column).saturating_add_signed(direction);
                scroll.set_lane(column, offset);
            }
        }
    }
}
