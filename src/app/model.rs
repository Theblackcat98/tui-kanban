//! Everything the app knows, in three parts: the saved [`Board`], the
//! [`Ui`] state (view, selection, focus, the screen stack) and the
//! [`Session`] (save status, toasts, quitting).
//!
//! The model is only changed by [`super::update`] and only read by
//! [`crate::ui::render`].

use std::time::Instant;
use uuid::Uuid;

use super::editor::EditorState;
use super::history::History;
use super::input::TextInput;
use crate::animation::{AnimationEngine, AnimationSettings};
use crate::command::{CommandId, Context};
use crate::domain::{Board, Task};
use crate::layout::{self, Breakpoint};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Board,
    AllTasks,
}

impl ViewMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "BOARD",
            Self::AllTasks => "ALL TASKS",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusRegion {
    Rail,
    Cards,
}

/// A screen or overlay above the board. The top of the stack gets input
/// first, and Esc pops it, returning to whatever is underneath.
#[derive(Clone, Debug)]
pub enum Screen {
    /// The detail drawer. `item` is the checklist item Space toggles.
    Detail {
        task: Uuid,
        scroll: u16,
        item: usize,
    },
    Editor(Box<EditorState>),
    ConfirmDelete {
        task: Uuid,
    },
    /// Help for `context`, the context it was opened from.
    Help {
        scroll: u16,
        context: Context,
    },
    /// The "move to…" menu: the task and the highlighted column.
    MoveTo {
        task: Uuid,
        selected: usize,
    },
    /// The one-line prompt that adds tasks to the end of `column`.
    QuickAdd {
        column: usize,
        input: TextInput,
    },
    /// "Discard changes?", above the editor.
    ConfirmDiscard,
}

/// A prefix key waiting for the key that completes it, as the `g` in
/// `g g`.
#[derive(Clone, Copy, Debug)]
pub struct Pending {
    pub prefix: char,
    pub since: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct Search {
    /// The filter applied to the board.
    pub query: String,
    /// The search box, while the user is typing in it.
    pub input: Option<TextInput>,
}

impl Search {
    pub fn is_typing(&self) -> bool {
        self.input.is_some()
    }

