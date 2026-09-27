//! `update(&mut Model, Action, Clock) -> Vec<Effect>`: the only place the
//! model changes. It does no I/O, so it can be tested directly.

use std::time::Duration;
use uuid::Uuid;

use super::action::{Action, Effect, ExternalTarget};
use super::editor::{EditorField, EditorState, Placement};
use super::history::Entry;
use super::input::TextInput;
use super::model::{FocusRegion, Model, Pending, SaveState, Screen, Toast, ToastKind, ViewMode};
use crate::animation::AnimationKind;
use crate::clock::Clock;
use crate::command::CommandId;
use crate::domain::{Board, BoardError};
use crate::ui;

pub fn update(model: &mut Model, action: Action, clock: Clock) -> Vec<Effect> {
    // An error toast stays until the next key press.
    let key_press = matches!(
        action,
        Action::Command(_)
            | Action::Edit(_)
            | Action::DismissHelp
            | Action::JumpToLane(_)
            | Action::GoToLane(_)
            | Action::CancelPrefix
            | Action::Paste(_)
    );
    // A prefix waits for exactly one more key.
    if key_press {
        model.ui.pending = None;
    }
    if key_press
        && model
            .session
            .toast
            .as_ref()
            .is_some_and(|toast| toast.expires_at.is_none())
    {
        model.session.toast = None;
    }
    let mut updater = Updater {
        model,
        clock,
        effects: Vec::new(),
    };
    updater.handle(action);
    ui::sync_scroll(updater.model);
    updater.effects
}

struct Updater<'a> {
    model: &'a mut Model,
    clock: Clock,
    effects: Vec<Effect>,
}

