//! Every keybinding, defined once.
//!
//! Key handling looks a key up here to get a [`CommandId`], and the help
//! overlay and the footer hints are generated from the same table, so
//! they can't drift apart.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where the user is, which decides what keys mean.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Context {
    /// The Board view with a card list focused.
    Board = 1 << 0,
    /// The All tasks view with the cards focused.
    AllTasks = 1 << 1,
    /// The column rail focused.
    Rail = 1 << 2,
    /// Typing a search query.
    Search = 1 << 3,
    Detail = 1 << 4,
    Editor = 1 << 5,
    Help = 1 << 6,
    Confirm = 1 << 7,
    /// The terminal is too small to use.
    TooSmall = 1 << 8,
}

impl Context {
    pub const ALL: [Context; 9] = [
        Context::Board,
        Context::AllTasks,
        Context::Rail,
        Context::Search,
        Context::Detail,
        Context::Editor,
        Context::Help,
        Context::Confirm,
        Context::TooSmall,
    ];
}

/// A set of contexts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Contexts(u16);

impl Contexts {
    const fn of(contexts: &[Context]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < contexts.len() {
            bits |= contexts[index] as u16;
            index += 1;
        }
        Self(bits)
    }

    pub fn contains(self, context: Context) -> bool {
        self.0 & context as u16 != 0
    }
}

const CARDS: Contexts = Contexts::of(&[Context::Board, Context::AllTasks]);
const DASHBOARD: Contexts = Contexts::of(&[Context::Board, Context::AllTasks, Context::Rail]);
const TASK_ACTIONS: Contexts = Contexts::of(&[Context::Board, Context::AllTasks, Context::Detail]);
const EVERYWHERE: Contexts = Contexts(u16::MAX);

