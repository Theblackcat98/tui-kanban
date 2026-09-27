//! Every keybinding, defined once.
//!
//! Key handling looks a key up here to get a [`CommandId`], and the help
//! overlay and the footer hints are generated from the same table, so
//! they can't drift apart.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::OnceLock;

/// Where the user is, which decides what keys mean.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
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
    /// The "move to…" menu.
    MoveTo = 1 << 9,
    /// The quick-add prompt.
    QuickAdd = 1 << 10,
    /// "Discard changes?" after cancelling a changed draft.
    Discard = 1 << 11,
    /// The command palette.
    Palette = 1 << 12,
    /// "Board changed on disk" while there are unsaved changes.
    Conflict = 1 << 13,
    /// "Quit without saving?"
    ConfirmQuit = 1 << 14,
    /// A prompt for a column's name or work-in-progress limit.
    Prompt = 1 << 15,
    /// "Delete column?"
    DeleteColumn = 1 << 16,
    /// The colour menu for a column.
    Colors = 1 << 17,
    /// Moving a task into a column at its work-in-progress limit.
    ConfirmWip = 1 << 18,
    /// The board switcher.
    Boards = 1 << 19,
    /// No board was found where tui-kanban started.
    NoBoard = 1 << 20,
}

impl Context {
    pub const ALL: [Context; 21] = [
        Context::Board,
        Context::AllTasks,
        Context::Rail,
        Context::Search,
        Context::Detail,
        Context::Editor,
        Context::Help,
        Context::Confirm,
        Context::TooSmall,
        Context::MoveTo,
        Context::QuickAdd,
        Context::Discard,
        Context::Palette,
        Context::Conflict,
        Context::ConfirmQuit,
        Context::Prompt,
        Context::DeleteColumn,
        Context::Colors,
        Context::ConfirmWip,
        Context::Boards,
        Context::NoBoard,
    ];

    /// The name shown in the status line's mode pill and help's title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "BOARD",
            Self::AllTasks => "ALL TASKS",
            Self::Rail => "RAIL",
            Self::Search => "SEARCH",
            Self::Detail => "DETAIL",
            Self::Editor => "EDIT",
            Self::Help => "HELP",
            Self::Confirm => "DELETE",
            Self::TooSmall => "",
            Self::MoveTo => "MOVE",
            Self::QuickAdd => "ADD",
            Self::Discard => "DISCARD",
            Self::Palette => "COMMAND",
            Self::Conflict => "CONFLICT",
            Self::ConfirmQuit => "QUIT",
            Self::Prompt => "COLUMN",
            Self::DeleteColumn => "DELETE",
            Self::Colors => "COLOUR",
            Self::ConfirmWip => "LIMIT",
            Self::Boards => "BOARDS",
            Self::NoBoard => "WELCOME",
        }
    }

    /// The name help uses for the context, as in "Keys · Board".
    pub fn title(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::AllTasks => "All tasks",
            Self::Rail => "Column rail",
            Self::Search => "Search",
            Self::Detail => "Details",
            Self::Editor => "Editor",
            Self::Help => "Help",
            Self::Confirm => "Delete",
            Self::TooSmall => "",
            Self::MoveTo => "Move to",
            Self::QuickAdd => "Quick add",
            Self::Discard => "Discard changes",
            Self::Palette => "Command palette",
            Self::Conflict => "Changed on disk",
            Self::ConfirmQuit => "Quit",
            Self::Prompt => "Column",
            Self::DeleteColumn => "Delete column",
            Self::Colors => "Column colour",
            Self::ConfirmWip => "Work-in-progress limit",
            Self::Boards => "Boards",
            Self::NoBoard => "No board here",
        }
    }
}

/// A set of contexts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Contexts(u32);

impl Contexts {
    const fn of(contexts: &[Context]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < contexts.len() {
            bits |= contexts[index] as u32;
            index += 1;
        }
        Self(bits)
    }

    pub fn contains(self, context: Context) -> bool {
        self.0 & context as u32 != 0
    }
}

const CARDS: Contexts = Contexts::of(&[Context::Board, Context::AllTasks]);
const DASHBOARD: Contexts = Contexts::of(&[Context::Board, Context::AllTasks, Context::Rail]);
const UNDO: Contexts = Contexts::of(&[
    Context::Board,
    Context::AllTasks,
    Context::Rail,
    Context::Detail,
]);
const TASK_ACTIONS: Contexts = Contexts::of(&[Context::Board, Context::AllTasks, Context::Detail]);
const MENUS: Contexts = Contexts::of(&[Context::MoveTo, Context::Colors, Context::Boards]);

const fn only(context: Context) -> Contexts {
    Contexts(context as u32)
}

