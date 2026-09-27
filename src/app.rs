use anyhow::{Context, Result};
use clap::Parser;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::animation::{AnimationEngine, AnimationKind, AnimationSettings};
use crate::clock::Clock;
use crate::domain::{Board, BoardError, Task};
use crate::layout::Breakpoint;
use crate::storage::JsonStore;
use crate::theme::{Theme, shared_theme};
use crate::tui::event::accepts_key;
use crate::ui;

/// How often the idle event loop wakes to refresh relative times.
const IDLE_REFRESH: Duration = Duration::from_secs(1);

#[derive(Debug, Parser)]
#[command(name = "tui-kanban", about = "A beautiful terminal Kanban board")]
pub struct Cli {
    #[arg(long, default_value = ".tui-kanban.json")]
    pub board: PathBuf,
    #[arg(long)]
    pub no_animation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorField {
    Title,
    Description,
}

#[derive(Clone, Debug)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let mut input = Self {
            value: value.into(),
            cursor: 0,
        };
        input.cursor = input.value.len();
        input
    }

    pub fn insert(&mut self, character: char) {
        self.clamp_cursor();
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub fn backspace(&mut self) {
        self.clamp_cursor();
        if self.cursor == 0 {
            return;
        }
        let previous = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.value.remove(previous);
        self.cursor = previous;
    }

    pub fn delete(&mut self) {
        self.clamp_cursor();
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        self.clamp_cursor();
        self.cursor = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
    }

    pub fn move_right(&mut self) {
        self.clamp_cursor();
        if self.cursor < self.value.len() {
            self.cursor += self.value[self.cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(0);
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.value.len());
        while self.cursor > 0 && !self.value.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }
}

#[derive(Clone, Debug)]
pub struct EditorState {
    pub task_id: Option<uuid::Uuid>,
    pub title: TextInput,
    pub description: TextInput,
    pub field: EditorField,
    pub error: Option<String>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            task_id: None,
            title: TextInput::new(String::new()),
            description: TextInput::new(String::new()),
            field: EditorField::Title,
            error: None,
        }
    }

    pub fn from_task(task: &Task) -> Self {
        Self {
            task_id: Some(task.id),
            title: TextInput::new(task.title.clone()),
            description: TextInput::new(task.description.clone()),
            field: EditorField::Title,
            error: None,
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub enum Mode {
    Dashboard,
    Editor(EditorState),
    Detail(uuid::Uuid),
    Help,
    ConfirmDelete(uuid::Uuid),
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
    pub expires_at: Instant,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SaveState {
    #[default]
    Clean,
    Dirty,
    Error(String),
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleTask {
    pub column: usize,
    pub task: usize,
    pub id: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct App {
    pub board: Board,
    pub store: JsonStore,
    /// The selected task. Its position is always looked up from the board,
    /// so it can't go stale when tasks move, are deleted or are filtered.
    pub selected: Option<uuid::Uuid>,
    /// The column that has focus in the Board view and the rail.
    pub active_column: usize,
    /// The terminal size in cells, updated on resize.
    pub viewport: (u16, u16),
    pub view_mode: ViewMode,
    pub focus: FocusRegion,
    pub all_tasks_scroll: u16,
    pub detail_scroll: u16,
    pub help_scroll: u16,
    pub mode: Mode,
    pub search_query: String,
    pub search_input: TextInput,
    pub search_active: bool,
    pub toast: Option<Toast>,
    pub save_state: SaveState,
    pub should_quit: bool,
    pub animations: AnimationEngine,
    pub theme: Theme,
    /// The time of the event being handled, set by `handle_key` and `tick`.
    clock: Clock,
}

impl App {
    pub fn new(store: JsonStore, animations_enabled: bool, clock: Clock) -> Result<Self> {
        let board = store
            .load_or_default()
            .with_context(|| format!("could not load {}", store.path().display()))?;
        let theme = shared_theme();
        let mut app = Self {
            board,
            store,
            selected: None,
            active_column: 0,
            viewport: (0, 0),
            view_mode: ViewMode::Board,
            focus: FocusRegion::Cards,
            all_tasks_scroll: 0,
            detail_scroll: 0,
            help_scroll: 0,
            mode: Mode::Dashboard,
            search_query: String::new(),
            search_input: TextInput::new(String::new()),
            search_active: false,
            toast: None,
            save_state: SaveState::Clean,
            should_quit: false,
            animations: AnimationEngine::new(AnimationSettings {
                enabled: animations_enabled,
            }),
            theme,
            clock,
        };
        app.reconcile_selection();
        Ok(app)
    }

    pub fn handle_key(&mut self, key: KeyEvent, clock: Clock) {
        if !accepts_key(&key) {
            return;
        }
        self.clock = clock;
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.should_quit = true;
            return;
        }

        match self.mode.clone() {
            Mode::Dashboard => self.handle_dashboard_key(key),
            Mode::Editor(mut editor) => {
                self.handle_editor_key(key, &mut editor);
                if matches!(self.mode, Mode::Editor(_)) {
                    self.mode = Mode::Editor(editor);
                }
            }
            Mode::Detail(_) => self.handle_detail_key(key),
            Mode::Help => self.handle_help_key(key),
            Mode::ConfirmDelete(_) => self.handle_confirm_key(key),
        }
    }

    /// Records the new terminal size. Focus leaves the rail if it is no
    /// longer shown.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.viewport = (width, height);
        if !self.breakpoint().shows_rail() {
            self.focus = FocusRegion::Cards;
        }
    }

    pub fn breakpoint(&self) -> Breakpoint {
        Breakpoint::from_width(self.viewport.0)
    }

    pub fn tick(&mut self, clock: Clock) {
        self.clock = clock;
        let now = clock.instant;
        self.animations.tick(now);
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| toast.expires_at <= now)
        {
            self.toast = None;
        }
    }

    /// How long the event loop may wait for input before it has to redraw:
    /// the next animation frame, the moment a toast expires, or an idle
    /// refresh so relative times such as "5m" stay current.
    pub fn next_timeout(&self, now: Instant) -> Duration {
        let mut timeout = IDLE_REFRESH;
        if let Some(frame) = self.animations.next_frame_timeout(now) {
            timeout = timeout.min(frame);
        }
        if let Some(toast) = &self.toast {
            timeout = timeout.min(toast.expires_at.saturating_duration_since(now));
        }
        timeout
    }

    pub fn flush(&mut self) -> Result<(), String> {
        if self.save_state == SaveState::Dirty {
            self.store
                .save(&self.board)
                .map_err(|error| error.to_string())?;
            self.save_state = SaveState::Clean;
        }
        Ok(())
    }

    pub fn selected_task_id(&self) -> Option<uuid::Uuid> {
        self.selected.filter(|id| self.board.task(*id).is_some())
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
                    .filter(|(_, task)| task.matches(&self.search_query))
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
                    .filter(|(_, task)| task.matches(&self.search_query))
                    .map(move |(task_index, task)| VisibleTask {
                        column: column_index,
                        task: task_index,
                        id: task.id,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// The selected task's position among the visible tasks of `column`,
    /// if it is in that column.
    pub fn selected_visual_index(&self, column: usize) -> Option<usize> {
        let (selected_column, index) = self.board.task_location(self.selected?)?;
        if selected_column != column {
            return None;
        }
        self.visible_task_indices(column)
            .iter()
            .position(|visible| *visible == index)
    }

    fn select_task(&mut self, id: uuid::Uuid) {
        if let Some((column, _)) = self.board.task_location(id) {
            self.selected = Some(id);
            self.active_column = column;
        }
    }

    fn first_visible_in(&self, column: usize) -> Option<uuid::Uuid> {
        let index = *self.visible_task_indices(column).first()?;
        Some(self.board.columns[column].tasks[index].id)
    }

    pub fn status_text(&self) -> String {
        match &self.save_state {
            SaveState::Clean => "saved".to_owned(),
            SaveState::Dirty => "saving".to_owned(),
            SaveState::Error(message) => format!("save error: {message}"),
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) {
        if self.search_active {
            self.handle_search_key(key);
            return;
        }
        if key.code == KeyCode::Esc && !self.search_query.is_empty() {
            self.clear_search();
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.mode = Mode::Editor(EditorState::new());
                self.animations.start(
                    AnimationKind::Modal,
                    Duration::from_millis(180),
                    self.clock.instant,
                );
            }
            KeyCode::Char('e') | KeyCode::Char('E') => self.open_editor(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_confirmation(),
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
                self.help_scroll = 0;
                self.animations.start(
                    AnimationKind::Modal,
                    Duration::from_millis(180),
                    self.clock.instant,
                );
            }
            KeyCode::Char('/') => {
                self.search_active = true;
                self.search_input = TextInput::new(self.search_query.clone());
                self.focus = FocusRegion::Cards;
                self.animations.start(
                    AnimationKind::Selection,
                    Duration::from_millis(120),
                    self.clock.instant,
                );
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusRegion::Rail => FocusRegion::Cards,
                    // The rail can only take focus while it is on screen.
                    FocusRegion::Cards if self.breakpoint().shows_rail() => FocusRegion::Rail,
                    FocusRegion::Cards => FocusRegion::Cards,
                };
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                self.view_mode = match self.view_mode {
                    ViewMode::Board => ViewMode::AllTasks,
                    ViewMode::AllTasks => ViewMode::Board,
                };
                self.all_tasks_scroll = 0;
                self.focus = FocusRegion::Cards;
                self.reconcile_selection();
                self.animations.start(
                    AnimationKind::Selection,
                    Duration::from_millis(140),
                    self.clock.instant,
                );
            }
            KeyCode::Char('H') => self.move_selected_task(-1),
            KeyCode::Char('L') => self.move_selected_task(1),
            _ => self.handle_dashboard_navigation_key(key),
        }
    }

    fn handle_dashboard_navigation_key(&mut self, key: KeyEvent) {
        match self.focus {
            FocusRegion::Rail => match key.code {
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Up | KeyCode::Char('k') => {
                    self.move_column(-1)
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Down | KeyCode::Char('j') => {
                    self.move_column(1)
                }
                KeyCode::Home => self.select_column(0),
                KeyCode::End => self.select_column(self.board.columns.len().saturating_sub(1)),
                KeyCode::Enter => self.focus = FocusRegion::Cards,
                _ => {}
            },
            FocusRegion::Cards => match key.code {
                KeyCode::Left | KeyCode::Char('h') => self.move_column(-1),
                KeyCode::Right | KeyCode::Char('l') => self.move_column(1),
                KeyCode::Up | KeyCode::Char('k') => self.move_task_vertical(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_task_vertical(1),
                KeyCode::PageUp => self.move_task_vertical(-5),
                KeyCode::PageDown => self.move_task_vertical(5),
                KeyCode::Home => self.select_first_visible_task(),
                KeyCode::End => self.select_last_visible_task(),
                KeyCode::Enter => self.open_detail(),
                _ => {}
            },
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.clear_search(),
            KeyCode::Enter => self.search_active = false,
            KeyCode::Backspace => {
                self.search_input.backspace();
                self.search_query = self.search_input.value.clone();
                self.reconcile_selection();
            }
            KeyCode::Delete => {
                self.search_input.delete();
                self.search_query = self.search_input.value.clone();
                self.reconcile_selection();
            }
            KeyCode::Left => self.search_input.move_left(),
            KeyCode::Right => self.search_input.move_right(),
            KeyCode::Home => self.search_input.home(),
            KeyCode::End => self.search_input.end(),
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_input.insert(character);
                self.search_query = self.search_input.value.clone();
                self.reconcile_selection();
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent, editor: &mut EditorState) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Dashboard;
                self.animations.start(
                    AnimationKind::Modal,
                    Duration::from_millis(140),
                    self.clock.instant,
                );
            }
            KeyCode::Tab | KeyCode::BackTab => {
                editor.field = match editor.field {
                    EditorField::Title => EditorField::Description,
                    EditorField::Description => EditorField::Title,
                };
            }
            KeyCode::Enter => self.submit_editor(editor),
            KeyCode::Backspace => match editor.field {
                EditorField::Title => editor.title.backspace(),
                EditorField::Description => editor.description.backspace(),
            },
            KeyCode::Delete => match editor.field {
                EditorField::Title => editor.title.delete(),
                EditorField::Description => editor.description.delete(),
            },
            KeyCode::Left => match editor.field {
                EditorField::Title => editor.title.move_left(),
                EditorField::Description => editor.description.move_left(),
            },
            KeyCode::Right => match editor.field {
                EditorField::Title => editor.title.move_right(),
                EditorField::Description => editor.description.move_right(),
            },
            KeyCode::Home => match editor.field {
                EditorField::Title => editor.title.home(),
                EditorField::Description => editor.description.home(),
            },
            KeyCode::End => match editor.field {
                EditorField::Title => editor.title.end(),
                EditorField::Description => editor.description.end(),
            },
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                match editor.field {
                    EditorField::Title => editor.title.insert(character),
                    EditorField::Description => editor.description.insert(character),
                }
                editor.error = None;
            }
            _ => {}
        }
    }

    fn handle_detail_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Dashboard;
                self.detail_scroll = 0;
            }
            KeyCode::Char('e') | KeyCode::Char('E') => self.open_editor(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_confirmation(),
            KeyCode::Char('H') => self.move_selected_task(-1),
            KeyCode::Char('L') => self.move_selected_task(1),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(3),
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(3),
            _ => {}
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        let max_scroll = ui::help_line_count().saturating_sub(1) as u16;
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = self.help_scroll.saturating_add(1).min(max_scroll);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            _ => self.mode = Mode::Dashboard,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Mode::ConfirmDelete(id) = self.mode.clone() {
                    self.delete_task(id);
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => self.mode = Mode::Dashboard,
            _ => {}
        }
    }

    fn open_editor(&mut self) {
        if let Some(id) = self.selected_task_id()
            && let Some(task) = self.board.task(id)
        {
            self.mode = Mode::Editor(EditorState::from_task(task));
            self.animations.start(
                AnimationKind::Modal,
                Duration::from_millis(180),
                self.clock.instant,
            );
        }
    }

    fn open_detail(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.mode = Mode::Detail(id);
            self.detail_scroll = 0;
            self.animations.start(
                AnimationKind::Drawer,
                Duration::from_millis(220),
                self.clock.instant,
            );
        }
    }

    fn open_delete_confirmation(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.mode = Mode::ConfirmDelete(id);
            self.animations.start(
                AnimationKind::Modal,
                Duration::from_millis(160),
                self.clock.instant,
            );
        }
    }

    fn submit_editor(&mut self, editor: &mut EditorState) {
        let title = editor.title.value.trim().to_owned();
        if title.is_empty() {
            editor.error = Some("Title cannot be empty".to_owned());
            return;
        }
        let description = editor.description.value.clone();
        let now = self.clock.wall_millis;
        let result = if let Some(id) = editor.task_id {
            self.mutate(|board| board.update_task(id, title, description, now).map(|_| id))
        } else {
            let column = self.active_column;
            self.mutate(move |board| board.add_task(column, title, description, now))
        };

        match result {
            Ok(id) => {
                self.mode = Mode::Dashboard;
                self.select_task(id);
                self.reconcile_selection();
                self.show_toast("Task saved", ToastKind::Success, Duration::from_secs(3));
                self.animations.start(
                    AnimationKind::CardMove,
                    Duration::from_millis(220),
                    self.clock.instant,
                );
            }
            Err(message) => editor.error = Some(message),
        }
    }

    fn delete_task(&mut self, id: uuid::Uuid) {
        // Select the task that takes the deleted one's place, so the
        // selection doesn't jump back to the top of the column.
        let tasks = self.navigable_tasks();
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
        if let Err(message) = self.mutate(|board| board.remove_task(id).map(|_| ())) {
            self.show_toast(message, ToastKind::Error, Duration::from_secs(4));
            return;
        }
        self.mode = Mode::Dashboard;
        self.selected = neighbour;
        self.reconcile_selection();
        self.show_toast("Task deleted", ToastKind::Info, Duration::from_secs(3));
        self.animations.start(
            AnimationKind::CardMove,
            Duration::from_millis(220),
            self.clock.instant,
        );
    }

    fn move_selected_task(&mut self, direction: i32) {
        let Some(id) = self.selected_task_id() else {
            return;
        };
        let Some((column, _)) = self.board.task_location(id) else {
            return;
        };
        let Some(target) = column.checked_add_signed(direction as isize) else {
            return;
        };
        if target >= self.board.columns.len() {
            return;
        }
        let now = self.clock.wall_millis;
        match self.mutate(|board| board.move_task(id, target, None, now)) {
            Ok(outcome) => {
                self.select_task(outcome.task_id);
                self.reconcile_selection();
                self.animations.start(
                    AnimationKind::CardMove,
                    Duration::from_millis(240),
                    self.clock.instant,
                );
            }
            Err(message) => self.show_toast(message, ToastKind::Error, Duration::from_secs(4)),
        }
    }

    fn mutate<T, F>(&mut self, change: F) -> Result<T, String>
    where
        F: FnOnce(&mut Board) -> Result<T, BoardError>,
    {
        let previous = self.board.clone();
        let value = match change(&mut self.board) {
            Ok(value) => value,
            Err(error) => {
                self.board = previous;
                return Err(error.to_string());
            }
        };
        match self.store.save(&self.board) {
            Ok(()) => {
                self.save_state = SaveState::Clean;
                Ok(value)
            }
            Err(error) => {
                self.board = previous;
                self.save_state = SaveState::Error(error.to_string());
                Err(error.to_string())
            }
        }
    }

    fn move_column(&mut self, direction: i32) {
        if self.board.columns.is_empty() {
            return;
        }
        let last = self.board.columns.len() - 1;
        let mut next = self.active_column;
        loop {
            next = match next.checked_add_signed(direction as isize) {
                Some(next) if next <= last => next,
                _ => return,
            };
            // All tasks shows only columns with matches, so skip the others.
            if self.view_mode == ViewMode::Board || self.first_visible_in(next).is_some() {
                break;
            }
        }
        self.select_column(next);
    }

    fn select_column(&mut self, column: usize) {
        if self.board.columns.is_empty() {
            return;
        }
        self.active_column = column.min(self.board.columns.len() - 1);
        self.selected = self.first_visible_in(self.active_column);
        self.reconcile_selection();
        self.animations.start(
            AnimationKind::Selection,
            Duration::from_millis(140),
            self.clock.instant,
        );
    }

    /// The tasks Up and Down move through: every visible task in All tasks,
    /// or the visible tasks of the active column in the Board view.
    fn navigable_tasks(&self) -> Vec<uuid::Uuid> {
        match self.view_mode {
            ViewMode::AllTasks => self.visible_tasks().iter().map(|task| task.id).collect(),
            ViewMode::Board => self
                .visible_task_indices(self.active_column)
                .into_iter()
                .map(|index| self.board.columns[self.active_column].tasks[index].id)
                .collect(),
        }
    }

    fn move_task_vertical(&mut self, direction: i32) {
        let tasks = self.navigable_tasks();
        if tasks.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected_task_id()
            .and_then(|id| tasks.iter().position(|task| *task == id));
        let next = match current {
            Some(current) => current
                .saturating_add_signed(direction as isize)
                .min(tasks.len() - 1),
            // Nothing selected yet: Down starts at the top, Up at the bottom.
            None if direction > 0 => 0,
            None => tasks.len() - 1,
        };
        self.select_task(tasks[next]);
        self.animations.start(
            AnimationKind::Selection,
            Duration::from_millis(100),
            self.clock.instant,
        );
    }

    fn select_first_visible_task(&mut self) {
        if let Some(id) = self.navigable_tasks().first() {
            self.select_task(*id);
        }
    }

    fn select_last_visible_task(&mut self) {
        if let Some(id) = self.navigable_tasks().last() {
            self.select_task(*id);
        }
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_input = TextInput::new(String::new());
        self.search_active = false;
        self.reconcile_selection();
    }

    /// Makes the selection valid after any change to the board, the search
    /// or the view: a selected task that still exists and still matches
    /// stays selected (and its column becomes active). Otherwise the first
    /// visible task in the active column is selected, or, in All tasks,
    /// the first visible task anywhere.
    fn reconcile_selection(&mut self) {
        if self.board.columns.is_empty() {
            self.active_column = 0;
            self.selected = None;
            return;
        }
        self.active_column = self.active_column.min(self.board.columns.len() - 1);
        if let Some(id) = self.selected
            && let Some(task) = self.board.task(id)
            && task.matches(&self.search_query)
        {
            self.select_task(id);
            return;
        }
        self.selected = self.first_visible_in(self.active_column);
        if self.selected.is_none()
            && self.view_mode == ViewMode::AllTasks
            && let Some(first) = self.visible_tasks().first()
        {
            self.select_task(first.id);
        }
    }

    fn show_toast(&mut self, message: impl Into<String>, kind: ToastKind, duration: Duration) {
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            expires_at: self.clock.instant + duration,
        });
        self.animations.start(
            AnimationKind::Toast,
            Duration::from_millis(220),
            self.clock.instant,
        );
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = JsonStore::new(cli.board);
    let mut app = App::new(store, !cli.no_animation, Clock::now())?;
    let mut terminal = ratatui::try_init().context("could not initialize terminal")?;
    if let Ok(size) = terminal.size() {
        app.resize(size.width, size.height);
    }
    let result = run_loop(&mut terminal, &mut app);
    let restore_result = ratatui::try_restore().context("could not restore terminal");
    result.and(restore_result)
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut needs_redraw = true;
    loop {
        // Tick on every iteration, not only when polling times out, so
        // toasts expire and animations finish while keys keep arriving.
        let clock = Clock::now();
        app.tick(clock);
        if needs_redraw {
            terminal.draw(|frame| ui::render(frame, app, clock))?;
            needs_redraw = false;
        }
        if app.should_quit {
            return app.flush().map_err(anyhow::Error::msg);
        }

        let timeout = app.next_timeout(clock.instant);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_key(key, Clock::now());
                    needs_redraw = true;
                }
                Event::Resize(width, height) => {
                    app.resize(width, height);
                    needs_redraw = true;
                }
                _ => {}
            }
        } else {
            // An animation frame is due, a toast expired, or it is time
            // for the idle refresh.
            needs_redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::TempDir;

    fn test_app() -> (TempDir, App) {
        test_app_at(Clock::fixed(0), false)
    }

    fn test_app_at(clock: Clock, animations: bool) -> (TempDir, App) {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let mut app = App::new(store, animations, clock).unwrap();
        app.resize(100, 30);
        (directory, app)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn idle_loop_waits_for_the_refresh_interval() {
        let (_directory, app) = test_app();
        assert_eq!(app.next_timeout(Instant::now()), IDLE_REFRESH);
    }

    #[test]
    fn animations_run_at_frame_rate() {
        let clock = Clock::fixed(0);
        let (_directory, mut app) = test_app_at(clock, true);
        app.handle_key(key(KeyCode::Char('?')), clock);
        assert_eq!(
            app.next_timeout(clock.instant),
            crate::animation::FRAME_INTERVAL
        );
    }

    #[test]
    fn edit_and_new_task_modals_both_animate() {
        let clock = Clock::fixed(0);
        let (_directory, mut app) = test_app_at(clock, true);
        app.board.add_task(0, "Edit me", "", 0).unwrap();
        app.reconcile_selection();
        for opener in ['n', 'e'] {
            app.mode = Mode::Dashboard;
            app.animations.finish(AnimationKind::Modal);
            app.handle_key(key(KeyCode::Char(opener)), clock);
            assert!(matches!(app.mode, Mode::Editor(_)), "{opener}");
            assert!(
                app.animations
                    .progress(AnimationKind::Modal, clock.instant)
                    .is_some(),
                "{opener} should start the modal animation the editor reads"
            );
        }
    }

    #[test]
    fn toasts_wake_the_loop_when_they_expire() {
        let clock = Clock::fixed(0);
        let (_directory, mut app) = test_app_at(clock, false);
        app.show_toast("hello", ToastKind::Info, Duration::from_millis(400));
        assert_eq!(
            app.next_timeout(clock.instant + Duration::from_millis(100)),
            Duration::from_millis(300)
        );
        app.tick(clock.advance(Duration::from_millis(400)));
        assert!(app.toast.is_none());
    }

    #[test]
    fn text_input_handles_utf8_boundaries() {
        let mut input = TextInput::new("é");
        input.backspace();
        assert_eq!(input.value, "");
        input.insert('🦀');
        input.move_left();
        input.insert('x');
        assert_eq!(input.value, "x🦀");
    }

    #[test]
    fn editor_creates_and_persists_a_task() {
        let (_directory, mut app) = test_app();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('n')), now);
        for character in "Ship it".chars() {
            app.handle_key(key(KeyCode::Char(character)), now);
        }
        app.handle_key(key(KeyCode::Tab), now);
        for character in "Make it beautiful".chars() {
            app.handle_key(key(KeyCode::Char(character)), now);
        }
        app.handle_key(key(KeyCode::Enter), now);
        assert_eq!(app.board.task_count(), 1);
        assert_eq!(app.board.columns[0].tasks[0].title, "Ship it");
        assert!(app.store.path().exists());
    }

    #[test]
    fn search_escape_clears_the_query() {
        let (_directory, mut app) = test_app();
        app.board
            .add_task(0, "Design cards", "Color and layout", 0)
            .unwrap();
        app.board.add_task(0, "Write tests", "Storage", 0).unwrap();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('/')), now);
        for character in "design".chars() {
            app.handle_key(key(KeyCode::Char(character)), now);
        }
        app.handle_key(key(KeyCode::Esc), now);
        assert_eq!(app.search_query, "");
        assert_eq!(app.visible_task_indices(0), vec![0, 1]);
        assert!(!app.search_active);
    }

    #[test]
    fn moving_a_task_changes_column_and_selection() {
        let (_directory, mut app) = test_app();
        let id = app.board.add_task(0, "Move me", "", 0).unwrap();
        app.reconcile_selection();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('L')), now);
        assert_eq!(app.board.task_location(id), Some((1, 0)));
        assert_eq!(app.active_column, 1);
        assert_eq!(app.selected_task_id(), Some(id));
    }

    #[test]
    fn delete_requires_confirmation() {
        let (_directory, mut app) = test_app();
        app.board.add_task(0, "Delete me", "", 0).unwrap();
        app.reconcile_selection();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('d')), now);
        assert!(matches!(app.mode, Mode::ConfirmDelete(_)));
        app.handle_key(key(KeyCode::Esc), now);
        assert_eq!(app.board.task_count(), 1);
        app.handle_key(key(KeyCode::Char('d')), now);
        app.handle_key(key(KeyCode::Char('y')), now);
        assert_eq!(app.board.task_count(), 0);
    }

    #[test]
    fn toggles_all_tasks_and_rail_focus() {
        let (_directory, mut app) = test_app();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('v')), now);
        assert_eq!(app.view_mode, ViewMode::AllTasks);
        assert_eq!(app.focus, FocusRegion::Cards);
        app.handle_key(key(KeyCode::Tab), now);
        assert_eq!(app.focus, FocusRegion::Rail);
        app.handle_key(key(KeyCode::Enter), now);
        assert_eq!(app.focus, FocusRegion::Cards);
    }

    fn type_text(app: &mut App, text: &str) {
        for character in text.chars() {
            app.handle_key(key(KeyCode::Char(character)), Clock::fixed(0));
        }
    }

    #[test]
    fn all_tasks_search_selects_the_first_match_anywhere() {
        // #4: a search that filters out the selected column used to clear
        // the selection, and Down then skipped the first match.
        let (_directory, mut app) = test_app();
        app.board.add_task(0, "beta", "", 0).unwrap();
        let first = app.board.add_task(1, "alpha one", "", 0).unwrap();
        let second = app.board.add_task(1, "alpha two", "", 0).unwrap();
        app.reconcile_selection();
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('v')), now);
        app.handle_key(key(KeyCode::Char('/')), now);
        type_text(&mut app, "alpha");
        app.handle_key(key(KeyCode::Enter), now);
        assert_eq!(app.selected_task_id(), Some(first));
        assert_eq!(app.active_column, 1);
        app.handle_key(key(KeyCode::Down), now);
        assert_eq!(app.selected_task_id(), Some(second));
    }

    #[test]
    fn down_with_nothing_selected_starts_at_the_first_task() {
        let (_directory, mut app) = test_app();
        let first = app.board.add_task(0, "First", "", 0).unwrap();
        app.board.add_task(0, "Second", "", 0).unwrap();
        let last = app.board.add_task(0, "Last", "", 0).unwrap();
        app.selected = None;
        app.handle_key(key(KeyCode::Down), Clock::fixed(0));
        assert_eq!(app.selected_task_id(), Some(first));
        app.selected = None;
        app.handle_key(key(KeyCode::Up), Clock::fixed(0));
        assert_eq!(app.selected_task_id(), Some(last));
    }

    #[test]
    fn selection_follows_the_task_not_its_position() {
        let (_directory, mut app) = test_app();
        app.board.add_task(0, "First", "", 0).unwrap();
        let second = app.board.add_task(0, "Second", "", 0).unwrap();
        app.select_task(second);
        // Moving another task out from above it doesn't change the selection.
        let first = app.board.columns[0].tasks[0].id;
        app.board.move_task(first, 2, None, 0).unwrap();
        app.reconcile_selection();
        assert_eq!(app.selected_task_id(), Some(second));
    }

    #[test]
    fn deleting_selects_the_next_task() {
        let (_directory, mut app) = test_app();
        app.board.add_task(0, "First", "", 0).unwrap();
        let doomed = app.board.add_task(0, "Second", "", 0).unwrap();
        let third = app.board.add_task(0, "Third", "", 0).unwrap();
        app.select_task(doomed);
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Char('d')), now);
        app.handle_key(key(KeyCode::Char('y')), now);
        assert_eq!(app.selected_task_id(), Some(third));
    }

    #[test]
    fn tab_does_not_focus_a_hidden_rail() {
        // #5: below the rail's breakpoint, Tab used to move focus to it.
        let (_directory, mut app) = test_app();
        app.resize(60, 20);
        app.handle_key(key(KeyCode::Tab), Clock::fixed(0));
        assert_eq!(app.focus, FocusRegion::Cards);
        app.resize(100, 30);
        app.handle_key(key(KeyCode::Tab), Clock::fixed(0));
        assert_eq!(app.focus, FocusRegion::Rail);
        // Shrinking the terminal hides the rail, so focus leaves it.
        app.resize(60, 20);
        assert_eq!(app.focus, FocusRegion::Cards);
    }

    #[test]
    fn all_tasks_navigation_keeps_task_identity() {
        let (_directory, mut app) = test_app();
        let first = app.board.add_task(0, "First", "", 0).unwrap();
        let second = app.board.add_task(1, "Second", "", 0).unwrap();
        app.view_mode = ViewMode::AllTasks;
        app.select_task(first);
        let now = Clock::fixed(0);
        app.handle_key(key(KeyCode::Down), now);
        assert_eq!(app.selected_task_id(), Some(second));
        assert_eq!(app.active_column, 1);
    }
}