const fn only(context: Context) -> Contexts {
    Contexts(context as u16)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    Navigation,
    Tasks,
    Details,
    Editing,
    General,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Navigation,
        Group::Tasks,
        Group::Details,
        Group::Editing,
        Group::General,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Tasks => "Tasks",
            Self::Details => "Details",
            Self::Editing => "Editor, search and dialogs",
            Self::General => "General",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandId {
    PreviousColumn,
    NextColumn,
    PreviousCard,
    NextCard,
    PageUp,
    PageDown,
    FirstCard,
    LastCard,
    RailPrevious,
    RailNext,
    RailFirst,
    RailLast,
    FocusCards,
    ToggleFocus,
    ToggleView,
    OpenDetail,
    NewTask,
    EditTask,
    DeleteTask,
    MoveTaskLeft,
    MoveTaskRight,
    Undo,
    Redo,
    Search,
    ClearSearch,
    ApplySearch,
    ScrollUp,
    ScrollDown,
    CloseDetail,
    NextField,
    SaveTask,
    CancelEdit,
    ConfirmDelete,
    CancelDelete,
    HelpScrollUp,
    HelpScrollDown,
    CloseHelp,
    Help,
    Quit,
    ForceQuit,
}

/// One key, as written in the table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Key {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

const fn key(code: KeyCode) -> Key {
    Key {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

const fn ch(character: char) -> Key {
    key(KeyCode::Char(character))
}

const fn ctrl(character: char) -> Key {
    Key {
        code: KeyCode::Char(character),
        modifiers: KeyModifiers::CONTROL,
    }
}

impl Key {
    /// Whether a key event is this key. Shift is ignored for characters,
    /// since it is already part of the character (`H` vs `h`).
    pub fn matches(self, event: &KeyEvent) -> bool {
        let significant = |modifiers: KeyModifiers| {
            modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        };
        match (self.code, event.code) {
            (KeyCode::Char(expected), KeyCode::Char(actual)) => {
                let same = if self.modifiers.contains(KeyModifiers::CONTROL) {
                    expected.eq_ignore_ascii_case(&actual)
                } else {
                    expected == actual
                };
                same && significant(self.modifiers) == significant(event.modifiers)
            }
            (expected, actual) => {
                expected == actual && significant(self.modifiers) == significant(event.modifiers)
            }
        }
    }

    /// How the key is shown in help and hints.
    pub fn label(self) -> String {
        let name = match self.code {
            KeyCode::Char(' ') => "Space".to_owned(),
            KeyCode::Char(character) => character.to_string(),
            KeyCode::Left => "←".to_owned(),
            KeyCode::Right => "→".to_owned(),
            KeyCode::Up => "↑".to_owned(),
            KeyCode::Down => "↓".to_owned(),
            KeyCode::Enter => "Enter".to_owned(),
            KeyCode::Esc => "Esc".to_owned(),
            KeyCode::Tab => "Tab".to_owned(),
            KeyCode::BackTab => "Shift+Tab".to_owned(),
            KeyCode::PageUp => "PgUp".to_owned(),
            KeyCode::PageDown => "PgDn".to_owned(),
            KeyCode::Home => "Home".to_owned(),
            KeyCode::End => "End".to_owned(),
            KeyCode::Backspace => "Backspace".to_owned(),
            KeyCode::Delete => "Del".to_owned(),
            other => format!("{other:?}"),
        };
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            format!("Ctrl+{}", name.to_uppercase())
        } else {
            name
        }
    }
}

/// How a command appears in the footer.
#[derive(Clone, Copy, Debug)]
pub struct Hint {
    /// Lower numbers are shown first and dropped last when space runs out.
    pub priority: u8,
    pub label: &'static str,
}

const fn hint(priority: u8, label: &'static str) -> Option<Hint> {
    Some(Hint { priority, label })
}

/// Two commands shown as one row in help and one hint in the footer, as
/// in "h/l previous / next column".
#[derive(Clone, Copy, Debug)]
pub struct Pair {
    pub with: CommandId,
    pub label: &'static str,
}

const fn pair(with: CommandId, label: &'static str) -> Option<Pair> {
    Some(Pair { with, label })
}

#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub id: CommandId,
    pub keys: &'static [Key],
    pub label: &'static str,
    pub group: Group,
    pub contexts: Contexts,
    pub hint: Option<Hint>,
    pub pair: Option<Pair>,
}

impl Command {
    /// All of the command's keys for the help overlay, as in "h / ←", or
    /// for a pair, "h/l  ←/→".
    pub fn keys_label(&self) -> String {
        match self.pair {
            Some(pair) => self
                .keys
                .iter()
                .zip(command(pair.with).keys)
                .map(|(first, second)| format!("{}/{}", first.label(), second.label()))
                .collect::<Vec<_>>()
                .join("  "),
            None => self
                .keys
                .iter()
                .map(|key| key.label())
                .collect::<Vec<_>>()
                .join(" / "),
        }
    }

    /// The label for the help overlay.
    pub fn help_label(&self) -> &'static str {
        self.pair.map_or(self.label, |pair| pair.label)
    }

    /// The keys shown in a footer hint: the first key, or the first key
    /// of both commands in a pair.
    pub fn hint_keys(&self) -> String {
        let first = self.keys[0].label();
        match self.pair {
            Some(pair) => format!("{first}/{}", command(pair.with).keys[0].label()),
            None => first,
        }
    }

    /// Whether this command is shown as part of another command's pair.
    pub fn is_paired_into_another(&self) -> bool {
        COMMANDS
            .iter()
            .any(|other| other.pair.is_some_and(|pair| pair.with == self.id))
    }
}

use CommandId as C;
use Group as G;

