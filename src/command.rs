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
    RowUp,
    RowDown,
    CardLeft,
    CardRight,
    PreviousGroup,
    NextGroup,
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
    LineUp,
    LineDown,
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

/// Two commands shown as one row in help and one hint in the footer, as
/// in "h/l previous / next column".
#[derive(Clone, Copy, Debug)]
pub struct Pair {
    pub with: CommandId,
    pub label: &'static str,
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
    const fn new(
        id: CommandId,
        keys: &'static [Key],
        label: &'static str,
        group: Group,
        contexts: Contexts,
    ) -> Self {
        Self {
            id,
            keys,
            label,
            group,
            contexts,
            hint: None,
            pair: None,
        }
    }

    /// Shows the command in the footer.
    const fn hint(self, priority: u8, label: &'static str) -> Self {
        Self {
            hint: Some(Hint { priority, label }),
            ..self
        }
    }

    /// Shows the command together with `with` in help and hints.
    const fn pair(self, with: CommandId, label: &'static str) -> Self {
        Self {
            pair: Some(Pair { with, label }),
            ..self
        }
    }

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
    Command::new(
        C::PreviousColumn,
        &[ch('h'), key(KeyCode::Left)],
        "previous column",
        G::Navigation,
        only(Context::Board),
    )
    .hint(3, "column")
    .pair(C::NextColumn, "previous / next column"),
    Command::new(
        C::NextColumn,
        &[ch('l'), key(KeyCode::Right)],
        "next column",
        G::Navigation,
        only(Context::Board),
    ),
    Command::new(
        C::PreviousCard,
        &[ch('k'), key(KeyCode::Up)],
        "previous card",
        G::Navigation,
        only(Context::Board),
    ),
    Command::new(
        C::NextCard,
        &[ch('j'), key(KeyCode::Down)],
        "next card",
        G::Navigation,
        only(Context::Board),
    )
    .hint(2, "card")
    .pair(C::PreviousCard, "next / previous card"),
    Command::new(
        C::CardLeft,
        &[ch('h'), key(KeyCode::Left)],
        "card to the left",
        G::Navigation,
        only(Context::AllTasks),
    )
    .hint(3, "card")
    .pair(C::CardRight, "card to the left / right"),
    Command::new(
        C::CardRight,
        &[ch('l'), key(KeyCode::Right)],
        "card to the right",
        G::Navigation,
        only(Context::AllTasks),
    ),
    Command::new(
        C::RowUp,
        &[ch('k'), key(KeyCode::Up)],
        "row up",
        G::Navigation,
        only(Context::AllTasks),
    ),
    Command::new(
        C::RowDown,
        &[ch('j'), key(KeyCode::Down)],
        "row down",
        G::Navigation,
        only(Context::AllTasks),
    )
    .hint(2, "row")
    .pair(C::RowUp, "row down / up"),
    Command::new(
        C::PreviousGroup,
        &[ch('[')],
        "previous column group",
        G::Navigation,
        only(Context::AllTasks),
    )
    .hint(4, "column")
    .pair(C::NextGroup, "previous / next column group"),
    Command::new(
        C::NextGroup,
        &[ch(']')],
        "next column group",
        G::Navigation,
        only(Context::AllTasks),
    ),
    Command::new(
        C::PageUp,
        &[key(KeyCode::PageUp)],
        "up five cards or rows",
        G::Navigation,
        CARDS,
    )
    .pair(C::PageDown, "up / down five cards or rows"),
    Command::new(
        C::PageDown,
        &[key(KeyCode::PageDown)],
        "down five cards or rows",
        G::Navigation,
        CARDS,
    ),
    Command::new(
        C::FirstCard,
        &[key(KeyCode::Home)],
        "first card",
        G::Navigation,
        CARDS,
    )
    .pair(C::LastCard, "first / last card"),
    Command::new(
        C::LastCard,
        &[key(KeyCode::End)],
        "last card",
        G::Navigation,
        CARDS,
    ),
    Command::new(
        C::RailPrevious,
        &[ch('k'), key(KeyCode::Up), ch('h'), key(KeyCode::Left)],
        "rail: previous column",
        G::Navigation,
        only(Context::Rail),
    ),
    Command::new(
        C::RailNext,
        &[ch('j'), key(KeyCode::Down), ch('l'), key(KeyCode::Right)],
        "rail: next column",
        G::Navigation,
        only(Context::Rail),
    )
    .hint(2, "column")
    .pair(C::RailPrevious, "rail: next / previous column"),
    Command::new(
        C::RailFirst,
        &[key(KeyCode::Home)],
        "rail: first column",
        G::Navigation,
        only(Context::Rail),
    )
    .pair(C::RailLast, "rail: first / last column"),
    Command::new(
        C::RailLast,
        &[key(KeyCode::End)],
        "rail: last column",
        G::Navigation,
        only(Context::Rail),
    ),
    Command::new(
        C::FocusCards,
        &[key(KeyCode::Enter)],
        "rail: focus cards",
        G::Navigation,
        only(Context::Rail),
    )
    .hint(3, "focus"),
    Command::new(
        C::ToggleFocus,
        &[key(KeyCode::Tab), key(KeyCode::BackTab)],
        "focus rail / cards",
        G::Navigation,
        DASHBOARD,
    )
    .hint(1, "rail"),
    Command::new(
        C::ToggleView,
        &[ch('v')],
        "Board / All tasks",
        G::Navigation,
        DASHBOARD,
    )
    .hint(4, "view"),
    // Tasks
    Command::new(
        C::OpenDetail,
        &[key(KeyCode::Enter)],
        "open details",
        G::Tasks,
        CARDS,
    )
    .hint(6, "open"),
    Command::new(C::NewTask, &[ch('n')], "new task", G::Tasks, DASHBOARD).hint(5, "new"),
    Command::new(C::EditTask, &[ch('e')], "edit task", G::Tasks, TASK_ACTIONS).hint(7, "edit"),
    Command::new(
        C::DeleteTask,
        &[ch('d')],
        "delete task",
        G::Tasks,
        TASK_ACTIONS,
    )
    .hint(9, "delete"),
    Command::new(
        C::MoveTaskLeft,
        &[ch('H')],
        "move task left",
        G::Tasks,
        TASK_ACTIONS,
    )
    .hint(8, "move")
    .pair(C::MoveTaskRight, "move task left / right"),
    Command::new(
        C::MoveTaskRight,
        &[ch('L')],
        "move task right",
        G::Tasks,
        TASK_ACTIONS,
    ),
    Command::new(C::Undo, &[ch('u'), ctrl('z')], "undo", G::Tasks, DASHBOARD)
        .pair(C::Redo, "undo / redo"),
    Command::new(C::Redo, &[ch('U'), ctrl('r')], "redo", G::Tasks, DASHBOARD),
    Command::new(C::Search, &[ch('/')], "search", G::Tasks, DASHBOARD).hint(10, "search"),
    Command::new(
        C::ClearSearch,
        &[key(KeyCode::Esc)],
        "clear search",
        G::Tasks,
        Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::Search,
        ]),
    )
    .hint(0, "clear"),
    // Details
    Command::new(
        C::ScrollUp,
        &[key(KeyCode::PageUp)],
        "scroll up a page",
        G::Details,
        only(Context::Detail),
    )
    .pair(C::ScrollDown, "scroll up / down a page"),
    Command::new(
        C::ScrollDown,
        &[key(KeyCode::PageDown)],
        "scroll down a page",
        G::Details,
        only(Context::Detail),
    ),
    Command::new(
        C::LineDown,
        &[ch('j'), key(KeyCode::Down)],
        "scroll down a line",
        G::Details,
        only(Context::Detail),
    )
    .hint(10, "scroll")
    .pair(C::LineUp, "scroll down / up a line"),
    Command::new(
        C::LineUp,
        &[ch('k'), key(KeyCode::Up)],
        "scroll up a line",
        G::Details,
        only(Context::Detail),
    ),
    Command::new(
        C::CloseDetail,
        &[key(KeyCode::Esc)],
        "close details",
        G::Details,
        only(Context::Detail),
    )
    .hint(9, "close"),
    // Editor, search and dialogs
    Command::new(
        C::ApplySearch,
        &[key(KeyCode::Enter)],
        "search: apply",
        G::Editing,
        only(Context::Search),
    )
    .hint(1, "apply"),
    Command::new(
        C::NextField,
        &[key(KeyCode::Tab), key(KeyCode::BackTab)],
        "editor: switch field",
        G::Editing,
        only(Context::Editor),
    )
    .hint(2, "switch field"),
    Command::new(
        C::SaveTask,
        &[key(KeyCode::Enter)],
        "editor: save",
        G::Editing,
        only(Context::Editor),
    )
    .hint(1, "save"),
    Command::new(
        C::CancelEdit,
        &[key(KeyCode::Esc)],
        "editor: cancel",
        G::Editing,
        only(Context::Editor),
    )
    .hint(3, "cancel"),
    Command::new(
        C::ConfirmDelete,
        &[ch('y')],
        "delete: confirm",
        G::Editing,
        only(Context::Confirm),
    )
    .hint(1, "delete"),
    Command::new(
        C::CancelDelete,
        &[ch('n'), key(KeyCode::Esc)],
        "delete: cancel",
        G::Editing,
        only(Context::Confirm),
    )
    .hint(2, "cancel"),
    // General
    Command::new(
        C::HelpScrollUp,
        &[ch('k'), key(KeyCode::Up)],
        "help: scroll up",
        G::General,
        only(Context::Help),
    ),
    Command::new(
        C::HelpScrollDown,
        &[ch('j'), key(KeyCode::Down)],
        "help: scroll down",
        G::General,
        only(Context::Help),
    )
    .hint(1, "scroll")
    .pair(C::HelpScrollUp, "help: scroll down / up"),
    Command::new(
        C::CloseHelp,
        &[key(KeyCode::Esc), ch('q'), ch('?')],
        "help: close",
        G::General,
        only(Context::Help),
    )
    .hint(2, "close"),
    Command::new(
        C::Help,
        &[ch('?')],
        "this help",
        G::General,
        Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::Detail,
        ]),
    )
    .hint(11, "help"),
    Command::new(
        C::Quit,
        &[ch('q')],
        "quit",
        G::General,
        Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::TooSmall,
        ]),
    ),
    Command::new(
        C::ForceQuit,
        &[ctrl('c')],
        "quit (anywhere)",
        G::General,
        EVERYWHERE,
    ),
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
        let hints = super::hints(Context::AllTasks, |_| true);
        assert!(hints.contains(&("j/k".to_owned(), "row")));
        assert!(hints.contains(&("h/l".to_owned(), "card")));
        assert!(hints.contains(&("[/]".to_owned(), "column")));
    }
}
