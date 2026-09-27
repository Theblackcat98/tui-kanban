//! Switching boards: the switcher, the "no board here" dialog, and
//! replacing the open board with another. The runtime does the reading
//! and writing; this decides what happens and resets the view.

use std::path::PathBuf;
use std::time::Duration;

use super::Updater;
use crate::animation::AnimationKind;
use crate::app::action::Effect;
use crate::app::model::{BoardEntry, FocusRegion, SaveState, Screen, Session, ToastKind};
use crate::domain::Board;

impl Updater<'_> {
    pub(super) fn show_boards(&mut self, entries: Vec<BoardEntry>) {
        let selected = entries.iter().position(|entry| entry.current).unwrap_or(0);
        self.push(Screen::Boards { entries, selected });
        self.animate(AnimationKind::Modal, 140);
    }

    /// Enter in the switcher: opens the highlighted board.
    pub(super) fn open_selected_board(&mut self) {
        let Some(Screen::Boards { entries, selected }) = self.model.ui.screens.last() else {
            return;
        };
        let entry = entries.get(*selected).cloned();
        self.pop();
        if let Some(entry) = entry.filter(|entry| !entry.current) {
            self.effects.push(Effect::SwitchBoard(entry.path));
        }
    }

    /// `c` when no board was found: the board on screen is saved where
    /// tui-kanban started.
    pub(super) fn create_here(&mut self) {
        let Some(Screen::NoBoard { directory, .. }) = self.model.ui.screens.last() else {
            return;
        };
        let message = format!("Created a board in {directory}");
        self.pop();
        self.save();
        self.effects.push(Effect::RememberBoard);
        self.toast(message, ToastKind::Success, Duration::from_secs(3));
    }

    /// `p` when no board was found: opens the personal board instead.
    pub(super) fn open_personal(&mut self) {
        let Some(Screen::NoBoard {
            personal: Some(path),
            ..
        }) = self.model.ui.screens.last()
        else {
            return;
        };
        let path = path.clone();
        self.pop();
        self.effects.push(Effect::LoadBoard(path));
    }

    /// Everything is saved that could be: switch, unless something isn't.
    pub(super) fn switch_ready(&mut self, path: PathBuf) {
        if self.model.session.save_state == SaveState::Saved {
            self.effects.push(Effect::LoadBoard(path));
        } else {
            self.toast(
                "Can't switch boards while changes here aren't saved",
                ToastKind::Error,
                None,
            );
        }
    }

    /// Shows another board, from scratch: nothing selected, no search,
    /// no screens, and nothing to undo.
    pub(super) fn board_opened(&mut self, board: Board) {
        let name = board.name.clone();
        let model = &mut *self.model;
        model.board = board;
        model.session = Session {
            recent_commands: std::mem::take(&mut model.session.recent_commands),
            ..Session::default()
        };
        let ui = &mut model.ui;
        ui.screens.clear();
        ui.search = Default::default();
        ui.selected = None;
        ui.active_column = 0;
        ui.focus = FocusRegion::Cards;
        ui.scroll = Default::default();
        ui.mouse = Default::default();
        ui.pending = None;
        model.reconcile_selection();
        self.toast(
            format!("Opened {name}"),
            ToastKind::Info,
            Duration::from_secs(3),
        );
        self.animate(AnimationKind::CardMove, 220);
    }
}