pub const COMMANDS: &[Command] = &[
    // Navigation
    Command {
        id: C::PreviousColumn,
        keys: &[ch('h'), key(KeyCode::Left)],
        label: "previous column",
        group: G::Navigation,
        contexts: CARDS,
        hint: hint(3, "column"),
        pair: pair(C::NextColumn, "previous / next column"),
    },
    Command {
        id: C::NextColumn,
        keys: &[ch('l'), key(KeyCode::Right)],
        label: "next column",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: None,
    },
    Command {
        id: C::PreviousCard,
        keys: &[ch('k'), key(KeyCode::Up)],
        label: "previous card",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: None,
    },
    Command {
        id: C::NextCard,
        keys: &[ch('j'), key(KeyCode::Down)],
        label: "next card",
        group: G::Navigation,
        contexts: CARDS,
        hint: hint(2, "card"),
        pair: pair(C::PreviousCard, "next / previous card"),
    },
    Command {
        id: C::PageUp,
        keys: &[key(KeyCode::PageUp)],
        label: "up five cards",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: pair(C::PageDown, "up / down five cards"),
    },
    Command {
        id: C::PageDown,
        keys: &[key(KeyCode::PageDown)],
        label: "down five cards",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: None,
    },
    Command {
        id: C::FirstCard,
        keys: &[key(KeyCode::Home)],
        label: "first card",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: pair(C::LastCard, "first / last card"),
    },
    Command {
        id: C::LastCard,
        keys: &[key(KeyCode::End)],
        label: "last card",
        group: G::Navigation,
        contexts: CARDS,
        hint: None,
        pair: None,
    },
    Command {
        id: C::RailPrevious,
        keys: &[ch('k'), key(KeyCode::Up), ch('h'), key(KeyCode::Left)],
        label: "rail: previous column",
        group: G::Navigation,
        contexts: only(Context::Rail),
        hint: None,
        pair: None,
    },
    Command {
        id: C::RailNext,
        keys: &[ch('j'), key(KeyCode::Down), ch('l'), key(KeyCode::Right)],
        label: "rail: next column",
        group: G::Navigation,
        contexts: only(Context::Rail),
        hint: hint(2, "column"),
        pair: pair(C::RailPrevious, "rail: next / previous column"),
    },
    Command {
        id: C::RailFirst,
        keys: &[key(KeyCode::Home)],
        label: "rail: first column",
        group: G::Navigation,
        contexts: only(Context::Rail),
        hint: None,
        pair: pair(C::RailLast, "rail: first / last column"),
    },
    Command {
        id: C::RailLast,
        keys: &[key(KeyCode::End)],
        label: "rail: last column",
        group: G::Navigation,
        contexts: only(Context::Rail),
        hint: None,
        pair: None,
    },
    Command {
        id: C::FocusCards,
        keys: &[key(KeyCode::Enter)],
        label: "rail: focus cards",
        group: G::Navigation,
        contexts: only(Context::Rail),
        hint: hint(3, "focus"),
        pair: None,
    },
    Command {
        id: C::ToggleFocus,
        keys: &[key(KeyCode::Tab), key(KeyCode::BackTab)],
        label: "focus rail / cards",
        group: G::Navigation,
        contexts: DASHBOARD,
        hint: hint(1, "rail"),
        pair: None,
    },
    Command {
        id: C::ToggleView,
        keys: &[ch('v')],
        label: "Board / All tasks",
        group: G::Navigation,
        contexts: DASHBOARD,
        hint: hint(4, "view"),
        pair: None,
    },
    // Tasks
    Command {
        id: C::OpenDetail,
        keys: &[key(KeyCode::Enter)],
        label: "open details",
        group: G::Tasks,
        contexts: CARDS,
        hint: hint(6, "open"),
        pair: None,
    },
    Command {
        id: C::NewTask,
        keys: &[ch('n')],
        label: "new task",
        group: G::Tasks,
        contexts: DASHBOARD,
        hint: hint(5, "new"),
        pair: None,
    },
    Command {
        id: C::EditTask,
        keys: &[ch('e')],
        label: "edit task",
        group: G::Tasks,
        contexts: TASK_ACTIONS,
        hint: hint(7, "edit"),
        pair: None,
    },
    Command {
        id: C::DeleteTask,
        keys: &[ch('d')],
        label: "delete task",
        group: G::Tasks,
        contexts: TASK_ACTIONS,
        hint: hint(9, "delete"),
        pair: None,
    },
    Command {
        id: C::MoveTaskLeft,
        keys: &[ch('H')],
        label: "move task left",
        group: G::Tasks,
        contexts: TASK_ACTIONS,
        hint: hint(8, "move"),
        pair: pair(C::MoveTaskRight, "move task left / right"),
    },
    Command {
        id: C::MoveTaskRight,
        keys: &[ch('L')],
        label: "move task right",
        group: G::Tasks,
        contexts: TASK_ACTIONS,
        hint: None,
        pair: None,
    },
    Command {
        id: C::Undo,
        keys: &[ch('u'), ctrl('z')],
        label: "undo",
        group: G::Tasks,
        contexts: DASHBOARD,
        hint: None,
        pair: pair(C::Redo, "undo / redo"),
    },
    Command {
        id: C::Redo,
        keys: &[ch('U'), ctrl('r')],
        label: "redo",
        group: G::Tasks,
        contexts: DASHBOARD,
        hint: None,
        pair: None,
    },
    Command {
        id: C::Search,
        keys: &[ch('/')],
        label: "search",
        group: G::Tasks,
        contexts: DASHBOARD,
        hint: hint(10, "search"),
        pair: None,
    },
    Command {
        id: C::ClearSearch,
        keys: &[key(KeyCode::Esc)],
        label: "clear search",
        group: G::Tasks,
        contexts: Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::Search,
        ]),
        hint: hint(0, "clear"),
        pair: None,
    },
    // Details
    Command {
        id: C::ScrollUp,
        keys: &[key(KeyCode::PageUp)],
        label: "scroll up",
        group: G::Details,
        contexts: only(Context::Detail),
        hint: hint(10, "scroll"),
        pair: pair(C::ScrollDown, "scroll up / down"),
    },
    Command {
        id: C::ScrollDown,
        keys: &[key(KeyCode::PageDown)],
        label: "scroll down",
        group: G::Details,
        contexts: only(Context::Detail),
        hint: None,
        pair: None,
    },
    Command {
        id: C::CloseDetail,
        keys: &[key(KeyCode::Esc)],
        label: "close details",
        group: G::Details,
        contexts: only(Context::Detail),
        hint: hint(9, "close"),
        pair: None,
    },
    // Editor, search and dialogs
    Command {
        id: C::ApplySearch,
        keys: &[key(KeyCode::Enter)],
        label: "search: apply",
        group: G::Editing,
        contexts: only(Context::Search),
        hint: hint(1, "apply"),
        pair: None,
    },
    Command {
        id: C::NextField,
        keys: &[key(KeyCode::Tab), key(KeyCode::BackTab)],
        label: "editor: switch field",
        group: G::Editing,
        contexts: only(Context::Editor),
        hint: hint(2, "switch field"),
        pair: None,
    },
    Command {
        id: C::SaveTask,
        keys: &[key(KeyCode::Enter)],
        label: "editor: save",
        group: G::Editing,
        contexts: only(Context::Editor),
        hint: hint(1, "save"),
        pair: None,
    },
    Command {
        id: C::CancelEdit,
        keys: &[key(KeyCode::Esc)],
        label: "editor: cancel",
        group: G::Editing,
        contexts: only(Context::Editor),
        hint: hint(3, "cancel"),
        pair: None,
    },
    Command {
        id: C::ConfirmDelete,
        keys: &[ch('y')],
        label: "delete: confirm",
        group: G::Editing,
        contexts: only(Context::Confirm),
        hint: hint(1, "delete"),
        pair: None,
    },
    Command {
        id: C::CancelDelete,
        keys: &[ch('n'), key(KeyCode::Esc)],
        label: "delete: cancel",
        group: G::Editing,
        contexts: only(Context::Confirm),
        hint: hint(2, "cancel"),
        pair: None,
    },
    // General
    Command {
        id: C::HelpScrollUp,
        keys: &[ch('k'), key(KeyCode::Up)],
        label: "help: scroll up",
        group: G::General,
        contexts: only(Context::Help),
        hint: None,
        pair: None,
    },
    Command {
        id: C::HelpScrollDown,
        keys: &[ch('j'), key(KeyCode::Down)],
        label: "help: scroll down",
        group: G::General,
        contexts: only(Context::Help),
        hint: hint(1, "scroll"),
        pair: pair(C::HelpScrollUp, "help: scroll down / up"),
    },
    Command {
        id: C::CloseHelp,
        keys: &[key(KeyCode::Esc), ch('q'), ch('?')],
        label: "help: close",
        group: G::General,
        contexts: only(Context::Help),
        hint: hint(2, "close"),
        pair: None,
    },
    Command {
        id: C::Help,
        keys: &[ch('?')],
        label: "this help",
        group: G::General,
        contexts: Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::Detail,
        ]),
        hint: hint(11, "help"),
        pair: None,
    },
    Command {
        id: C::Quit,
        keys: &[ch('q')],
        label: "quit",
        group: G::General,
        contexts: Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::TooSmall,
        ]),
        hint: None,
        pair: None,
    },
    Command {
        id: C::ForceQuit,
        keys: &[ctrl('c')],
        label: "quit (anywhere)",
        group: G::General,
        contexts: EVERYWHERE,
        hint: None,
        pair: None,
    },
];

