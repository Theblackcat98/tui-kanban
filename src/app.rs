use anyhow::{Context, Result};
use clap::Parser;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::animation::{AnimationEngine, AnimationKind, AnimationSettings};
use crate::domain::{Board, BoardError, Task};
use crate::storage::JsonStore;
use crate::theme::{Theme, shared_theme};
use crate::tui::event::accepts_key;
use crate::ui;

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

#[derive(Clone, Debug)]
pub struct App {
    pub board: Board,
    pub store: JsonStore,
    pub selected_column: usize,
    pub selected_task: usize,
    pub mode: Mode,
    pub search_query: String,
    pub search_input: TextInput,
    pub search_active: bool,
    pub toast: Option<Toast>,
    pub save_state: SaveState,
    pub should_quit: bool,
    pub animations: AnimationEngine,
    pub theme: Theme,
}

impl App {
    pub fn new(store: JsonStore, animations_enabled: bool) -> Result<Self> {
        let board = store
            .load_or_default()
            .with_context(|| format!("could not load {}", store.path().display()))?;
        let theme = shared_theme();
        let mut app = Self {
            board,
            store,
            selected_column: 0,
            selected_task: 0,
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
        };
        app.normalize_selection();
        Ok(app)
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        if !accepts_key(&key) {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.should_quit = true;
            return;
        }

        match self.mode.clone() {
            Mode::Dashboard => self.handle_dashboard_key(key, now),
            Mode::Editor(mut editor) => {
                self.handle_editor_key(key, &mut editor);
                if matches!(self.mode, Mode::Editor(_)) {
                    self.mode = Mode::Editor(editor);
                }
            }
            Mode::Detail(_) => self.handle_detail_key(key),
            Mode::Help => self.handle_help_key(key),
            Mode::ConfirmDelete(_) => self.handle_confirm_key(key, now),
        }
    }

