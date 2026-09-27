//! Managing columns from the rail: adding, renaming, deleting and
//! reordering them, their colours and work-in-progress limits, and
//! collapsing lanes. Every change goes through [`Updater::change`], so it
//! can be undone.

use std::time::Duration;
use uuid::Uuid;

use super::Updater;
use crate::animation::AnimationKind;
use crate::app::input::TextInput;
use crate::app::model::{PromptKind, Screen, ToastKind};

impl Updater<'_> {
    /// Opens the prompt for a new column after the active one, or for the
    /// active column's name or limit.
    pub(super) fn open_prompt(&mut self, kind: PromptKind) {
        let text = match kind {
            PromptKind::AddColumn { .. } => String::new(),
            PromptKind::RenameColumn { column } => self.column_name(column),
            PromptKind::WipLimit { column } => self
                .model
                .board
                .columns
                .get(column)
                .and_then(|column| column.limit())
                .map(|limit| limit.to_string())
                .unwrap_or_default(),
        };
        self.push(Screen::Prompt {
            kind,
            input: TextInput::new(text),
            error: None,
        });
        self.animate(AnimationKind::Modal, 120);
    }

    /// Enter in the prompt: adds or renames the column, or sets its limit.
    pub(super) fn submit_prompt(&mut self) {
        let Some(Screen::Prompt { kind, input, .. }) = self.model.ui.screens.last() else {
            return;
        };
        let kind = *kind;
        let text = input.value.trim().to_owned();
        let result = match kind {
            PromptKind::AddColumn { at } => self
                .change(format!("add column '{text}'"), |board, _| {
                    board.add_column(at, &text)
                })
                .map(|index| {
                    self.model.ui.scroll.reset_lanes();
                    self.model.ui.active_column = index;
                    format!("Added column {text}")
                }),
            PromptKind::RenameColumn { column } => {
                let old = self.column_name(column);
                if old == text {
                    Ok(String::new())
                } else {
                    self.change(format!("rename '{old}' → '{text}'"), |board, _| {
                        board.rename_column(column, &text)
                    })
                    .map(|()| String::new())
                }
            }
            PromptKind::WipLimit { column } => match parse_limit(&text) {
                Ok(limit) => {
                    let name = self.column_name(column);
                    let label = match limit {
                        Some(limit) => format!("limit '{name}' to {limit}"),
                        None => format!("remove the limit on '{name}'"),
                    };
                    self.change(label, |board, _| board.set_wip_limit(column, limit))
                        .map(|()| String::new())
                }
                Err(message) => Err(message),
            },
        };
        match result {
            Ok(message) => {
                self.pop();
                self.model.reconcile_selection();
                if !message.is_empty() {
                    self.toast(message, ToastKind::Success, Duration::from_secs(2));
                }
            }
            Err(message) => {
                if let Some(Screen::Prompt { error, .. }) = self.model.ui.screens.last_mut() {
                    *error = Some(capitalised(&message));
                }
            }
        }
    }

    /// Asks before deleting the active column, offering to move its tasks
    /// to the column before it (or after, for the first).
    pub(super) fn open_delete_column(&mut self) {
        let columns = self.model.board.columns.len();
        if columns < 2 {
            return;
        }
        let column = self.model.ui.active_column.min(columns - 1);
        let tasks_to = Some(if column > 0 { column - 1 } else { 1 });
        self.push(Screen::DeleteColumn { column, tasks_to });
        self.animate(AnimationKind::Modal, 160);
    }

    /// Steps through where the deleted column's tasks go: each other
    /// column in turn, then "delete them too".
    pub(super) fn step_tasks_to(&mut self, direction: isize) {
        let columns = self.model.board.columns.len();
        let Some(Screen::DeleteColumn { column, tasks_to }) = self.model.ui.screens.last_mut()
        else {
            return;
        };
        // The choices, in order: every other column, then None.
        let mut choices: Vec<Option<usize>> = (0..columns)
            .filter(|index| index != column)
            .map(Some)
            .collect();
        choices.push(None);
        let current = choices
            .iter()
            .position(|choice| choice == tasks_to)
            .unwrap_or(0);
        let next = (current as isize + direction).rem_euclid(choices.len() as isize) as usize;
        *tasks_to = choices[next];
    }

    pub(super) fn delete_column(&mut self) {
        let Some(&Screen::DeleteColumn { column, tasks_to }) = self.model.ui.screens.last() else {
            return;
        };
        let name = self.column_name(column);
        let count = self
            .model
            .board
            .columns
            .get(column)
            .map_or(0, |column| column.tasks.len());
        let tasks_to = tasks_to.filter(|_| count > 0);
        let label = format!("delete column '{name}'");
        let result = self.change(label, |board, now| {
            board.remove_column(column, tasks_to, now).map(|_| ())
        });
        self.pop();
        match result {
            Ok(()) => {
                self.model.ui.scroll.reset_lanes();
                let target =
                    tasks_to.map(|target| if target > column { target - 1 } else { target });
                self.model.ui.active_column = target.unwrap_or(column.saturating_sub(1));
                self.model.ui.selected = None;
                self.model.reconcile_selection();
                let message = match (count, target) {
                    (0, _) => format!("Deleted {name} · u to undo"),
                    (_, Some(target)) => format!(
                        "Deleted {name}, its tasks moved to {} · u to undo",
                        self.column_name(target)
                    ),
                    (_, None) => format!("Deleted {name} and its tasks · u to undo"),
                };
                self.toast(message, ToastKind::Info, Duration::from_secs(5));
                self.animate(AnimationKind::CardMove, 220);
            }
            Err(message) => self.toast(capitalised(&message), ToastKind::Error, None),
        }
    }

    /// Moves the active column up or down the rail (left or right on the
    /// board).
    pub(super) fn move_active_column(&mut self, direction: isize) {
        let column = self.model.ui.active_column;
        let Some(target) = column
            .checked_add_signed(direction)
            .filter(|target| *target < self.model.board.columns.len())
        else {
            return;
        };
        let label = format!(
            "move column '{}' {}",
            self.column_name(column),
            if direction > 0 { "down" } else { "up" }
        );
        if self
            .change(label, |board, _| board.move_column(column, target))
            .is_ok()
        {
            self.model.ui.scroll.reset_lanes();
            self.model.ui.active_column = target;
            self.model.reconcile_selection();
            self.animate(AnimationKind::Selection, 140);
        }
    }

    pub(super) fn open_colors(&mut self) {
        let column = self.model.ui.active_column;
        let Some(data) = self.model.board.columns.get(column) else {
            return;
        };
        let names = self.model.ui.theme.accent_names();
        // Highlight the current colour, or "automatic".
        let selected = data
            .color
            .as_deref()
            .and_then(|color| {
                names
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(color.trim()))
            })
            .map_or(0, |index| index + 1);
        self.push(Screen::Colors { column, selected });
        self.animate(AnimationKind::Modal, 140);
    }

    pub(super) fn pick_color(&mut self) {
        let Some(&Screen::Colors { column, selected }) = self.model.ui.screens.last() else {
            return;
        };
        self.pop();
        let names = self.model.ui.theme.accent_names();
        let color = selected
            .checked_sub(1)
            .and_then(|index| names.get(index))
            .map(|name| (*name).to_owned());
        let Some(current) = self.model.board.columns.get(column) else {
            return;
        };
        if current.color == color {
            return;
        }
        let name = self.column_name(column);
        let label = match &color {
            Some(color) => format!("colour '{name}' {color}"),
            None => format!("automatic colour for '{name}'"),
        };
        if self
            .change(label, |board, _| board.set_column_color(column, color))
            .is_ok()
        {
            self.animate(AnimationKind::CardMove, 220);
        }
    }

    /// Collapses the active lane to a narrow strip, or expands it.
    pub(super) fn toggle_collapse(&mut self) {
        let column = self.model.ui.active_column;
        let Some(collapsed) = self
            .model
            .board
            .columns
            .get(column)
            .map(|column| column.collapsed)
        else {
            return;
        };
        self.set_collapsed(column, !collapsed);
    }

    pub(super) fn set_collapsed(&mut self, column: usize, collapsed: bool) {
        let verb = if collapsed { "collapse" } else { "expand" };
        let label = format!("{verb} '{}'", self.column_name(column));
        if self
            .change(label, |board, _| board.set_collapsed(column, collapsed))
            .is_ok()
        {
            self.model.reconcile_selection();
            self.animate(AnimationKind::Selection, 140);
        }
    }

    /// Moves a task to `column` (at `index`, or the end), first asking if
    /// that column is already at its work-in-progress limit.
    pub(super) fn request_move(&mut self, task: Uuid, column: usize, index: Option<usize>) {
        let from = self.model.board.task_location(task).map(|(from, _)| from);
        let full = self
            .model
            .board
            .columns
            .get(column)
            .is_some_and(|data| data.limit().is_some_and(|limit| data.tasks.len() >= limit));
        if full && from != Some(column) {
            self.push(Screen::ConfirmWip {
                task,
                column,
                index,
            });
            self.animate(AnimationKind::Modal, 140);
        } else {
            self.move_task_to(task, column, index);
        }
    }

    pub(super) fn confirm_wip_move(&mut self) {
        let Some(&Screen::ConfirmWip {
            task,
            column,
            index,
        }) = self.model.ui.screens.last()
        else {
            return;
        };
        self.pop();
        self.move_task_to(task, column, index);
    }
}

/// A work-in-progress limit as typed: a positive number, or nothing (or
/// 0) for no limit.
fn parse_limit(text: &str) -> Result<Option<usize>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<usize>()
        .map(|limit| (limit > 0).then_some(limit))
        .map_err(|_| "Enter a number, or nothing for no limit".to_owned())
}

fn capitalised(message: &str) -> String {
    let mut characters = message.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_numbers_or_nothing() {
        assert_eq!(parse_limit(""), Ok(None));
        assert_eq!(parse_limit("0"), Ok(None));
        assert_eq!(parse_limit("4"), Ok(Some(4)));
        assert!(parse_limit("four").is_err());
        assert!(parse_limit("-1").is_err());
    }
}