impl Updater<'_> {
    fn handle(&mut self, action: Action) {
        match action {
            Action::Command(id) => self.execute(id),
            Action::Edit(key) => self.edit(&key),
            Action::DismissHelp => self.pop(),
            Action::JumpToLane(index) => self.jump_to_lane(index),
            Action::GoToLane(letter) => self.go_to_lane(letter),
            Action::CancelPrefix => {}
            Action::Paste(text) => self.paste(&text),
            Action::ExternalEditFinished { target, result } => {
                self.finish_external_edit(target, result)
            }
            Action::Resize(width, height) => {
                self.model.ui.viewport = (width, height);
                if !self.model.breakpoint().shows_rail() {
                    self.model.ui.focus = FocusRegion::Cards;
                }
            }
            Action::Tick => self.tick(),
            Action::SaveFinished(Ok(())) => self.model.session.save_state = SaveState::Saved,
            Action::SaveFinished(Err(message)) => {
                self.toast(format!("Could not save: {message}"), ToastKind::Error, None);
                self.model.session.save_state = SaveState::Failed(message);
            }
        }
    }

    fn tick(&mut self) {
        let now = self.clock.instant;
        self.model.ui.animations.tick(now);
        if self
            .model
            .session
            .toast
            .as_ref()
            .and_then(|toast| toast.expires_at)
            .is_some_and(|expires_at| expires_at <= now)
        {
            self.model.session.toast = None;
        }
    }

    fn execute(&mut self, id: CommandId) {
        match id {
            CommandId::PreviousColumn | CommandId::RailPrevious => self.move_column(-1),
            CommandId::NextColumn | CommandId::RailNext => self.move_column(1),
            CommandId::PreviousCard => self.move_selection(-1),
            CommandId::NextCard => self.move_selection(1),
            CommandId::RowUp => self.move_row(-1),
            CommandId::RowDown => self.move_row(1),
            CommandId::CardLeft => self.move_in_row(-1),
            CommandId::CardRight => self.move_in_row(1),
            CommandId::PreviousGroup => self.move_column(-1),
            CommandId::NextGroup => self.move_column(1),
            CommandId::PageUp => self.page(-5),
            CommandId::PageDown => self.page(5),
            CommandId::FirstCard => self.select_edge(true),
            CommandId::LastCard => self.select_edge(false),
            CommandId::RailFirst => self.select_column(0),
            CommandId::RailLast => {
                self.select_column(self.model.board.columns.len().saturating_sub(1))
            }
            // Digits and `g` + letter arrive as their own actions.
            CommandId::JumpToLane | CommandId::GoToLane => {}
            CommandId::GoPrefix => {
                self.model.ui.pending = Some(Pending {
                    prefix: 'g',
                    since: self.clock.instant,
                });
            }
            CommandId::FocusCards => self.model.ui.focus = FocusRegion::Cards,
            CommandId::ToggleFocus => {
                let rail_shown = self.model.breakpoint().shows_rail();
                let ui = &mut self.model.ui;
                ui.focus = match ui.focus {
                    FocusRegion::Rail => FocusRegion::Cards,
                    // The rail can only take focus while it is on screen.
                    FocusRegion::Cards if rail_shown => FocusRegion::Rail,
                    FocusRegion::Cards => FocusRegion::Cards,
                };
            }
            CommandId::ToggleView => {
                self.model.ui.view = match self.model.ui.view {
                    ViewMode::Board => ViewMode::AllTasks,
                    ViewMode::AllTasks => ViewMode::Board,
                };
                self.model.ui.focus = FocusRegion::Cards;
                self.model.reconcile_selection();
                self.animate(AnimationKind::Selection, 140);
            }
            CommandId::OpenDetail => {
                if let Some(task) = self.model.selected_task_id() {
                    self.push(Screen::Detail {
                        task,
                        scroll: 0,
                        item: 0,
                    });
                    self.animate(AnimationKind::Drawer, 220);
                }
            }
            CommandId::NewTask | CommandId::NewTaskAbove => {
                let placement = self.placement(id == CommandId::NewTaskAbove);
                let editor = EditorState::new(placement, &self.model.ui.theme);
                self.push(Screen::Editor(Box::new(editor)));
                self.animate(AnimationKind::Modal, 180);
            }
            CommandId::QuickAdd => {
                if !self.model.board.columns.is_empty() {
                    self.push(Screen::QuickAdd {
                        column: self.model.ui.active_column,
                        input: TextInput::new(""),
                    });
                    self.animate(AnimationKind::Modal, 120);
                }
            }
            CommandId::AddQuickTask => self.add_quick_task(),
            CommandId::CloseQuickAdd | CommandId::CloseMenu => self.pop(),
            CommandId::DuplicateTask => self.duplicate_task(),
            CommandId::MoveTo => {
                if let Some(task) = self.target_task()
                    && let Some((column, _)) = self.model.board.task_location(task)
                {
                    self.push(Screen::MoveTo {
                        task,
                        selected: column,
                    });
                    self.animate(AnimationKind::Modal, 140);
                }
            }
            CommandId::MenuUp => self.move_menu(-1),
            CommandId::MenuDown => self.move_menu(1),
            CommandId::MenuPick => {
                if let Some(&Screen::MoveTo { selected, .. }) = self.model.ui.screens.last() {
                    self.pick_move_target(selected);
                }
            }
            CommandId::EditExternally => {
                if let Some(task) = self.target_task().and_then(|id| self.model.board.task(id)) {
                    self.effects.push(Effect::EditExternally {
                        text: task.description.clone(),
                        target: ExternalTarget::Task(task.id),
                    });
                }
            }
            CommandId::DraftInEditor => {
                if let Some(Screen::Editor(editor)) = self.model.ui.screens.last() {
                    self.effects.push(Effect::EditExternally {
                        text: editor.description_text(),
                        target: ExternalTarget::Draft,
                    });
                }
            }
            CommandId::ToggleItem => self.toggle_item(),
            CommandId::NextItem => self.move_item(1),
            CommandId::PreviousItem => self.move_item(-1),
            CommandId::MoveTaskUp => self.reorder_task(-1),
            CommandId::MoveTaskDown => self.reorder_task(1),
            CommandId::EditTask => {
                if let Some(task) = self.target_task().and_then(|id| self.model.board.task(id)) {
                    let editor = EditorState::from_task(task, &self.model.ui.theme);
                    self.push(Screen::Editor(Box::new(editor)));
                    self.animate(AnimationKind::Modal, 180);
                }
            }
            CommandId::DeleteTask => {
                if let Some(task) = self.target_task() {
                    self.push(Screen::ConfirmDelete { task });
                    self.animate(AnimationKind::Modal, 160);
                }
            }
            CommandId::MoveTaskLeft => self.move_task_sideways(-1),
            CommandId::MoveTaskRight => self.move_task_sideways(1),
            CommandId::Undo => self.undo(),
            CommandId::Redo => self.redo(),
            CommandId::Search => {
                let search = &mut self.model.ui.search;
                search.input = Some(TextInput::new(search.query.clone()));
                self.model.ui.focus = FocusRegion::Cards;
                self.animate(AnimationKind::Selection, 120);
            }
            // Esc goes back one level: it clears a search, leaves the rail,
            // or dismisses the tip, and never quits.
            CommandId::ClearSearch => {
                if self.model.ui.search.is_active() {
                    self.model.ui.search = Default::default();
                    self.model.reconcile_selection();
                } else if self.model.ui.focus == FocusRegion::Rail {
                    self.model.ui.focus = FocusRegion::Cards;
                } else {
                    self.dismiss_tip();
                }
            }
            CommandId::ApplySearch => self.model.ui.search.input = None,
            CommandId::ScrollUp => self.scroll_detail(-10),
            CommandId::ScrollDown => self.scroll_detail(10),
            CommandId::LineUp => self.scroll_detail(-1),
            CommandId::LineDown => self.scroll_detail(1),
            CommandId::NextField => {
                if let Some(Screen::Editor(editor)) = self.model.ui.screens.last_mut() {
                    editor.field = match editor.field {
                        EditorField::Title => EditorField::Description,
                        EditorField::Description => EditorField::Title,
                    };
                }
            }
            CommandId::SaveTask => self.save_editor(),
            CommandId::CancelEdit => {
                let dirty = matches!(
                    self.model.ui.screens.last(),
                    Some(Screen::Editor(editor)) if editor.is_dirty()
                );
                if dirty {
                    self.push(Screen::ConfirmDiscard);
                    self.animate(AnimationKind::Modal, 140);
                } else {
                    self.pop();
                }
            }
            CommandId::DiscardChanges => {
                // The dialog, then the editor under it.
                self.pop();
                self.pop();
            }
            CommandId::KeepEditing => self.pop(),
            CommandId::ConfirmDelete => {
                if let Some(&Screen::ConfirmDelete { task }) = self.model.ui.screens.last() {
                    self.delete_task(task);
                }
            }
            CommandId::CloseDetail | CommandId::CancelDelete | CommandId::CloseHelp => self.pop(),
            CommandId::HelpScrollUp => self.scroll_help(-1),
            CommandId::HelpScrollDown => self.scroll_help(1),
            CommandId::Help => {
                let context = self.model.context();
                self.push(Screen::Help { scroll: 0, context });
                self.animate(AnimationKind::Modal, 180);
                self.dismiss_tip();
            }
            CommandId::Quit | CommandId::ForceQuit => self.effects.push(Effect::Quit),
        }
    }

    /// Keys typed into the search box or the editor.
    fn edit(&mut self, key: &ratatui::crossterm::event::KeyEvent) {
        if let Some(input) = &mut self.model.ui.search.input {
            if input.handle_key(key) {
                self.model.ui.search.query = input.value.clone();
                self.model.reconcile_selection();
            }
        } else {
            match self.model.ui.screens.last_mut() {
                Some(Screen::Editor(editor)) => {
                    if editor.handle_key(key) {
                        editor.error = None;
                    }
                }
                Some(Screen::QuickAdd { input, .. }) => {
                    input.handle_key(key);
                }
                _ => {}
            }
        }
    }

    /// Ticks or unticks the focused checklist item in the drawer.
    fn toggle_item(&mut self) {
        let Some(&Screen::Detail { task: id, item, .. }) = self.model.ui.screens.last() else {
            return;
        };
        let Some(task) = self.model.board.task(id) else {
            return;
        };
        let Some((description, checked, text)) = crate::markdown::toggle(&task.description, item)
        else {
            return;
        };
        let title = task.title.clone();
        let label = format!("{} '{text}'", if checked { "tick" } else { "untick" });
        if let Err(message) = self.change(label, |board, now| {
            board.update_task(id, title, description, now)
        }) {
            self.toast(message, ToastKind::Error, None);
        }
    }

    /// Moves the drawer's checklist focus, wrapping around, and scrolls
    /// to it.
    fn move_item(&mut self, direction: isize) {
        let Some(Screen::Detail { task, item, .. }) = self.model.ui.screens.last() else {
            return;
        };
        let Some((_, total)) = self
            .model
            .board
            .task(*task)
            .and_then(|task| crate::markdown::progress(&task.description))
        else {
            return;
        };
        let next = (*item as isize + direction).rem_euclid(total as isize) as usize;
        if let Some(Screen::Detail { item, .. }) = self.model.ui.screens.last_mut() {
            *item = next;
        }
        ui::reveal_detail_item(self.model);
    }

    fn finish_external_edit(&mut self, target: ExternalTarget, result: Result<String, String>) {
        let text = match result {
            Ok(text) => text.trim_end().to_owned(),
            Err(message) => {
                self.toast(format!("Editor: {message}"), ToastKind::Error, None);
                return;
            }
        };
        match target {
            ExternalTarget::Draft => {
                if let Some(Screen::Editor(editor)) = self.model.ui.screens.last_mut() {
                    editor.set_description(&text);
                    editor.field = EditorField::Description;
                }
            }
            ExternalTarget::Task(id) => {
                let Some(task) = self.model.board.task(id) else {
                    return;
                };
                if task.description == text {
                    return;
                }
                let title = task.title.clone();
                let label = format!("edit '{title}'");
                match self.change(label, |board, now| board.update_task(id, title, text, now)) {
                    Ok(()) => self.toast(
                        "Description saved · u to undo",
                        ToastKind::Success,
                        Duration::from_secs(3),
                    ),
                    Err(message) => self.toast(message, ToastKind::Error, None),
                }
            }
        }
    }

    /// Pasted text goes into whichever input is being typed in.
    fn paste(&mut self, text: &str) {
        if let Some(input) = &mut self.model.ui.search.input {
            input.insert_str(&text.replace(['\n', '\r'], " "));
            self.model.ui.search.query = input.value.clone();
            self.model.reconcile_selection();
            return;
        }
        match self.model.ui.screens.last_mut() {
            Some(Screen::Editor(editor)) => {
                if editor.paste(text) {
                    editor.error = None;
                }
            }
            Some(Screen::QuickAdd { input, .. }) => {
                input.insert_str(&text.replace(['\n', '\r'], " "));
            }
            _ => {}
        }
    }

    fn push(&mut self, screen: Screen) {
        self.model.ui.screens.push(screen);
    }

    fn pop(&mut self) {
        self.model.ui.screens.pop();
    }

    /// The task that task commands act on: the one open in the detail
    /// drawer, or else the selected one.
    fn target_task(&self) -> Option<Uuid> {
        match self.model.ui.screens.last() {
            Some(Screen::Detail { task, .. }) => Some(*task),
            _ => self.model.selected_task_id(),
        }
    }

    fn scroll_detail(&mut self, delta: i32) {
        if let Some(Screen::Detail { scroll, .. }) = self.model.ui.screens.last_mut() {
            *scroll = scroll.saturating_add_signed(delta as i16);
        }
    }

    fn scroll_help(&mut self, delta: i16) {
        if let Some(Screen::Help { scroll, context }) = self.model.ui.screens.last_mut() {
            let max_scroll = ui::help_line_count(*context).saturating_sub(1) as u16;
            *scroll = scroll.saturating_add_signed(delta).min(max_scroll);
        }
    }

    fn dismiss_tip(&mut self) {
        if self.model.ui.tip {
            self.model.ui.tip = false;
            self.effects.push(Effect::DismissTip);
        }
    }

    /// Where `n` (or `N`, `above`) puts a new task: next to the selected
    /// card when the cards have focus, else at the end (or start) of the
    /// active column.
    fn placement(&self, above: bool) -> Placement {
        let column = self.model.ui.active_column;
        let anchor = self
            .model
            .selected_task_id()
            .filter(|_| self.model.ui.focus == FocusRegion::Cards)
            .filter(|id| {
                self.model
                    .board
                    .task_location(*id)
                    .is_some_and(|(task_column, _)| task_column == column)
            });
        Placement {
            column,
            anchor,
            above,
        }
    }

    fn animate(&mut self, kind: AnimationKind, millis: u64) {
        self.model
            .ui
            .animations
            .start(kind, Duration::from_millis(millis), self.clock.instant);
    }

    /// Shows a toast for `duration`, or until the next key press if that
    /// is `None`.
    fn toast(
        &mut self,
        message: impl Into<String>,
        kind: ToastKind,
        duration: impl Into<Option<Duration>>,
    ) {
        self.model.session.toast = Some(Toast {
            message: message.into(),
            kind,
            expires_at: duration
                .into()
                .map(|duration| self.clock.instant + duration),
        });
        self.animate(AnimationKind::Toast, 220);
    }

    /// Applies a change to the board, records the previous board so the
    /// change can be undone, and asks the runtime to save. If the change
    /// is rejected, the board is left as it was.
    fn change<T>(
        &mut self,
        label: String,
        change: impl FnOnce(&mut Board, i64) -> Result<T, BoardError>,
    ) -> Result<T, String> {
        let mut board = self.model.board.clone();
        let value =
            change(&mut board, self.clock.wall_millis).map_err(|error| error.to_string())?;
        let previous = std::mem::replace(&mut self.model.board, board);
        self.model.session.history.record(Entry {
            board: previous,
            label,
            selected: self.model.ui.selected,
        });
        self.effects.push(Effect::Save);
        Ok(value)
    }

    fn current_entry(&self) -> Entry {
        Entry {
            board: self.model.board.clone(),
            label: String::new(),
            selected: self.model.ui.selected,
        }
    }

    fn undo(&mut self) {
        let current = self.current_entry();
        match self.model.session.history.undo(current) {
            Some(entry) => self.restore(entry, "Undid"),
            None => self.toast("Nothing to undo", ToastKind::Info, Duration::from_secs(2)),
        }
    }

    fn redo(&mut self) {
        let current = self.current_entry();
        match self.model.session.history.redo(current) {
            Some(entry) => self.restore(entry, "Redid"),
            None => self.toast("Nothing to redo", ToastKind::Info, Duration::from_secs(2)),
        }
    }

    fn restore(&mut self, entry: Entry, verb: &str) {
        self.model.board = entry.board;
        // A drawer showing a task the undo removed has nothing to show.
        if let Some(Screen::Detail { task, .. }) = self.model.ui.screens.last()
            && self.model.board.task(*task).is_none()
        {
            self.pop();
        }
        self.model.ui.selected = entry.selected;
        self.model.reconcile_selection();
        self.effects.push(Effect::Save);
        self.toast(
            format!("{verb}: {}", entry.label),
            ToastKind::Info,
            Duration::from_secs(3),
        );
        self.animate(AnimationKind::CardMove, 220);
    }

    fn task_title(&self, id: Uuid) -> String {
        self.model
            .board
            .task(id)
            .map(|task| task.title.clone())
            .unwrap_or_default()
    }

    fn column_name(&self, index: usize) -> String {
        self.model
            .board
            .columns
            .get(index)
            .map(|column| column.name.clone())
            .unwrap_or_default()
    }

    fn save_editor(&mut self) {
        let Some(Screen::Editor(editor)) = self.model.ui.screens.last() else {
            return;
        };
        let title = editor.title_text().trim().to_owned();
        let description = editor.description_text().trim_end().to_owned();
        let task_id = editor.task_id;
        if title.is_empty() {
            self.set_editor_error("Title cannot be empty");
            return;
        }
        let placement = editor.placement.unwrap_or(Placement {
            column: self.model.ui.active_column,
            anchor: None,
            above: false,
        });
        let label = match task_id {
            Some(_) => format!("edit '{title}'"),
            None => format!("add '{title}'"),
        };
        let result = match task_id {
            Some(id) => self.change(label, |board, now| {
                board.update_task(id, title, description, now).map(|_| id)
            }),
            None => self.change(label, |board, now| {
                let anchor = placement.anchor.and_then(|id| board.task_location(id));
                let index = match anchor {
                    Some((column, index)) if column == placement.column => {
                        index + usize::from(!placement.above)
                    }
                    _ if placement.above => 0,
                    _ => usize::MAX,
                };
                board.insert_task(placement.column, index, title, description, now)
            }),
        };
        match result {
            Ok(id) => {
                self.pop();
                self.model.select_task(id);
                self.model.reconcile_selection();
                self.toast("Task saved", ToastKind::Success, Duration::from_secs(3));
                self.animate(AnimationKind::CardMove, 220);
            }
            Err(message) => self.set_editor_error(&message),
        }
    }

    fn set_editor_error(&mut self, message: &str) {
        if let Some(Screen::Editor(editor)) = self.model.ui.screens.last_mut() {
            editor.error = Some(message.to_owned());
        }
    }

    fn delete_task(&mut self, id: Uuid) {
        // Select the task that takes the deleted one's place, so the
        // selection doesn't jump back to the top of the column.
        let tasks = self.model.navigable_tasks();
        let position = tasks.iter().position(|task| *task == id);
        let neighbour = position.and_then(|position| {
            tasks
                .get(position + 1)
                .or_else(|| {
                    position
                        .checked_sub(1)
                        .and_then(|previous| tasks.get(previous))
                })
                .copied()
        });
        let label = format!("delete '{}'", self.task_title(id));
        if let Err(message) = self.change(label, |board, _| board.remove_task(id).map(|_| ())) {
            self.toast(message, ToastKind::Error, None);
            return;
        }
        // Close the dialog, and the detail drawer too if it showed this task.
        self.pop();
        if matches!(self.model.ui.screens.last(), Some(Screen::Detail { task, .. }) if *task == id)
        {
            self.pop();
        }
        self.model.ui.selected = neighbour;
        self.model.reconcile_selection();
        self.toast(
            "Task deleted · u to undo",
            ToastKind::Info,
            Duration::from_secs(5),
        );
        self.animate(AnimationKind::CardMove, 220);
    }

    fn add_quick_task(&mut self) {
        let Some(Screen::QuickAdd { column, input }) = self.model.ui.screens.last() else {
            return;
        };
        let column = *column;
        let title = input.value.trim().to_owned();
        // Enter on an empty prompt means done.
        if title.is_empty() {
            self.pop();
            return;
        }
        let label = format!("add '{title}'");
        match self.change(label, |board, now| board.add_task(column, title, "", now)) {
            Ok(id) => {
                if let Some(Screen::QuickAdd { input, .. }) = self.model.ui.screens.last_mut() {
                    *input = TextInput::new("");
                }
                self.model.select_task(id);
                self.model.reconcile_selection();
                let message = format!("Added to {}", self.column_name(column));
                self.toast(message, ToastKind::Success, Duration::from_secs(2));
                self.animate(AnimationKind::CardMove, 220);
            }
            Err(message) => self.toast(message, ToastKind::Error, None),
        }
    }

    fn duplicate_task(&mut self) {
        let Some(id) = self.target_task() else {
            return;
        };
        let label = format!("duplicate '{}'", self.task_title(id));
        match self.change(label, |board, now| board.duplicate_task(id, now)) {
            Ok(copy) => {
                // A copy made from the drawer opens in the drawer.
                if let Some(Screen::Detail { task, scroll, item }) =
                    self.model.ui.screens.last_mut()
                {
                    *task = copy;
                    *scroll = 0;
                    *item = 0;
                }
                self.model.select_task(copy);
                self.model.reconcile_selection();
                self.toast(
                    "Task duplicated · u to undo",
                    ToastKind::Info,
                    Duration::from_secs(3),
                );
                self.animate(AnimationKind::CardMove, 220);
            }
            Err(message) => self.toast(message, ToastKind::Error, None),
        }
    }

    fn move_menu(&mut self, direction: isize) {
        let columns = self.model.board.columns.len();
        if let Some(Screen::MoveTo { selected, .. }) = self.model.ui.screens.last_mut() {
            *selected = selected
                .saturating_add_signed(direction)
                .min(columns.saturating_sub(1));
        }
    }

    /// Closes the "move to…" menu and moves its task to `column`.
    fn pick_move_target(&mut self, column: usize) {
        let Some(&Screen::MoveTo { task, .. }) = self.model.ui.screens.last() else {
            return;
        };
        if column >= self.model.board.columns.len() {
            return;
        }
        self.pop();
        if self.model.board.task_location(task).map(|(from, _)| from) != Some(column) {
            self.move_task_to(task, column);
        }
    }

    fn jump_to_lane(&mut self, index: usize) {
        if matches!(self.model.ui.screens.last(), Some(Screen::MoveTo { .. })) {
            self.pick_move_target(index);
        } else if index < self.model.board.columns.len() {
            self.select_column(index);
        }
    }

    /// Selects the next lane after the active one whose name starts with
    /// `letter`, wrapping around.
    fn go_to_lane(&mut self, letter: char) {
        let columns = &self.model.board.columns;
        let active = self.model.ui.active_column;
        let starts_with = |index: usize| {
            columns[index]
                .name
                .chars()
                .next()
                .is_some_and(|first| first.to_lowercase().eq(letter.to_lowercase()))
        };
        let found = (1..=columns.len())
            .map(|step| (active + step) % columns.len())
            .find(|index| starts_with(*index));
        match found {
            Some(index) => self.select_column(index),
            None => self.toast(
                format!("No lane starts with \"{letter}\""),
                ToastKind::Info,
                Duration::from_secs(2),
            ),
        }
    }

    /// Moves the target task up or down past the next visible task in its
    /// column.
    fn reorder_task(&mut self, direction: isize) {
        let Some(id) = self.target_task() else {
            return;
        };
        let Some((column, index)) = self.model.board.task_location(id) else {
            return;
        };
        let visible = self.model.visible_task_indices(column);
        let Some(position) = visible.iter().position(|visible| *visible == index) else {
            return;
        };
        let Some(&neighbour) = position
            .checked_add_signed(direction)
            .and_then(|next| visible.get(next))
        else {
            return;
        };
        // Moving down lands after the neighbour; `move_task` counts the
        // index as if the task were still in place.
        let target = if direction > 0 {
            neighbour + 1
        } else {
            neighbour
        };
        let label = format!(
            "move '{}' {}",
            self.task_title(id),
            if direction > 0 { "down" } else { "up" }
        );
        match self.change(label, |board, now| {
            board.move_task(id, column, Some(target), now)
        }) {
            Ok(_) => {
                self.model.select_task(id);
                self.animate(AnimationKind::CardMove, 200);
            }
            Err(message) => self.toast(message, ToastKind::Error, None),
        }
    }

    fn move_task_sideways(&mut self, direction: isize) {
        let Some(id) = self.target_task() else {
            return;
        };
        let Some((column, _)) = self.model.board.task_location(id) else {
            return;
        };
        let Some(target) = column.checked_add_signed(direction) else {
            return;
        };
        if target < self.model.board.columns.len() {
            self.move_task_to(id, target);
        }
    }

    /// Moves a task to the end of another column.
    fn move_task_to(&mut self, id: Uuid, target: usize) {
        let label = format!(
            "move '{}' → {}",
            self.task_title(id),
            self.column_name(target)
        );
        match self.change(label, |board, now| board.move_task(id, target, None, now)) {
            Ok(outcome) => {
                self.model.select_task(outcome.task_id);
                self.model.reconcile_selection();
                self.animate(AnimationKind::CardMove, 240);
            }
            Err(message) => self.toast(message, ToastKind::Error, None),
        }
    }

    fn move_column(&mut self, direction: isize) {
        let columns = self.model.board.columns.len();
        let mut next = self.model.ui.active_column;
        loop {
            next = match next.checked_add_signed(direction) {
                Some(next) if next < columns => next,
                _ => return,
            };
            // All tasks shows only columns with matches, so skip the others.
            if self.model.ui.view == ViewMode::Board || self.model.first_visible_in(next).is_some()
            {
                break;
            }
        }
        self.select_column(next);
    }

    fn select_column(&mut self, column: usize) {
        if self.model.board.columns.is_empty() {
            return;
        }
        let column = column.min(self.model.board.columns.len() - 1);
        self.model.ui.active_column = column;
        // In the Board view, select the lane's first card on screen, so
        // switching lanes doesn't scroll them.
        let offset = match self.model.ui.view {
            ViewMode::Board => self.model.ui.scroll.lane(column),
            ViewMode::AllTasks => 0,
        };
        let visible = self.model.visible_task_indices(column);
        self.model.ui.selected = visible
            .get(offset)
            .or(visible.first())
            .map(|index| self.model.board.columns[column].tasks[*index].id);
        self.model.reconcile_selection();
        self.animate(AnimationKind::Selection, 140);
    }

    fn move_selection(&mut self, direction: isize) {
        let tasks = self.model.navigable_tasks();
        if tasks.is_empty() {
            self.model.ui.selected = None;
            return;
        }
        let current = self
            .model
            .selected_task_id()
            .and_then(|id| tasks.iter().position(|task| *task == id));
        let next = match current {
            Some(current) => current
                .saturating_add_signed(direction)
                .min(tasks.len() - 1),
            // Nothing selected yet: Down starts at the top, Up at the bottom.
            None if direction > 0 => 0,
            None => tasks.len() - 1,
        };
        self.model.select_task(tasks[next]);
        self.animate(AnimationKind::Selection, 100);
    }

    fn page(&mut self, direction: isize) {
        match self.model.ui.view {
            ViewMode::Board => self.move_selection(direction),
            ViewMode::AllTasks => self.move_row(direction),
        }
    }

    /// The selected card's (row, position in the row) in the All tasks
    /// grid.
    fn grid_position(&self, rows: &[Vec<Uuid>]) -> Option<(usize, usize)> {
        let id = self.model.selected_task_id()?;
        rows.iter().enumerate().find_map(|(row, tasks)| {
            tasks
                .iter()
                .position(|task| *task == id)
                .map(|slot| (row, slot))
        })
    }

    /// Moves up or down the All tasks grid by `direction` rows, keeping
    /// the same position in the row where the new row is long enough.
    fn move_row(&mut self, direction: isize) {
        let rows = ui::all_tasks_rows(self.model);
        if rows.is_empty() {
            return;
        }
        let (row, slot) = match self.grid_position(&rows) {
            Some((row, slot)) => (
                row.saturating_add_signed(direction).min(rows.len() - 1),
                slot,
            ),
            None if direction > 0 => (0, 0),
            None => (rows.len() - 1, 0),
        };
        let tasks = &rows[row];
        self.model.select_task(tasks[slot.min(tasks.len() - 1)]);
        self.animate(AnimationKind::Selection, 100);
    }

    /// Moves left or right within the selected card's row.
    fn move_in_row(&mut self, direction: isize) {
        let rows = ui::all_tasks_rows(self.model);
        let Some((row, slot)) = self.grid_position(&rows) else {
            return self.move_row(1);
        };
        let tasks = &rows[row];
        let slot = slot.saturating_add_signed(direction).min(tasks.len() - 1);
        self.model.select_task(tasks[slot]);
        self.animate(AnimationKind::Selection, 100);
    }

    fn select_edge(&mut self, first: bool) {
        let tasks = self.model.navigable_tasks();
        let edge = if first { tasks.first() } else { tasks.last() };
        if let Some(id) = edge {
            self.model.select_task(*id);
        }
    }
}