const fn except(contexts: &[Context]) -> Contexts {
    Contexts(!Contexts::of(contexts).0)
}

const DIGITS: [Key; 9] = [
    ch('1'),
    ch('2'),
    ch('3'),
    ch('4'),
    ch('5'),
    ch('6'),
    ch('7'),
    ch('8'),
    ch('9'),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    Navigation,
    Tasks,
    Columns,
    Details,
    Editing,
    General,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Navigation,
        Group::Tasks,
        Group::Columns,
        Group::Details,
        Group::Editing,
        Group::General,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Tasks => "Tasks",
            Self::Columns => "Columns",
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
    JumpToLane,
    GoPrefix,
    GoToLane,
    FocusCards,
    ToggleFocus,
    ToggleView,
    OpenDetail,
    NewTask,
    NewTaskAbove,
    QuickAdd,
    EditTask,
    DuplicateTask,
    EditExternally,
    DraftInEditor,
    MoveTo,
    MoveTaskUp,
    MoveTaskDown,
    DeleteTask,
    MoveTaskLeft,
    MoveTaskRight,
    AddColumn,
    RenameColumn,
    DeleteColumn,
    MoveColumnUp,
    MoveColumnDown,
    ColumnColor,
    WipLimit,
    ToggleCollapse,
    Undo,
    Redo,
    Search,
    ClearSearch,
    ApplySearch,
    SearchDown,
    SearchUp,
    NextMatch,
    PreviousMatch,
    RemoveFilterTerm,
    ScrollUp,
    ScrollDown,
    ToggleItem,
    NextItem,
    PreviousItem,
    LineUp,
    LineDown,
    CloseDetail,
    NextField,
    SaveTask,
    CancelEdit,
    ConfirmDelete,
    CancelDelete,
    DiscardChanges,
    KeepEditing,
    MenuUp,
    MenuDown,
    MenuPick,
    CloseMenu,
    AddQuickTask,
    CloseQuickAdd,
    PromptSave,
    PromptCancel,
    ConfirmDeleteColumn,
    TasksToPrevious,
    TasksToNext,
    CancelDeleteColumn,
    PickColor,
    MoveAnyway,
    CancelMove,
    SwitchBoard,
    OpenBoard,
    CreateHere,
    OpenPersonal,
    HelpScrollUp,
    HelpScrollDown,
    CloseHelp,
    Help,
    OpenPalette,
    Leader,
    PaletteDown,
    PaletteUp,
    PaletteRun,
    PaletteClose,
    ReloadFromDisk,
    KeepMine,
    DecideLater,
    QuitWithoutSaving,
    KeepOpen,
    Quit,
    ForceQuit,
}

/// One key, as written in the table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Key {
    /// A key pressed first, as the `g` in `g g`.
    pub prefix: Option<char>,
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

const fn key(code: KeyCode) -> Key {
    Key {
        prefix: None,
        code,
        modifiers: KeyModifiers::NONE,
    }
}

/// A key pressed after `g`.
const fn after_g(character: char) -> Key {
    Key {
        prefix: Some('g'),
        ..ch(character)
    }
}

const fn ch(character: char) -> Key {
    key(KeyCode::Char(character))
}

