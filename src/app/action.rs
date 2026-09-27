//! What can happen ([`Action`]), what the runtime should do about it
//! ([`Effect`]), and how key presses become actions ([`keymap`]).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    /// A digit: the lane with that index, from 0.
    JumpToLane(usize),
    /// `g` and a letter: the next lane whose name starts with it.
    GoToLane(char),
    /// A key that doesn't complete the pending prefix.
    CancelPrefix,
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
    /// Remember that the first-run tip was dismissed.
    DismissTip,
    Quit,
}

/// Turns a key press into an action, using the command table for the
/// current context.
pub fn keymap(model: &Model, key: KeyEvent) -> Option<Action> {
    if !accepts_key(&key) {
        return None;
    }
    let context = model.context();
    if let Some(pending) = model.ui.pending {
        return Some(complete_prefix(context, pending.prefix, &key));
    }
    if let Some(id) = command::lookup(context, &key) {
        if id == CommandId::JumpToLane
            && let KeyCode::Char(digit @ '1'..='9') = key.code
        {
            return Some(Action::JumpToLane(digit as usize - '1' as usize));
        }
        return Some(Action::Command(id));
    }
    match context {
        Context::Search | Context::Editor => Some(Action::Edit(key)),
        Context::Help => Some(Action::DismissHelp),
        Context::QuickAdd => Some(Action::Edit(key)),
        _ => None,
    }
}

/// The action for the key after a prefix: a chord from the command table,
/// `g` and a letter for a lane, or cancelling the prefix.
fn complete_prefix(context: Context, prefix: char, key: &KeyEvent) -> Action {
    if let Some(id) = command::lookup_after(context, Some(prefix), key) {
        return Action::Command(id);
    }
    match key.code {
        KeyCode::Char(letter)
            if prefix == 'g'
                && letter.is_alphanumeric()
                && !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            Action::GoToLane(letter)
        }
        _ => Action::CancelPrefix,
    }
}