pub fn command(id: CommandId) -> &'static Command {
    COMMANDS
        .iter()
        .find(|command| command.id == id)
        .expect("every CommandId has a row in COMMANDS")
}

/// The command a key runs in a context, if any.
pub fn lookup(context: Context, event: &KeyEvent) -> Option<CommandId> {
    COMMANDS
        .iter()
        .filter(|command| command.contexts.contains(context))
        .find(|command| command.keys.iter().any(|key| key.matches(event)))
        .map(|command| command.id)
}

/// The commands available in a context, for help and hints.
pub fn available(context: Context) -> impl Iterator<Item = &'static Command> {
    COMMANDS
        .iter()
        .filter(move |command| command.contexts.contains(context))
}

/// The footer hints for a context, most important first, as
/// `(keys, label)` pairs. `enabled` can hide commands that do nothing
/// right now, such as "clear" without a search.
pub fn hints(context: Context, enabled: impl Fn(CommandId) -> bool) -> Vec<(String, &'static str)> {
    let mut commands: Vec<(&Command, Hint)> = available(context)
        .filter(|command| enabled(command.id))
        .filter_map(|command| command.hint.map(|hint| (command, hint)))
        .collect();
    commands.sort_by_key(|(_, hint)| hint.priority);
    commands
        .into_iter()
        .map(|(command, hint)| (command.hint_keys(), hint.label))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn no_two_commands_share_a_key_in_a_context() {
        for context in Context::ALL {
            let commands: Vec<&Command> = available(context).collect();
            for (index, first) in commands.iter().enumerate() {
                for second in &commands[index + 1..] {
                    for key in first.keys {
                        assert!(
                            !second.keys.contains(key),
                            "{:?} and {:?} both use {} in {context:?}",
                            first.id,
                            second.id,
                            key.label()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_command_is_reachable() {
        for command in COMMANDS {
            assert!(!command.keys.is_empty(), "{:?} has no keys", command.id);
            assert!(
                Context::ALL
                    .iter()
                    .any(|context| command.contexts.contains(*context)),
                "{:?} has no context",
                command.id
            );
            assert_eq!(super::command(command.id).label, command.label);
        }
    }

    #[test]
    fn lookup_respects_context_and_case() {
        let h = event(KeyCode::Char('h'), KeyModifiers::NONE);
        let shift_h = event(KeyCode::Char('H'), KeyModifiers::SHIFT);
        assert_eq!(lookup(Context::Board, &h), Some(C::PreviousColumn));
        assert_eq!(lookup(Context::Rail, &h), Some(C::RailPrevious));
        assert_eq!(lookup(Context::Board, &shift_h), Some(C::MoveTaskLeft));
        assert_eq!(lookup(Context::Editor, &h), None);
        let ctrl_c = event(KeyCode::Char('c'), KeyModifiers::CONTROL);
        for context in Context::ALL {
            assert_eq!(lookup(context, &ctrl_c), Some(C::ForceQuit));
        }
        // Plain "c" is not Ctrl+C, and Ctrl+H is not h.
        let ctrl_h = event(KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert_eq!(lookup(Context::Board, &ctrl_h), None);
    }

    #[test]
    fn pairs_are_symmetric_and_share_contexts() {
        for command in COMMANDS {
            if let Some(pair) = command.pair {
                let other = super::command(pair.with);
                assert_eq!(command.keys.len(), other.keys.len(), "{:?}", command.id);
                assert_eq!(command.contexts, other.contexts, "{:?}", command.id);
                assert!(
                    other.pair.is_none() && other.hint.is_none(),
                    "{:?}",
                    other.id
                );
            }
        }
    }

    #[test]
    fn hints_pair_keys_and_follow_priority() {
        let hints = hints(Context::Board, |id| id != C::ClearSearch);
        assert_eq!(hints[0], ("Tab".to_owned(), "rail"));
        assert_eq!(hints[1], ("j/k".to_owned(), "card"));
        assert!(hints.contains(&("h/l".to_owned(), "column")));
        assert!(hints.contains(&("H/L".to_owned(), "move")));
    }
}