const fn ctrl(character: char) -> Key {
    Key {
        prefix: None,
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
        let name = if self.modifiers.contains(KeyModifiers::CONTROL) {
            format!("Ctrl+{}", name.to_uppercase())
        } else {
            name
        };
        match self.prefix {
            Some(prefix) => format!("{prefix} {name}"),
            None => name,
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
    /// How help shows the keys, when listing them would be too long (as
    /// "1–9") or they aren't fixed (as "g + letter").
    pub shown_keys: Option<&'static str>,
    /// Whether help and the command palette list the command.
    pub listed: bool,
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
            shown_keys: None,
            listed: true,
        }
    }

    const fn shown_as(self, keys: &'static str) -> Self {
        Self {
            shown_keys: Some(keys),
            ..self
        }
    }

    /// Leaves the command out of help and the palette, for keys that only
    /// start something that is listed on its own, such as `g`.
    const fn unlisted(self) -> Self {
        Self {
            listed: false,
            ..self
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
        if let Some(keys) = self.shown_keys {
            return keys.to_owned();
        }
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
        if let Some(keys) = self.shown_keys {
            return keys.to_owned();
        }
        let first = self.keys[0].label();
        match self.pair {
            Some(pair) => format!("{first}/{}", command(pair.with).keys[0].label()),
            None => first,
        }
    }

    /// Whether this command is shown as part of another command's pair.
    pub fn is_paired_into_another(&self) -> bool {
        commands()
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
        &[key(KeyCode::Home), after_g('g')],
        "first card",
        G::Navigation,
        CARDS,
    )
    .pair(C::LastCard, "first / last card"),
    Command::new(
        C::LastCard,
        &[key(KeyCode::End), ch('G')],
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
        C::JumpToLane,
        &DIGITS,
        "lane by its number",
        G::Navigation,
        Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::MoveTo,
        ]),
    )
    .shown_as("1–9"),
    Command::new(C::GoPrefix, &[ch('g')], "go to…", G::Navigation, CARDS).unlisted(),
    Command::new(
        C::GoToLane,
        &[],
        "lane by its initial",
        G::Navigation,
        CARDS,
    )
    .shown_as("g + letter"),
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
    Command::new(
        C::SwitchBoard,
        &[ch('b')],
        "switch board",
        G::Navigation,
        DASHBOARD,
    ),
    // Tasks
    Command::new(
        C::OpenDetail,
        &[key(KeyCode::Enter)],
        "open details",
        G::Tasks,
        CARDS,
    )
    .hint(6, "open"),
    Command::new(
        C::NewTask,
        &[ch('n')],
        "new task below",
        G::Tasks,
        DASHBOARD,
    )
    .hint(5, "new")
    .pair(C::NewTaskAbove, "new task below / above"),
    Command::new(
        C::NewTaskAbove,
        &[ch('N')],
        "new task above",
        G::Tasks,
        DASHBOARD,
    ),
    Command::new(
        C::QuickAdd,
        &[ch('a')],
        "quick add to this lane",
        G::Tasks,
        CARDS,
    ),
    Command::new(C::EditTask, &[ch('e')], "edit task", G::Tasks, TASK_ACTIONS).hint(7, "edit"),
    Command::new(
        C::DuplicateTask,
        &[ch('y')],
        "duplicate task",
        G::Tasks,
        TASK_ACTIONS,
    ),
    Command::new(C::MoveTo, &[ch('m')], "move to…", G::Tasks, TASK_ACTIONS),
    Command::new(
        C::EditExternally,
        &[ch('E')],
        "edit description in $EDITOR",
        G::Tasks,
        TASK_ACTIONS,
    ),
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
    Command::new(
        C::MoveTaskDown,
        &[ch('J')],
        "move task down",
        G::Tasks,
        TASK_ACTIONS,
    )
    .pair(C::MoveTaskUp, "move task down / up"),
    Command::new(
        C::MoveTaskUp,
        &[ch('K')],
        "move task up",
        G::Tasks,
        TASK_ACTIONS,
    ),
    // Columns
    Command::new(
        C::AddColumn,
        &[ch('a')],
        "add a column",
        G::Columns,
        only(Context::Rail),
    )
    .hint(5, "add column"),
    Command::new(
        C::RenameColumn,
        &[ch('r')],
        "rename column",
        G::Columns,
        only(Context::Rail),
    )
    .hint(6, "rename"),
    Command::new(
        C::DeleteColumn,
        &[ch('d')],
        "delete column",
        G::Columns,
        only(Context::Rail),
    )
    .hint(9, "delete"),
    Command::new(
        C::MoveColumnDown,
        &[ch('J')],
        "move column down",
        G::Columns,
        only(Context::Rail),
    )
    .hint(7, "move")
    .pair(C::MoveColumnUp, "move column down / up"),
    Command::new(
        C::MoveColumnUp,
        &[ch('K')],
        "move column up",
        G::Columns,
        only(Context::Rail),
    ),
    Command::new(
        C::ColumnColor,
        &[ch('c')],
        "column colour",
        G::Columns,
        only(Context::Rail),
    )
    .hint(8, "colour"),
    Command::new(
        C::WipLimit,
        &[ch('w')],
        "work-in-progress limit",
        G::Columns,
        only(Context::Rail),
    )
    .hint(8, "limit"),
    Command::new(
        C::ToggleCollapse,
        &[ch('z')],
        "collapse / expand lane",
        G::Columns,
        Contexts::of(&[Context::Board, Context::Rail]),
    ),
    Command::new(C::Undo, &[ch('u'), ctrl('z')], "undo", G::Tasks, UNDO)
        .pair(C::Redo, "undo / redo"),
    Command::new(C::Redo, &[ch('U'), ctrl('r')], "redo", G::Tasks, UNDO),
    Command::new(C::Search, &[ch('/')], "search", G::Tasks, DASHBOARD).hint(10, "search"),
    Command::new(
        C::ClearSearch,
        &[key(KeyCode::Esc)],
        "back / clear search",
        G::Tasks,
        Contexts::of(&[
            Context::Board,
            Context::AllTasks,
            Context::Rail,
            Context::Search,
        ]),
    )
    .hint(0, "clear"),
    Command::new(C::NextMatch, &[ctrl('n')], "next match", G::Tasks, CARDS)
        .pair(C::PreviousMatch, "next / previous match"),
    Command::new(
        C::PreviousMatch,
        &[ctrl('p')],
        "previous match",
        G::Tasks,
        CARDS,
    ),
    Command::new(
        C::RemoveFilterTerm,
        &[key(KeyCode::Backspace)],
        "remove the last filter term",
        G::Tasks,
        DASHBOARD,
    ),
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
        C::ToggleItem,
        &[ch(' ')],
        "tick / untick checklist item",
        G::Details,
        only(Context::Detail),
    )
    .hint(8, "tick"),
    Command::new(
        C::NextItem,
        &[key(KeyCode::Tab)],
        "next checklist item",
        G::Details,
        only(Context::Detail),
    )
    .hint(9, "item")
    .pair(C::PreviousItem, "next / previous checklist item"),
    Command::new(
        C::PreviousItem,
        &[key(KeyCode::BackTab)],
        "previous checklist item",
        G::Details,
        only(Context::Detail),
    ),
    Command::new(
        C::CloseDetail,
        &[key(KeyCode::Esc), ch('q')],
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
        C::SearchDown,
        &[key(KeyCode::Down), ctrl('n')],
        "search: next match",
        G::Editing,
        only(Context::Search),
    )
    .hint(2, "match")
    .pair(C::SearchUp, "search: next / previous match"),
    Command::new(
        C::SearchUp,
        &[key(KeyCode::Up), ctrl('p')],
        "search: previous match",
        G::Editing,
        only(Context::Search),
    ),
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
        &[ctrl('s'), key(KeyCode::Enter)],
        "editor: save (Enter in the title)",
        G::Editing,
        only(Context::Editor),
    )
    .hint(1, "save"),
    Command::new(
        C::CancelEdit,
        &[key(KeyCode::Esc), ctrl('c')],
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
        &[ch('n'), key(KeyCode::Esc), ch('q')],
        "delete: cancel",
        G::Editing,
        only(Context::Confirm),
    )
    .hint(2, "cancel"),
    Command::new(
        C::DraftInEditor,
        &[ctrl('o')],
        "editor: description in $EDITOR",
        G::Editing,
        only(Context::Editor),
    ),
    Command::new(
        C::PaletteDown,
        &[key(KeyCode::Down), ctrl('n')],
        "palette: next",
        G::Editing,
        only(Context::Palette),
    )
    .hint(2, "choose")
    .pair(C::PaletteUp, "palette: next / previous"),
    Command::new(
        C::PaletteUp,
        &[key(KeyCode::Up), ctrl('p')],
        "palette: previous",
        G::Editing,
        only(Context::Palette),
    ),
    Command::new(
        C::PaletteRun,
        &[key(KeyCode::Enter)],
        "palette: run",
        G::Editing,
        only(Context::Palette),
    )
    .hint(1, "run"),
    Command::new(
        C::PaletteClose,
        &[key(KeyCode::Esc), ctrl('c')],
        "palette: close",
        G::Editing,
        only(Context::Palette),
    )
    .hint(3, "close"),
    Command::new(
        C::DiscardChanges,
        &[ch('y')],
        "discard: confirm",
        G::Editing,
        only(Context::Discard),
    )
    .hint(1, "discard"),
    Command::new(
        C::KeepEditing,
        &[ch('n'), key(KeyCode::Esc), ch('q')],
        "discard: keep editing",
        G::Editing,
        only(Context::Discard),
    )
    .hint(2, "keep editing"),
    Command::new(
        C::ReloadFromDisk,
        &[ch('r')],
        "changed on disk: load it (u brings yours back)",
        G::Editing,
        only(Context::Conflict),
    )
    .hint(1, "load it"),
    Command::new(
        C::KeepMine,
        &[ch('o')],
        "changed on disk: overwrite it with yours",
        G::Editing,
        only(Context::Conflict),
    )
    .hint(2, "keep mine"),
    Command::new(
        C::DecideLater,
        &[key(KeyCode::Esc)],
        "changed on disk: decide later",
        G::Editing,
        only(Context::Conflict),
    )
    .hint(3, "later"),
    Command::new(
        C::QuitWithoutSaving,
        &[ch('y')],
        "quit: without saving",
        G::Editing,
        only(Context::ConfirmQuit),
    )
    .hint(1, "quit anyway"),
    Command::new(
        C::KeepOpen,
        &[ch('n'), key(KeyCode::Esc)],
        "quit: stay",
        G::Editing,
        only(Context::ConfirmQuit),
    )
    .hint(2, "stay"),
    Command::new(
        C::MenuDown,
        &[ch('j'), key(KeyCode::Down)],
        "menu: next",
        G::Editing,
        MENUS,
    )
    .hint(2, "choose")
    .pair(C::MenuUp, "menu: next / previous"),
    Command::new(
        C::MenuUp,
        &[ch('k'), key(KeyCode::Up)],
        "menu: previous",
        G::Editing,
        MENUS,
    ),
    Command::new(
        C::MenuPick,
        &[key(KeyCode::Enter)],
        "menu: pick",
        G::Editing,
        only(Context::MoveTo),
    )
    .hint(1, "move"),
    Command::new(
        C::CloseMenu,
        &[key(KeyCode::Esc), ch('q')],
        "menu: close",
        G::Editing,
        MENUS,
    )
    .hint(3, "cancel"),
    Command::new(
        C::OpenBoard,
        &[key(KeyCode::Enter)],
        "boards: open",
        G::Editing,
        only(Context::Boards),
    )
    .hint(1, "open"),
    Command::new(
        C::CreateHere,
        &[ch('c')],
        "create a board here",
        G::Editing,
        only(Context::NoBoard),
    )
    .hint(1, "create here"),
    Command::new(
        C::OpenPersonal,
        &[ch('p')],
        "open your personal board",
        G::Editing,
        only(Context::NoBoard),
    )
    .hint(2, "personal board"),
    Command::new(
        C::PickColor,
        &[key(KeyCode::Enter)],
        "colour: set",
        G::Editing,
        only(Context::Colors),
    )
    .hint(1, "pick"),
    Command::new(
        C::PromptSave,
        &[key(KeyCode::Enter)],
        "column: save",
        G::Editing,
        only(Context::Prompt),
    )
    .hint(1, "save"),
    Command::new(
        C::PromptCancel,
        &[key(KeyCode::Esc), ctrl('c')],
        "column: cancel",
        G::Editing,
        only(Context::Prompt),
    )
    .hint(2, "cancel"),
    Command::new(
        C::ConfirmDeleteColumn,
        &[ch('y')],
        "delete column: confirm",
        G::Editing,
        only(Context::DeleteColumn),
    )
    .hint(1, "delete"),
    Command::new(
        C::TasksToPrevious,
        &[ch('h'), key(KeyCode::Left)],
        "delete column: previous place for its tasks",
        G::Editing,
        only(Context::DeleteColumn),
    )
    .hint(2, "tasks go to")
    .pair(C::TasksToNext, "delete column: where its tasks go"),
    Command::new(
        C::TasksToNext,
        &[ch('l'), key(KeyCode::Right)],
        "delete column: next place for its tasks",
        G::Editing,
        only(Context::DeleteColumn),
    ),
    Command::new(
        C::CancelDeleteColumn,
        &[ch('n'), key(KeyCode::Esc), ch('q')],
        "delete column: cancel",
        G::Editing,
        only(Context::DeleteColumn),
    )
    .hint(3, "cancel"),
    Command::new(
        C::MoveAnyway,
        &[ch('y')],
        "limit: move anyway",
        G::Editing,
        only(Context::ConfirmWip),
    )
    .hint(1, "move anyway"),
    Command::new(
        C::CancelMove,
        &[ch('n'), key(KeyCode::Esc), ch('q')],
        "limit: don't move",
        G::Editing,
        only(Context::ConfirmWip),
    )
    .hint(2, "cancel"),
    Command::new(
        C::AddQuickTask,
        &[key(KeyCode::Enter)],
        "quick add: add",
        G::Editing,
        only(Context::QuickAdd),
    )
    .hint(1, "add"),
    Command::new(
        C::CloseQuickAdd,
        &[key(KeyCode::Esc), ctrl('c')],
        "quick add: done",
        G::Editing,
        only(Context::QuickAdd),
    )
    .hint(2, "done"),
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
        C::OpenPalette,
        &[ch(':'), ctrl('k')],
        "command palette",
        G::General,
        UNDO,
    )
    .hint(12, "commands"),
    Command::new(
        C::Leader,
        &[ch(' ')],
        "show the keys you can press",
        G::General,
        DASHBOARD,
    ),
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
            Context::NoBoard,
        ]),
    ),
    Command::new(
        C::ForceQuit,
        &[ctrl('c')],
        "quit (anywhere)",
        G::General,
        // Ctrl+C cancels typing rather than losing it.
        except(&[
            Context::QuickAdd,
            Context::Editor,
            Context::Palette,
            Context::Prompt,
        ]),
    ),
];