    pub fn is_active(&self) -> bool {
        self.is_typing() || !self.query.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    /// When the toast goes away. Errors have none: they stay until the
    /// next key press, so they can't be missed.
    pub expires_at: Option<Instant>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SaveState {
    #[default]
    Saved,
    /// The last save failed. The change is kept in memory and is saved
    /// with the next successful save.
    Failed(String),
}

/// Scroll positions, kept in the model so they survive focus changes and
/// redraws. `update` keeps them valid with `ui::sync_scroll`.
#[derive(Clone, Debug, Default)]
pub struct Scroll {
    /// Each lane's first visible card, as an index into its visible tasks.
    lanes: Vec<usize>,
    /// The first lane shown when not all of them fit.
    pub first_lane: usize,
    /// The first visible item of the All tasks list.
    pub all_tasks: usize,
}

impl Scroll {
    pub fn lane(&self, column: usize) -> usize {
        self.lanes.get(column).copied().unwrap_or(0)
    }

    pub fn set_lane(&mut self, column: usize, offset: usize) {
        if self.lanes.len() <= column {
            self.lanes.resize(column + 1, 0);
        }
        self.lanes[column] = offset;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleTask {
    pub column: usize,
    pub task: usize,
    pub id: Uuid,
}

#[derive(Clone, Debug)]
pub struct Ui {
    pub view: ViewMode,
    pub focus: FocusRegion,
    /// The selected task. Its position is always looked up from the board,
    /// so it can't go stale when tasks move, are deleted or are filtered.
    pub selected: Option<Uuid>,
    /// The column that has focus in the Board view and the rail.
    pub active_column: usize,
    pub search: Search,
    /// Screens above the board, bottom first.
    pub screens: Vec<Screen>,
    pub scroll: Scroll,
    /// The terminal size in cells.
    pub viewport: (u16, u16),
    pub theme: Theme,
    pub animations: AnimationEngine,
    /// A prefix key waiting for its second key.
    pub pending: Option<Pending>,
    /// Whether the first-run tip bar shows.
    pub tip: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub save_state: SaveState,
    pub toast: Option<Toast>,
    pub history: History,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub board: Board,
    pub ui: Ui,
    pub session: Session,
}

impl Model {
    pub fn new(board: Board, theme: Theme, animations_enabled: bool) -> Self {
        let mut model = Self {
            board,
            ui: Ui {
                view: ViewMode::Board,
                focus: FocusRegion::Cards,
                selected: None,
                active_column: 0,
                search: Search::default(),
                screens: Vec::new(),
                scroll: Scroll::default(),
                viewport: (0, 0),
                theme,
                animations: AnimationEngine::new(AnimationSettings {
                    enabled: animations_enabled,
                }),
                pending: None,
                tip: false,
            },
            session: Session::default(),
        };
        model.reconcile_selection();
        model
    }

    pub fn breakpoint(&self) -> Breakpoint {
        Breakpoint::from_width(self.ui.viewport.0)
    }

    /// Whether the terminal is too small to use; only quitting works then.
    pub fn too_small(&self) -> bool {
        let (width, height) = self.ui.viewport;
        !layout::fits(width, height)
    }

    /// Where the user is, which decides what keys mean.
    pub fn context(&self) -> Context {
        if self.too_small() {
            return Context::TooSmall;
        }
        match self.ui.screens.last() {
            Some(Screen::Help { .. }) => Context::Help,
            Some(Screen::MoveTo { .. }) => Context::MoveTo,
            Some(Screen::QuickAdd { .. }) => Context::QuickAdd,
            Some(Screen::ConfirmDiscard) => Context::Discard,
            Some(Screen::ConfirmDelete { .. }) => Context::Confirm,
            Some(Screen::Editor(_)) => Context::Editor,
            Some(Screen::Detail { .. }) => Context::Detail,
            None if self.ui.search.is_typing() => Context::Search,
            None if self.ui.focus == FocusRegion::Rail => Context::Rail,
            None => match self.ui.view {
                ViewMode::Board => Context::Board,
                ViewMode::AllTasks => Context::AllTasks,
            },
        }
    }

    /// Whether a command would do anything right now. Used to leave out
    /// footer hints that don't apply, such as "clear" with no search.
    pub fn command_enabled(&self, id: CommandId) -> bool {
        match id {
            CommandId::ClearSearch => self.ui.search.is_active(),
            CommandId::JumpToLane | CommandId::GoToLane => self.board.columns.len() > 1,
            CommandId::ToggleItem | CommandId::NextItem | CommandId::PreviousItem => {
                matches!(
                    self.ui.screens.last(),
                    Some(Screen::Detail { task, .. })
                        if self.board.task(*task).is_some_and(|task| {
                            crate::markdown::progress(&task.description).is_some()
                        })
                )
            }
            CommandId::ToggleFocus => self.breakpoint().shows_rail(),
            CommandId::Undo => self.session.history.can_undo(),
            CommandId::Redo => self.session.history.can_redo(),
            CommandId::OpenDetail
            | CommandId::EditTask
            | CommandId::DeleteTask
            | CommandId::MoveTaskLeft
            | CommandId::MoveTaskRight
            | CommandId::MoveTaskUp
            | CommandId::MoveTaskDown
            | CommandId::MoveTo
            | CommandId::DuplicateTask
            | CommandId::EditExternally => {
                self.selected_task_id().is_some()
                    || matches!(self.ui.screens.last(), Some(Screen::Detail { .. }))
            }
            _ => true,
        }
    }

    pub fn status_text(&self) -> String {
        match &self.session.save_state {
            SaveState::Saved => "saved".to_owned(),
            SaveState::Failed(message) => format!("not saved: {message}"),
        }
    }

    pub fn selected_task_id(&self) -> Option<Uuid> {
        self.ui.selected.filter(|id| self.board.task(*id).is_some())
    }

    fn matches(&self, task: &Task) -> bool {
        task.matches(&self.ui.search.query)
    }

    pub fn visible_task_indices(&self, column: usize) -> Vec<usize> {
        self.board
            .columns
            .get(column)
            .map(|column| {
                column
                    .tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, task)| self.matches(task))
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn visible_tasks(&self) -> Vec<VisibleTask> {
        self.board
            .columns
            .iter()
            .enumerate()
            .flat_map(|(column_index, column)| {
                column
                    .tasks
                    .iter()
                    .enumerate()
                    .filter(|(_, task)| self.matches(task))
                    .map(move |(task_index, task)| VisibleTask {
                        column: column_index,
                        task: task_index,
                        id: task.id,
                    })
            })
            .collect()
    }

    /// The selected task's position among the visible tasks of `column`,
    /// if it is in that column.
    pub fn selected_visual_index(&self, column: usize) -> Option<usize> {
        let (selected_column, index) = self.board.task_location(self.ui.selected?)?;
        if selected_column != column {
            return None;
        }
        self.visible_task_indices(column)
            .iter()
            .position(|visible| *visible == index)
    }

    pub(super) fn first_visible_in(&self, column: usize) -> Option<Uuid> {
        let index = *self.visible_task_indices(column).first()?;
        Some(self.board.columns[column].tasks[index].id)
    }

    /// The tasks Up and Down move through: every visible task in All tasks,
    /// or the visible tasks of the active column in the Board view.
    pub(super) fn navigable_tasks(&self) -> Vec<Uuid> {
        match self.ui.view {
            ViewMode::AllTasks => self.visible_tasks().iter().map(|task| task.id).collect(),
            ViewMode::Board => self
                .visible_task_indices(self.ui.active_column)
                .into_iter()
                .map(|index| self.board.columns[self.ui.active_column].tasks[index].id)
                .collect(),
        }
    }

    pub(super) fn select_task(&mut self, id: Uuid) {
        if let Some((column, _)) = self.board.task_location(id) {
            self.ui.selected = Some(id);
            self.ui.active_column = column;
        }
    }

    /// Makes the selection valid after any change to the board, the search
    /// or the view: a selected task that still exists and still matches
    /// stays selected (and its column becomes active). Otherwise the first
    /// visible task in the active column is selected, or, in All tasks,
    /// the first visible task anywhere.
    pub fn reconcile_selection(&mut self) {
        if self.board.columns.is_empty() {
            self.ui.active_column = 0;
            self.ui.selected = None;
            return;
        }
        self.ui.active_column = self.ui.active_column.min(self.board.columns.len() - 1);
        if let Some(id) = self.ui.selected
            && let Some(task) = self.board.task(id)
            && self.matches(task)
        {
            self.select_task(id);
            return;
        }
        self.ui.selected = self.first_visible_in(self.ui.active_column);
        if self.ui.selected.is_none()
            && self.ui.view == ViewMode::AllTasks
            && let Some(first) = self.visible_tasks().first()
        {
            self.select_task(first.id);
        }
    }
}
