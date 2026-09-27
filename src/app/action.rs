//! What can happen ([`Action`]), what the runtime should do about it
//! ([`Effect`]), and how key presses become actions ([`keymap`]).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use uuid::Uuid;

use super::editor::EditorField;
use super::model::{Model, Screen};
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
    /// Text pasted into the terminal (with bracketed paste).
    Paste(String),
    /// The result of an [`Effect::EditExternally`]: the saved text, or
    /// why it couldn't be edited.
    ExternalEditFinished {
        target: ExternalTarget,
        result: Result<String, String>,
    },
    Resize(u16, u16),
    Tick,
    /// The result of an [`Effect::Save`].
    SaveFinished(Result<(), String>),
}

/// Where text edited in `$EDITOR` goes back to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalTarget {
    /// A task's description, saved straight to the board.
    Task(Uuid),
    /// The description field of the open editor.
    Draft,
}

/// Work for the runtime, returned by [`super::update`] so that `update`
/// itself stays pure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Write the board to disk.
    Save,
    /// Remember that the first-run tip was dismissed.
    DismissTip,
    /// Suspend the TUI and edit `text` in the user's `$EDITOR`.
    EditExternally {
        text: String,
        target: ExternalTarget,
    },
    Quit,
}

/// Turns a key press into an action, using the command table for the
/// current context.
pub fn keymap(model: &Model, key: KeyEvent) -> Option<Action> {
    if !accepts_key(&key) {
        return None;
    }
    let context = model.context();
    // Only quitting works while the terminal is too small.
    if let Some(pending) = model.ui.pending.filter(|_| context != Context::TooSmall) {
        // After Space, any key does what it would have done anyway.
        if pending.prefix == ' ' {
            return Some(match key.code {
                KeyCode::Esc | KeyCode::Char(' ') => Action::CancelPrefix,
                _ => command_for(context, &key).unwrap_or(Action::CancelPrefix),
            });
        }
        return Some(complete_prefix(context, pending.prefix, &key));
    }
    // Enter saves from the title, but types a line break in the
    // description.
    if context == Context::Editor
        && key.code == KeyCode::Enter
        && key.modifiers.is_empty()
        && matches!(
            model.ui.screens.last(),
            Some(Screen::Editor(editor)) if editor.field == EditorField::Description
        )
    {
        return Some(Action::Edit(key));
    }
    if let Some(action) = command_for(context, &key) {
        return Some(action);
    }
    match context {
        Context::Search | Context::Editor => Some(Action::Edit(key)),
        Context::Help => Some(Action::DismissHelp),
        Context::QuickAdd | Context::Palette => Some(Action::Edit(key)),
        _ => None,
    }
}

/// The action for a key from the command table, with digits carrying the
/// lane they name.
fn command_for(context: Context, key: &KeyEvent) -> Option<Action> {
    let id = command::lookup(context, key)?;
    if id == CommandId::JumpToLane
        && let KeyCode::Char(digit @ '1'..='9') = key.code
    {
        return Some(Action::JumpToLane(digit as usize - '1' as usize));
    }
    Some(Action::Command(id))
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