/// The command table in use: [`COMMANDS`], with any keys the config file
/// changed (see [`remap`] and [`install`]).
pub fn commands() -> &'static [Command] {
    ACTIVE.get().copied().unwrap_or(COMMANDS)
}

static ACTIVE: OnceLock<&'static [Command]> = OnceLock::new();

/// Makes `table` the command table for the rest of the run. Only the
/// first call has any effect.
pub fn install(table: &'static [Command]) {
    let _ = ACTIVE.set(table);
}

/// Commands whose keys can't be changed: digits and `g` + letter carry
/// the lane they name, and `g` and Space start the chords after them.
const FIXED: [CommandId; 4] = [
    CommandId::JumpToLane,
    CommandId::GoToLane,
    CommandId::GoPrefix,
    CommandId::Leader,
];

impl CommandId {
    /// The command's name in the config file, as in `new_task`.
    pub fn name(self) -> String {
        let mut name = String::new();
        for (index, character) in format!("{self:?}").chars().enumerate() {
            if character.is_uppercase() && index > 0 {
                name.push('_');
            }
            name.extend(character.to_lowercase());
        }
        name
    }

    /// The command called `name` in the config file.
    pub fn from_name(name: &str) -> Option<Self> {
        COMMANDS
            .iter()
            .map(|command| command.id)
            .find(|id| id.name() == name)
    }

