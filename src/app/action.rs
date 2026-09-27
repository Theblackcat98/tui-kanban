//! What can happen ([`Action`]), what the runtime should do about it
//! ([`Effect`]), and how key presses become actions ([`keymap`]).

use ratatui::crossterm::event::KeyEvent;

use super::model::Model;
use crate::command::{self, CommandId, Context};
use crate::tui::event::accepts_key;

#[derive(Clone, Debug)]
pub enum Action {
    /// A command from the command table.
    Command(CommandId),
    /// A key no command claims, typed into the search box or the editor.
    Edit(KeyEvent),
    /// A key no command claims while help is open: it closes help.
    DismissHelp,
    Resize(u16, u16),
    Tick,
    /// The result of an [`Effect::Save`].
    SaveFinished(Result<(), String>),
}

/// Work for the runtime, returned by [`super::update`] so that `update`
/// itself stays pure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Write the board to disk.
    Save,
    Quit,
}

/// Turns a key press into an action, using the command table for the
/// current context.
pub fn keymap(model: &Model, key: KeyEvent) -> Option<Action> {
    if !accepts_key(&key) {
        return None;
    }
    let context = model.context();
    if let Some(id) = command::lookup(context, &key) {
        return Some(Action::Command(id));
    }
    match context {
        Context::Search | Context::Editor => Some(Action::Edit(key)),
        Context::Help => Some(Action::DismissHelp),
        _ => None,
    }
}