    pub fn tick(&mut self, now: Instant) {
        self.animations.tick(now);
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| toast.expires_at <= now)
        {
            self.toast = None;
        }
    }

    pub fn next_timeout(&self, now: Instant) -> Duration {
        self.animations
            .next_frame_timeout(now)
            .min(Duration::from_millis(100))
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
        let column = self.board.columns.get(self.selected_column)?;
        let visible = self.visible_task_indices(self.selected_column);
        let index = *visible.get(self.selected_task_visual_index(self.selected_column))?;
        column.tasks.get(index).map(|task| task.id)
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

    pub fn selected_task_visual_index(&self, column: usize) -> usize {
        let visible = self.visible_task_indices(column);
        visible
            .iter()
            .position(|index| *index == self.selected_task)
            .unwrap_or(0)
    }

    pub fn selected_column_name(&self) -> &str {
        self.board
            .columns
            .get(self.selected_column)
            .map(|column| column.name.as_str())
            .unwrap_or("No column")
    }

    pub fn status_text(&self) -> String {
        match &self.save_state {
            SaveState::Clean => "saved".to_owned(),
            SaveState::Dirty => "saving".to_owned(),
            SaveState::Error(message) => format!("save error: {message}"),
        }
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent, now: Instant) {
        if self.search_active {
            self.handle_search_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.mode = Mode::Editor(EditorState::new());
                self.animations
                    .start(AnimationKind::Modal, Duration::from_millis(180));
            }
            KeyCode::Char('e') | KeyCode::Char('E') => self.open_editor(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_confirmation(),
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
                self.animations
                    .start(AnimationKind::Modal, Duration::from_millis(180));
            }
            KeyCode::Char('/') => {
                self.search_active = true;
                self.search_input = TextInput::new(self.search_query.clone());
                self.animations
                    .start(AnimationKind::Selection, Duration::from_millis(120));
            }
            KeyCode::Char('H') => self.move_selected_task(-1, now),
            KeyCode::Char('L') => self.move_selected_task(1, now),
            KeyCode::Enter => self.open_detail(),
            KeyCode::Left | KeyCode::Char('h') => self.move_column(-1),
            KeyCode::Right | KeyCode::Char('l') => self.move_column(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_task_vertical(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_task_vertical(1),
            KeyCode::PageUp => self.move_task_vertical(-5),
            KeyCode::PageDown => self.move_task_vertical(5),
            KeyCode::Home => self.select_first_visible_task(),
            KeyCode::End => self.select_last_visible_task(),
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.clear_search(),
            KeyCode::Enter => self.search_active = false,
            KeyCode::Backspace => {
                self.search_input.backspace();
                self.search_query = self.search_input.value.clone();
                self.normalize_selection();
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search_input.insert(character);
                self.search_query = self.search_input.value.clone();
                self.normalize_selection();
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent, editor: &mut EditorState) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Dashboard;
                self.animations
                    .start(AnimationKind::Modal, Duration::from_millis(140));
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
            KeyCode::Esc => self.mode = Mode::Dashboard,
            KeyCode::Char('e') | KeyCode::Char('E') => self.open_editor(),
            KeyCode::Char('d') | KeyCode::Char('D') => self.open_delete_confirmation(),
            _ => {}
        }
    }

    fn handle_help_key(&mut self, _key: KeyEvent) {
        self.mode = Mode::Dashboard;
    }

    fn handle_confirm_key(&mut self, key: KeyEvent, _now: Instant) {
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
            self.animations
                .start(AnimationKind::Drawer, Duration::from_millis(180));
        }
    }

    fn open_detail(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.mode = Mode::Detail(id);
            self.animations
                .start(AnimationKind::Drawer, Duration::from_millis(220));
        }
    }

    fn open_delete_confirmation(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.mode = Mode::ConfirmDelete(id);
            self.animations
                .start(AnimationKind::Modal, Duration::from_millis(160));
        }
    }

    fn submit_editor(&mut self, editor: &mut EditorState) {
        let title = editor.title.value.trim().to_owned();
        if title.is_empty() {
            editor.error = Some("Title cannot be empty".to_owned());
            return;
        }
        let description = editor.description.value.clone();
        let result = if let Some(id) = editor.task_id {
            self.mutate(|board| board.update_task(id, title, description).map(|_| id))
        } else {
            let column = self.selected_column;
            self.mutate(move |board| board.add_task(column, title, description))
        };

        match result {
            Ok(id) => {
                self.mode = Mode::Dashboard;
                if let Some((column, task_index)) = self.board.task_location(id) {
                    self.selected_column = column;
                    self.selected_task = task_index;
                }
                self.normalize_selection();
                self.show_toast("Task saved", ToastKind::Success, Duration::from_secs(3));
                self.animations
                    .start(AnimationKind::CardMove, Duration::from_millis(220));
            }
            Err(message) => editor.error = Some(message),
        }
    }

    fn delete_task(&mut self, id: uuid::Uuid) {
        if let Err(message) = self.mutate(|board| board.remove_task(id).map(|_| ())) {
            self.show_toast(message, ToastKind::Error, Duration::from_secs(4));
            return;
        }
        self.mode = Mode::Dashboard;
        self.normalize_selection();
        self.show_toast("Task deleted", ToastKind::Info, Duration::from_secs(3));
        self.animations
            .start(AnimationKind::CardMove, Duration::from_millis(220));
    }

    fn move_selected_task(&mut self, direction: i32, _now: Instant) {
        let Some(id) = self.selected_task_id() else {
            return;
        };
        let Some(target) = self.selected_column.checked_add_signed(direction as isize) else {
            return;
        };
        if target >= self.board.columns.len() {
            return;
        }
        match self.mutate(|board| board.move_task(id, target, None)) {
            Ok(outcome) => {
                self.selected_column = outcome.to_column;
                self.selected_task = outcome.to_index;
                self.normalize_selection();
                self.animations
                    .start(AnimationKind::CardMove, Duration::from_millis(240));
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
        let next = self
            .selected_column
            .saturating_add_signed(direction as isize)
            .min(self.board.columns.len() - 1);
        if next != self.selected_column {
            self.selected_column = next;
            self.normalize_selection();
            self.animations
                .start(AnimationKind::Selection, Duration::from_millis(140));
        }
    }

    fn move_task_vertical(&mut self, direction: i32) {
        let visible = self.visible_task_indices(self.selected_column);
        if visible.is_empty() {
            self.selected_task = 0;
            return;
        }
        let current = self
            .selected_task_visual_index(self.selected_column)
            .saturating_add_signed(direction as isize);
        let next = current.clamp(0, visible.len() - 1);
        self.selected_task = visible[next];
        self.animations
            .start(AnimationKind::Selection, Duration::from_millis(100));
    }

    fn select_first_visible_task(&mut self) {
        if let Some(index) = self.visible_task_indices(self.selected_column).first() {
            self.selected_task = *index;
        }
    }

    fn select_last_visible_task(&mut self) {
        if let Some(index) = self.visible_task_indices(self.selected_column).last() {
            self.selected_task = *index;
        }
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_input = TextInput::new(String::new());
        self.search_active = false;
        self.normalize_selection();
    }

    fn normalize_selection(&mut self) {
        if self.board.columns.is_empty() {
            self.selected_column = 0;
            self.selected_task = 0;
            return;
        }
        self.selected_column = self.selected_column.min(self.board.columns.len() - 1);
        let visible = self.visible_task_indices(self.selected_column);
        self.selected_task = if visible.is_empty() {
            0
        } else if visible.contains(&self.selected_task) {
            self.selected_task
        } else {
            visible[0]
        };
    }

    fn show_toast(&mut self, message: impl Into<String>, kind: ToastKind, duration: Duration) {
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            expires_at: Instant::now() + duration,
        });
        self.animations
            .start(AnimationKind::Toast, Duration::from_millis(220));
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = JsonStore::new(cli.board);
    let mut app = App::new(store, !cli.no_animation)?;
    let mut terminal = ratatui::try_init().context("could not initialize terminal")?;
    let result = run_loop(&mut terminal, &mut app);
    let restore_result = ratatui::try_restore().context("could not restore terminal");
    result.and(restore_result)
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;
        if app.should_quit {
            return app.flush().map_err(anyhow::Error::msg);
        }

        let now = Instant::now();
        let timeout = app.next_timeout(now);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key, Instant::now()),
                Event::Resize(_, _) => {}
                _ => {}
            }
        } else {
            app.tick(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::TempDir;

    fn test_app() -> (TempDir, App) {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let app = App::new(store, false).unwrap();
        (directory, app)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
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
        let now = Instant::now();
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
            .add_task(0, "Design cards", "Color and layout")
            .unwrap();
        app.board.add_task(0, "Write tests", "Storage").unwrap();
        let now = Instant::now();
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
        let id = app.board.add_task(0, "Move me", "").unwrap();
        app.selected_task = 0;
        let now = Instant::now();
        app.handle_key(key(KeyCode::Char('L')), now);
        assert_eq!(app.board.task_location(id), Some((1, 0)));
        assert_eq!(app.selected_column, 1);
        assert_eq!(app.selected_task, 0);
    }

    #[test]
    fn delete_requires_confirmation() {
        let (_directory, mut app) = test_app();
        app.board.add_task(0, "Delete me", "").unwrap();
        let now = Instant::now();
        app.handle_key(key(KeyCode::Char('d')), now);
        assert!(matches!(app.mode, Mode::ConfirmDelete(_)));
        app.handle_key(key(KeyCode::Esc), now);
        assert_eq!(app.board.task_count(), 1);
        app.handle_key(key(KeyCode::Char('d')), now);
        app.handle_key(key(KeyCode::Char('y')), now);
        assert_eq!(app.board.task_count(), 0);
    }
}