    /// Whether the config file can change the command's keys.
    pub fn remappable(self) -> bool {
        !FIXED.contains(&self)
    }
}

/// The command table with some commands' keys replaced, checked so that
/// no two commands share a key where both apply. The table is kept for
/// the rest of the run, so pass it to [`install`].
pub fn remap(changes: &[(CommandId, Vec<Key>)]) -> Result<&'static [Command], String> {
    let mut table: Vec<Command> = COMMANDS.to_vec();
    for (id, keys) in changes {
        if !id.remappable() {
            return Err(format!("the keys for {} can't be changed", id.name()));
        }
        if keys
            .iter()
            .any(|key| key.prefix.is_some_and(|prefix| prefix != 'g'))
        {
            return Err(format!(
                "{}: only g can start a two-key chord, as in \"g g\"",
                id.name()
            ));
        }
        let command = table
            .iter_mut()
            .find(|command| command.id == *id)
            .expect("every CommandId has a row in COMMANDS");
        command.keys = Box::leak(keys.clone().into_boxed_slice());
    }
    check_conflicts(&table)?;
    Ok(Box::leak(table.into_boxed_slice()))
}

/// Fails if two commands share a key in a context where both apply.
fn check_conflicts(table: &[Command]) -> Result<(), String> {
    for context in Context::ALL {
        let commands: Vec<&Command> = table
            .iter()
            .filter(|command| command.contexts.contains(context))
            .collect();
        for (index, first) in commands.iter().enumerate() {
            for second in &commands[index + 1..] {
                if let Some(key) = first.keys.iter().find(|key| second.keys.contains(key)) {
                    return Err(format!(
                        "{} is used by both {} and {} ({})",
                        key.config_name(),
                        first.id.name(),
                        second.id.name(),
                        context.title()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Key names in the config file, and what they mean.
const KEY_NAMES: [(&str, KeyCode); 16] = [
    ("space", KeyCode::Char(' ')),
    ("enter", KeyCode::Enter),
    ("esc", KeyCode::Esc),
    ("escape", KeyCode::Esc),
    ("tab", KeyCode::Tab),
    ("backtab", KeyCode::BackTab),
    ("backspace", KeyCode::Backspace),
    ("del", KeyCode::Delete),
    ("delete", KeyCode::Delete),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pgup", KeyCode::PageUp),
];

impl Key {
    /// Reads a key as written in the config file: a character (`n`, `N`,
    /// `?`), a name (`enter`, `pgdn`, `f5`), either with `ctrl+`, `alt+`
    /// or `shift+` in front, or `g` and a key for a chord (`g g`).
    pub fn parse(text: &str) -> Result<Key, String> {
        let text = text.trim();
        let invalid = || format!("unknown key {text:?}");
        let (prefix, rest) = match text.split_once(' ') {
            Some((prefix, rest)) if !rest.trim().is_empty() => {
                let mut characters = prefix.chars();
                match (characters.next(), characters.next()) {
                    (Some(prefix), None) => (Some(prefix), rest.trim()),
                    _ => return Err(invalid()),
                }
            }
            _ => (None, text),
        };
        let mut modifiers = KeyModifiers::NONE;
        let mut name = rest;
        // "+" alone, or at the end as in "ctrl++", is the plus key.
        while let Some((modifier, after)) =
            name.split_once('+').filter(|(_, after)| !after.is_empty())
        {
            modifiers |= match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "meta" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                _ => return Err(invalid()),
            };
            name = after;
        }
        let mut characters = name.chars();
        let code = match (characters.next(), characters.next()) {
            (Some(character), None) => KeyCode::Char(character),
            _ => {
                let lower = name.to_lowercase();
                let named = KEY_NAMES
                    .iter()
                    .chain(&[("pgdn", KeyCode::PageDown), ("pageup", KeyCode::PageUp)])
                    .chain(&[("pagedown", KeyCode::PageDown), ("insert", KeyCode::Insert)])
                    .find(|(candidate, _)| *candidate == lower)
                    .map(|(_, code)| *code);
                match named {
                    Some(code) => code,
                    None => match lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
                        Some(number @ 1..=12) => KeyCode::F(number),
                        _ => return Err(invalid()),
                    },
                }
            }
        };
        // Shift is part of a character (H, not shift+h), and Shift+Tab
        // is its own key.
        let code = match code {
            KeyCode::Char(character) if modifiers.contains(KeyModifiers::SHIFT) => {
                KeyCode::Char(character.to_ascii_uppercase())
            }
            KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
            code => code,
        };
        modifiers.remove(KeyModifiers::SHIFT);
        let code = match code {
            KeyCode::Char(character) if modifiers.contains(KeyModifiers::CONTROL) => {
                KeyCode::Char(character.to_ascii_lowercase())
            }
            code => code,
        };
        Ok(Key {
            prefix,
            code,
            modifiers,
        })
    }

    /// The key as written in the config file; [`Key::parse`] reads it back.
    pub fn config_name(self) -> String {
        let name = match self.code {
            KeyCode::Char(character) => KEY_NAMES
                .iter()
                .find(|(_, code)| *code == self.code)
                .map_or(character.to_string(), |(name, _)| (*name).to_owned()),
            KeyCode::BackTab => "shift+tab".to_owned(),
            KeyCode::PageDown => "pgdn".to_owned(),
            KeyCode::F(number) => format!("f{number}"),
            KeyCode::Insert => "insert".to_owned(),
            code => KEY_NAMES
                .iter()
                .find(|(_, candidate)| *candidate == code)
                .map_or_else(
                    || format!("{code:?}").to_lowercase(),
                    |(name, _)| (*name).to_owned(),
                ),
        };
        let mut text = String::new();
        if let Some(prefix) = self.prefix {
            text.push(prefix);
            text.push(' ');
        }
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            text.push_str("ctrl+");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            text.push_str("alt+");
        }
        text.push_str(&name);
        text
    }
}

/// Every listed command as Markdown tables, one per group, for the
/// README: its keys, what it does, and where it works. A test keeps the
/// README's copy up to date.
pub fn markdown_reference() -> String {
    let mut text = String::new();
    for group in Group::ALL {
        let rows: Vec<&Command> = COMMANDS
            .iter()
            .filter(|command| {
                command.group == group && command.listed && !command.is_paired_into_another()
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        text.push_str(&format!(
            "#### {}\n\n| Keys | | Where |\n|---|---|---|\n",
            group.title()
        ));
        for command in rows {
            let contexts: Vec<&str> = Context::ALL
                .iter()
                .filter(|context| command.contexts.contains(**context))
                .filter(|context| **context != Context::TooSmall)
                .map(|context| context.title())
                .collect();
            let place = if contexts.len() > 8 {
                "anywhere".to_owned()
            } else {
                contexts.join(", ")
            };
            let label = command.help_label();
            // The dialog a command belongs to is already in its label.
            let label = label.split_once(": ").map_or(label, |(_, rest)| rest);
            text.push_str(&format!(
                "| `{}` | {} | {} |\n",
                command.keys_label().replace('|', "\\|"),
                label,
                place
            ));
        }
        text.push('\n');
    }
    text.trim_end().to_owned() + "\n"
}

/// How a command's first key is written in messages, as "u" in "u to
/// undo", or `None` if the config file left it without keys.
pub fn key_label(id: CommandId) -> Option<String> {
    command(id).keys.first().map(|key| key.label())
}

/// `message`, followed by how to undo it, as in "Task deleted · u to
/// undo".
pub fn with_undo_hint(message: &str) -> String {
    match key_label(CommandId::Undo) {
        Some(key) => format!("{message} · {key} to undo"),
        None => message.to_owned(),
    }
}

pub fn command(id: CommandId) -> &'static Command {
    commands()
        .iter()
        .find(|command| command.id == id)
        .expect("every CommandId has a row in COMMANDS")
}

/// The command a key runs in a context, if any.
pub fn lookup(context: Context, event: &KeyEvent) -> Option<CommandId> {
    lookup_after(context, None, event)
}

/// The command a key runs in a context after `prefix` was pressed (or
/// with no prefix), if any.
pub fn lookup_after(context: Context, prefix: Option<char>, event: &KeyEvent) -> Option<CommandId> {
    commands()
        .iter()
        .filter(|command| command.contexts.contains(context))
        .find(|command| {
            command
                .keys
                .iter()
                .any(|key| key.prefix == prefix && key.matches(event))
        })
        .map(|command| command.id)
}

/// The commands available in a context, for help and hints.
pub fn available(context: Context) -> impl Iterator<Item = &'static Command> {
    commands()
        .iter()
        .filter(move |command| command.contexts.contains(context))
}

/// The footer hints for a context, most important first, as
/// `(keys, label)` pairs. `enabled` can hide commands that do nothing
/// right now, such as "clear" without a search.
pub fn hints(context: Context, enabled: impl Fn(CommandId) -> bool) -> Vec<(String, &'static str)> {
    hint_commands(context, enabled)
        .into_iter()
        .map(|(keys, label, _)| (keys, label))
        .collect()
}

/// As [`hints`], with the command each hint runs when clicked.
pub fn hint_commands(
    context: Context,
    enabled: impl Fn(CommandId) -> bool,
) -> Vec<(String, &'static str, CommandId)> {
    let mut commands: Vec<(&Command, Hint)> = available(context)
        .filter(|command| enabled(command.id))
        .filter_map(|command| command.hint.map(|hint| (command, hint)))
        .collect();
    commands.sort_by_key(|(_, hint)| hint.priority);
    commands
        .into_iter()
        .map(|(command, hint)| (command.hint_keys(), hint.label, command.id))
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
            assert!(
                !command.keys.is_empty() || command.shown_keys.is_some(),
                "{:?} has no keys",
                command.id
            );
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
            let expected = match context {
                Context::QuickAdd => C::CloseQuickAdd,
                Context::Editor => C::CancelEdit,
                Context::Palette => C::PaletteClose,
                Context::Prompt => C::PromptCancel,
                _ => C::ForceQuit,
            };
            assert_eq!(lookup(context, &ctrl_c), Some(expected));
        }
        // Plain "c" is not Ctrl+C, and Ctrl+H is not h.
        let ctrl_h = event(KeyCode::Char('h'), KeyModifiers::CONTROL);
        assert_eq!(lookup(Context::Board, &ctrl_h), None);
    }

    #[test]
    fn chords_only_match_after_their_prefix() {
        let g = event(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(lookup(Context::Board, &g), Some(C::GoPrefix));
        assert_eq!(
            lookup_after(Context::Board, Some('g'), &g),
            Some(C::FirstCard)
        );
        let b = event(KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(lookup_after(Context::Board, Some('g'), &b), None);
        assert_eq!(command(C::FirstCard).keys_label(), "Home/End  g g/G");
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
    fn commands_have_config_names() {
        assert_eq!(C::NewTask.name(), "new_task");
        assert_eq!(C::MoveTaskLeft.name(), "move_task_left");
        assert_eq!(CommandId::from_name("open_palette"), Some(C::OpenPalette));
        assert_eq!(CommandId::from_name("nope"), None);
    }

    #[test]
    fn every_key_in_the_table_reads_back_from_its_config_name() {
        for command in COMMANDS {
            for key in command.keys {
                let name = key.config_name();
                assert_eq!(Key::parse(&name), Ok(*key), "{name}");
            }
        }
    }

    #[test]
    fn keys_are_read_as_people_write_them() {
        let parse = |text: &str| Key::parse(text).unwrap();
        assert_eq!(parse("Ctrl+K"), ctrl('k'));
        assert_eq!(parse("shift+h"), ch('H'));
        assert_eq!(parse("Shift+Tab"), key(KeyCode::BackTab));
        assert_eq!(parse("PageDown"), key(KeyCode::PageDown));
        assert_eq!(parse("+"), ch('+'));
        assert_eq!(parse("g x"), after_g('x'));
        assert_eq!(parse("f5"), key(KeyCode::F(5)));
        assert_eq!(
            parse("alt+x"),
            Key {
                modifiers: KeyModifiers::ALT,
                ..ch('x')
            }
        );
        for bad in ["", "hyper+x", "f13", "nonsense", "gg x"] {
            assert!(Key::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn remapping_checks_for_clashes() {
        let table = remap(&[(C::NewTask, vec![ch('+')])]).unwrap();
        let new_task = table
            .iter()
            .find(|command| command.id == C::NewTask)
            .unwrap();
        assert_eq!(new_task.keys, [ch('+')]);
        // n is now free, so another command can take it.
        assert!(remap(&[(C::NewTask, vec![ch('+')]), (C::Search, vec![ch('n')])]).is_ok());
        let clash = remap(&[(C::Search, vec![ch('n')])]).unwrap_err();
        assert!(
            clash.contains("new_task") && clash.contains("search"),
            "{clash}"
        );
        assert!(remap(&[(C::JumpToLane, vec![ch('x')])]).is_err());
        assert!(
            remap(&[(
                C::Search,
                vec![Key {
                    prefix: Some('z'),
                    ..ch('x')
                }]
            )])
            .is_err()
        );
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
