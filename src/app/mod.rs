//! The app, structured as model → action → update → view:
//!
//! ```text
//! Event (key, resize, tick)
//!   → keymap(&Model, key) -> Action        (action.rs, via the command table)
//!   → update(&mut Model, Action) -> Effects (update.rs, pure)
//!   → the runtime runs the effects          (this file: quit, $EDITOR;
//!                                            persist.rs: saving, reloading)
//!   → ui::render(&Model, Clock)             (pure)
//! ```

mod action;
mod editor;
mod history;
mod input;
mod model;
mod mouse;
mod palette;
mod persist;
mod update;

pub use action::{Action, Effect, ExternalTarget, keymap};
pub use editor::{EditorField, EditorState, Placement};
pub use history::History;
pub use input::TextInput;
pub use model::{
    BoardEntry, FocusRegion, Model, PromptKind, SaveState, Screen, Scroll, Search, Session, Toast,
    ToastKind, Ui, ViewMode, VisibleTask,
};
pub use mouse::{DOUBLE_CLICK, Drag, DropTarget, Hit, Marker, Mouse};
pub use palette::{
    Entry as PaletteEntry, Palette, Target as PaletteTarget, entries as palette_entries,
};
pub use persist::{RETRY_FIRST, RETRY_MAX, SAVE_DELAY, SETTLE};
pub use update::update;

use anyhow::{Context as _, Result, bail};
use clap::Parser;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEvent, MouseEventKind,
};
use ratatui::crossterm::execute;
use std::collections::VecDeque;
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::boards::{self, Named, Recent};
use crate::clock::Clock;
use crate::command;
use crate::config::{self, Config, DEFAULT_COLUMNS, DateFormat};
use crate::domain::Board;
use crate::paths;
use crate::storage::JsonStore;
use crate::theme::{self, Theme, reduce_motion};
use crate::ui;
use persist::Persistence;

/// How often the idle event loop wakes to refresh relative times.
const IDLE_REFRESH: Duration = Duration::from_secs(1);

#[derive(Debug, Parser)]
#[command(
    name = "tui-kanban",
    about = "A beautiful terminal Kanban board",
    version
)]
pub struct Cli {
    /// The board to open: a path to a board file, or a name, which opens
    /// the recent board with that name or a personal board of that name
    /// in ~/.local/share/tui-kanban/boards. Without it, the nearest
    /// .tui-kanban.json in this directory or above, up to the git root.
    #[arg(long, value_name = "PATH OR NAME")]
    pub board: Option<String>,
    /// The colour theme: auto (Latte on light terminals, Mocha on dark
    /// ones), latte, frappe, macchiato, mocha, ansi, the name of a theme in
    /// ~/.config/tui-kanban/themes, or a path to a .toml theme file.
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,
    /// Turn off animations (a non-empty REDUCE_MOTION does the same).
    #[arg(long)]
    pub no_animation: bool,
    /// Leave the mouse to the terminal, so text can be selected as usual.
    #[arg(long)]
    pub no_mouse: bool,
    #[command(subcommand)]
    pub command: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Where the config file is, or a commented one with every default.
    Config {
        /// Print a config file with every setting at its default, to
        /// start from: tui-kanban config --print-default > <path>
        #[arg(long)]
        print_default: bool,
    },
}

/// How the app looks and behaves, from the config file and the command
/// line.
#[derive(Clone, Debug)]
pub struct Settings {
    pub theme: Theme,
    pub animations: bool,
    pub date_format: DateFormat,
    /// The columns of a new board.
    pub columns: Vec<String>,
    /// Whether a new board is named after where it is (its directory, or
    /// its file), rather than "Project Board".
    pub name_new_boards: bool,
}

impl Settings {
    pub fn new(theme: Theme, animations: bool) -> Self {
        Self {
            theme,
            animations,
            date_format: DateFormat::default(),
            columns: DEFAULT_COLUMNS.map(str::to_owned).to_vec(),
            name_new_boards: false,
        }
    }

    /// The board to start with where there is no file at `path` yet.
    pub fn new_board(&self, path: &std::path::Path) -> Board {
        let mut board = if self.columns == DEFAULT_COLUMNS {
            Board::default()
        } else {
            Board::with_columns("", &self.columns)
        };
        if self.name_new_boards || board.name.is_empty() {
            board.name = boards::default_name(path);
        }
        board
    }
}

/// The runtime: the model plus where it is saved. It turns events into
/// actions, runs [`update`], and carries out the effects.
pub struct App {
    pub model: Model,
    persistence: Persistence,
    /// Created when the first-run tip is dismissed, so it stays dismissed.
    /// Without one, the tip isn't shown.
    tip_marker: Option<PathBuf>,
    /// Text waiting to be edited in `$EDITOR`, which needs the terminal.
    external_edit: Option<(String, ExternalTarget)>,
    quit: bool,
    /// Whether the user chose to quit without saving.
    discarded: bool,
    settings: Settings,
    /// The recently opened boards, for the switcher.
    recent: Option<Recent>,
    /// The personal board, which the switcher always offers.
    personal: Option<PathBuf>,
    /// Whether to watch boards for changes by other programs.
    watching: bool,
}

impl App {
    pub fn new(store: JsonStore, theme: Theme, animations_enabled: bool) -> Result<Self> {
        Self::with_settings(store, Settings::new(theme, animations_enabled))
    }

    pub fn with_settings(store: JsonStore, settings: Settings) -> Result<Self> {
        let path = store.path().to_owned();
        let new_board = settings.new_board(&path);
        let (persistence, board) = Persistence::open(store, new_board)
            .with_context(|| format!("could not load {}", path.display()))?;
        let mut model = Model::new(board, settings.theme.clone(), settings.animations);
        model.ui.date_format = settings.date_format.clone();
        Ok(Self {
            model,
            persistence,
            tip_marker: None,
            external_edit: None,
            quit: false,
            discarded: false,
            settings,
            recent: None,
            personal: None,
            watching: false,
        })
    }

    /// Remembers opened boards in `recent`, and offers them and the
    /// personal board (at `personal`) in the board switcher.
    pub fn with_boards(mut self, recent: Option<Recent>, personal: Option<PathBuf>) -> Self {
        self.recent = recent;
        self.personal = personal;
        self
    }

    /// Asks where to put the board, when none was found where tui-kanban
    /// started: here, or the personal board.
    pub fn ask_for_board(mut self) -> Self {
        let directory = self
            .store()
            .path()
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(|| ".".to_owned(), tidy_path);
        self.model.ui.screens.push(Screen::NoBoard {
            directory,
            personal: self.personal.clone(),
        });
        self
    }

    /// Adds the open board to the recent boards.
    pub fn remember_board(&self) {
        if let Some(recent) = &self.recent {
            recent.record(self.store().path());
        }
    }

    pub fn store(&self) -> &JsonStore {
        self.persistence.store()
    }

    /// Watches the board file, loading changes other programs make.
    pub fn watch_board(mut self) -> Self {
        self.persistence.watch();
        self.watching = true;
        self
    }

    /// Shows the first-run tip unless `marker` exists, and creates it when
    /// the tip is dismissed.
    pub fn with_tip_marker(mut self, marker: PathBuf) -> Self {
        self.model.ui.tip = !marker.exists();
        self.tip_marker = Some(marker);
        self
    }

    pub fn handle_key(&mut self, key: KeyEvent, clock: Clock) {
        if let Some(action) = keymap(&self.model, key) {
            self.dispatch(action, clock);
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.dispatch(Action::Resize(width, height), Clock::now());
    }

    pub fn tick(&mut self, clock: Clock) {
        self.dispatch(Action::Tick, clock);
    }

    /// Catches up with saving: takes in finished saves, loads the file if
    /// another program changed it, and starts a save that is due. Returns
    /// whether anything changed that needs a redraw.
    pub fn poll_saving(&mut self, clock: Clock) -> bool {
        let finished = self.persistence.finished(clock.instant);
        let mut changed = !finished.is_empty();
        for action in finished {
            self.dispatch(action, clock);
        }
        // After the finished saves, so the save state is current.
        let checked = self.persistence.check_disk(&self.model, clock.instant);
        let failed = self.persistence.submit_due(&self.model, clock.instant);
        for action in checked.into_iter().chain(failed) {
            changed = true;
            self.dispatch(action, clock);
        }
        changed
    }

    /// Looks at the board file at the next [`App::poll_saving`] after
    /// [`SETTLE`], as when the watcher sees it change.
    pub fn notice_disk_change(&mut self, clock: Clock) {
        self.persistence.notice_change(clock.instant);
    }

    /// Waits until every change is saved, or has failed to save.
    pub fn flush(&mut self, clock: Clock) {
        for action in self.persistence.flush(&self.model, clock.instant) {
            self.dispatch(action, clock);
        }
    }

    /// Runs an action through `update`, then carries out its effects.
    /// Actions that come out of effects are run the same way.
    pub fn dispatch(&mut self, action: Action, clock: Clock) {
        let mut queue = VecDeque::from([action]);
        while let Some(action) = queue.pop_front() {
            for effect in update(&mut self.model, action, clock) {
                match effect {
                    Effect::Save => self.persistence.request_save(clock.instant),
                    Effect::Overwrite => self.persistence.overwrite(clock.instant),
                    Effect::Reload => queue.push_back(self.persistence.reload()),
                    Effect::Quit => {
                        queue.extend(self.persistence.flush(&self.model, clock.instant));
                        queue.push_back(Action::FinishQuit);
                    }
                    Effect::Exit => {
                        self.quit = true;
                        self.discarded = self.model.session.save_state != SaveState::Saved;
                    }
                    Effect::EditExternally { text, target } => {
                        self.external_edit = Some((text, target));
                    }
                    Effect::ListBoards => queue.push_back(Action::ShowBoards(self.board_entries())),
                    Effect::SwitchBoard(path) => {
                        queue.extend(self.persistence.flush(&self.model, clock.instant));
                        queue.push_back(Action::SwitchReady(path));
                    }
                    Effect::LoadBoard(path) => queue.push_back(self.load_board(path)),
                    Effect::RememberBoard => self.remember_board(),
                    Effect::DismissTip => {
                        // Best effort: if this fails the tip shows again
                        // next time, which is harmless.
                        if let Some(marker) = &self.tip_marker {
                            if let Some(parent) = marker.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(marker, "");
                        }
                    }
                }
            }
        }
    }

    /// Opens the board at `path` in place of this one. The caller has
    /// saved this one first.
    fn load_board(&mut self, path: PathBuf) -> Action {
        let new_board = self.settings.new_board(&path);
        match Persistence::open(JsonStore::new(&path), new_board) {
            Ok((mut persistence, board)) => {
                if self.watching {
                    persistence.watch();
                }
                self.persistence = persistence;
                self.remember_board();
                Action::BoardOpened(board)
            }
            Err(error) => Action::OpenBoardFailed(error.to_string()),
        }
    }

    /// The boards the switcher offers: this one, the recent ones that
    /// still exist, and the personal board.
    pub fn board_entries(&self) -> Vec<BoardEntry> {
        let current = boards::absolute(self.store().path());
        let mut paths = vec![current.clone()];
        let recent = self.recent.as_ref().map(Recent::load).unwrap_or_default();
        for path in recent {
            if !paths.contains(&path) && path.is_file() {
                paths.push(path);
            }
        }
        if let Some(personal) = &self.personal
            && !paths.contains(&boards::absolute(personal))
        {
            paths.push(personal.clone());
        }
        paths
            .into_iter()
            .map(|path| {
                let is_current = path == current;
                let (name, tasks) = if is_current {
                    (
                        self.model.board.name.clone(),
                        Some(self.model.board.task_count()),
                    )
                } else {
                    match boards::summary(&path) {
                        Some(summary) => (summary.name, Some(summary.tasks)),
                        None => (boards::default_name(&path), None),
                    }
                };
                let count = match tasks {
                    Some(1) => "1 task".to_owned(),
                    Some(count) => format!("{count} tasks"),
                    None => "new".to_owned(),
                };
                BoardEntry {
                    detail: format!("{count} · {}", tidy_path(&path)),
                    name,
                    current: is_current,
                    path,
                }
            })
            .collect()
    }

    /// Text the runtime should open in `$EDITOR`, if any.
    pub fn take_external_edit(&mut self) -> Option<(String, ExternalTarget)> {
        self.external_edit.take()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Before exiting, saves anything still unsaved (such as after an
    /// error in the event loop) and reports changes that couldn't be,
    /// unless the user chose to quit without saving.
    pub fn finish(&mut self) -> Result<()> {
        if self.discarded {
            return Ok(());
        }
        self.flush(Clock::now());
        let path = self.store().path().display();
        match &self.model.session.save_state {
            SaveState::Saved => Ok(()),
            SaveState::Saving => bail!("your last changes could not be saved to {path} in time"),
            SaveState::Failed(message) => {
                bail!("your last changes could not be saved to {path}: {message}")
            }
            SaveState::Conflict => {
                bail!("{path} changed on disk, so your last changes were not saved")
            }
        }
    }

    /// How long the event loop may wait for input before it has to redraw:
    /// the next animation frame, the moment a toast expires, or an idle
    /// refresh so relative times such as "5m" stay current.
    pub fn next_timeout(&self, now: Instant) -> Duration {
        let mut timeout = IDLE_REFRESH;
        if let Some(frame) = self.model.ui.animations.next_frame_timeout(now) {
            timeout = timeout.min(frame);
        }
        if let Some(saving) = self.persistence.next_timeout(now) {
            timeout = timeout.min(saving);
        }
        // Wake when a held prefix should show its which-key panel.
        if let Some(pending) = self.model.ui.pending {
            let due = pending.since + crate::ui::WHICH_KEY_DELAY;
            if due > now {
                timeout = timeout.min(due - now);
            }
        }
        if let Some(expires_at) = self
            .model
            .session
            .toast
            .as_ref()
            .and_then(|toast| toast.expires_at)
        {
            timeout = timeout.min(expires_at.saturating_duration_since(now));
        }
        timeout
    }
}

/// `path` with the home directory written as `~`.
fn tidy_path(path: &std::path::Path) -> String {
    match crate::paths::home_dir().and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = paths::config_file();
    if let Some(Subcommand::Config { print_default }) = cli.command {
        if print_default {
            print!("{}", config::default_file());
        } else {
            match &config_path {
                Some(path) if path.exists() => println!("{}", path.display()),
                Some(path) => println!(
                    "{} (not created yet; tui-kanban config --print-default prints one)",
                    path.display()
                ),
                None => println!("there is no config directory (set HOME or XDG_CONFIG_HOME)"),
            }
        }
        return Ok(());
    }
    let config = match &config_path {
        Some(path) => Config::load(path)?,
        None => Config::default(),
    };
    command::install(config.command_table()?);

    let animations = !cli.no_animation
        && config.animations
        && !reduce_motion(env::var_os("REDUCE_MOTION").as_deref());
    let theme = theme::resolve(
        cli.theme.as_deref().or(config.theme.as_deref()),
        &theme::Environment::from_env(),
        theme::detect_light_background,
    )?;
    let settings = Settings {
        date_format: config.date_format.clone(),
        columns: config.columns.clone(),
        name_new_boards: true,
        ..Settings::new(theme, animations)
    };
    let state = paths::state_dir();
    let recent = state
        .as_ref()
        .map(|state| Recent::new(state.join("recent-boards")));
    let data = paths::data_dir();
    let personal = data
        .as_ref()
        .map(|data| boards::named(data, boards::PERSONAL));

    let (path, found) = match cli.board.as_deref().map(Named::parse) {
        Some(Named::Path(path)) => (path, true),
        Some(Named::Name(name)) => {
            let recent_boards = recent.as_ref().map(Recent::load).unwrap_or_default();
            let path = boards::resolve_name(&name, &recent_boards, data.as_deref()).with_context(
                || format!("there is no data directory for a board called {name}; give a path"),
            )?;
            (path, true)
        }
        None => {
            let here = env::current_dir().context("could not read the current directory")?;
            match boards::find_upward(&here, paths::home_dir().as_deref()) {
                Some(path) => (path, true),
                None => (here.join(boards::FILE_NAME), false),
            }
        }
    };
    let mut app = App::with_settings(JsonStore::new(path), settings)?
        .watch_board()
        .with_boards(recent, personal);
    if found {
        app.remember_board();
    } else {
        app = app.ask_for_board();
    }
    if let Some(state) = state {
        app = app.with_tip_marker(state.join("tip-dismissed"));
    }
    let mouse = !cli.no_mouse && config.mouse;
    let mut terminal = init_terminal(mouse)?;
    if let Ok(size) = terminal.size() {
        app.resize(size.width, size.height);
    }
    let result = run_loop(&mut terminal, &mut app, mouse);
    let restore_result = restore_terminal();
    result.and(restore_result).and(app.finish())
}

/// Enters the alternate screen and raw mode, with bracketed paste so a
/// multi-line paste arrives as one piece of text rather than as keys, and
/// with mouse capture unless `mouse` is off. Capturing the mouse stops the
/// terminal selecting text, except with Shift held in most terminals.
fn init_terminal(mouse: bool) -> Result<DefaultTerminal> {
    let terminal = ratatui::try_init().context("could not initialize terminal")?;
    // Terminals without bracketed paste or a mouse ignore the requests.
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    if mouse {
        let _ = execute!(io::stdout(), EnableMouseCapture);
    }
    Ok(terminal)
}

/// Leaves the TUI while the user edits `text` in their own editor, then
/// comes back to it.
fn edit_externally(
    terminal: &mut DefaultTerminal,
    text: &str,
    mouse: bool,
) -> Result<std::result::Result<String, String>> {
    restore_terminal()?;
    let result = crate::tui::external::edit(text);
    *terminal = init_terminal(mouse)?;
    terminal.clear()?;
    Ok(result)
}

fn restore_terminal() -> Result<()> {
    let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
    ratatui::try_restore().context("could not restore terminal")
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App, mouse: bool) -> Result<()> {
    let mut needs_redraw = true;
    loop {
        // Tick on every iteration, not only when polling times out, so
        // toasts expire and animations finish while keys keep arriving.
        let clock = Clock::now();
        needs_redraw |= app.poll_saving(clock);
        app.tick(clock);
        if needs_redraw {
            terminal.draw(|frame| ui::render(frame, &app.model, clock))?;
            needs_redraw = false;
        }
        if app.should_quit() {
            return Ok(());
        }

        let timeout = app.next_timeout(clock.instant);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    app.handle_key(key, Clock::now());
                    needs_redraw = true;
                }
                Event::Paste(text) => {
                    app.dispatch(Action::Paste(text), Clock::now());
                    needs_redraw = true;
                }
                // Plain pointer movement changes nothing on screen.
                Event::Mouse(event) if event.kind != MouseEventKind::Moved => {
                    app.dispatch(Action::Mouse(event), Clock::now());
                    needs_redraw = true;
                }
                Event::Resize(width, height) => {
                    app.resize(width, height);
                    needs_redraw = true;
                }
                _ => {}
            }
            // A key, or a click on a hint, can ask for $EDITOR.
            if let Some((text, target)) = app.take_external_edit() {
                let result = edit_externally(terminal, &text, mouse)?;
                app.dispatch(
                    Action::ExternalEditFinished { target, result },
                    Clock::now(),
                );
            }
        } else {
            // An animation frame is due, a toast expired, a save finished,
            // or it is time for the idle refresh.
            needs_redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{AnimationKind, FRAME_INTERVAL};
    use crate::domain::Board;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers, MouseEventKind};
    use tempfile::TempDir;
    use uuid::Uuid;

    fn test_app() -> (TempDir, App) {
        test_app_with(false)
    }

    fn test_app_with(animations: bool) -> (TempDir, App) {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let mut app = App::new(store, Theme::mocha(), animations).unwrap();
        app.resize(100, 30);
        (directory, app)
    }

    fn clock() -> Clock {
        Clock::fixed(0)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(app: &mut App, codes: &[KeyCode]) {
        for code in codes {
            app.handle_key(key(*code), clock());
        }
    }

    fn type_text(app: &mut App, text: &str) {
        for character in text.chars() {
            app.handle_key(key(KeyCode::Char(character)), clock());
        }
    }

    /// Adds a task directly to the board, as if loaded from disk.
    fn add(app: &mut App, column: usize, title: &str) -> Uuid {
        let id = app.model.board.add_task(column, title, "", 0).unwrap();
        app.model.reconcile_selection();
        id
    }

    fn top_screen(app: &App) -> Option<&Screen> {
        app.model.ui.screens.last()
    }

    #[test]
    fn idle_loop_waits_for_the_refresh_interval() {
        let (_directory, app) = test_app();
        assert_eq!(app.next_timeout(Instant::now()), IDLE_REFRESH);
    }

    #[test]
    fn animations_run_at_frame_rate() {
        let (_directory, mut app) = test_app_with(true);
        let clock = clock();
        app.handle_key(key(KeyCode::Char('?')), clock);
        assert_eq!(app.next_timeout(clock.instant), FRAME_INTERVAL);
    }

    #[test]
    fn edit_and_new_task_modals_both_animate() {
        let (_directory, mut app) = test_app_with(true);
        add(&mut app, 0, "Edit me");
        for opener in ['n', 'e'] {
            app.model.ui.screens.clear();
            app.model.ui.animations.finish(AnimationKind::Modal);
            app.handle_key(key(KeyCode::Char(opener)), clock());
            assert!(
                matches!(top_screen(&app), Some(Screen::Editor(_))),
                "{opener}"
            );
            assert!(
                app.model
                    .ui
                    .animations
                    .progress(AnimationKind::Modal, clock().instant)
                    .is_some(),
                "{opener} should start the modal animation the editor reads"
            );
        }
    }

    #[test]
    fn toasts_wake_the_loop_when_they_expire() {
        let (_directory, mut app) = test_app();
        let clock = clock();
        app.model.session.toast = Some(Toast {
            message: "hello".to_owned(),
            kind: ToastKind::Info,
            expires_at: Some(clock.instant + Duration::from_millis(400)),
        });
        assert_eq!(
            app.next_timeout(clock.instant + Duration::from_millis(100)),
            Duration::from_millis(300)
        );
        app.tick(clock.advance(Duration::from_millis(400)));
        assert!(app.model.session.toast.is_none());
    }

    #[test]
    fn editor_creates_and_persists_a_task() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Ship it");
        press(&mut app, &[KeyCode::Tab]);
        type_text(&mut app, "Make it");
        // #22: Enter starts a new line in the description; Ctrl+S saves.
        press(&mut app, &[KeyCode::Enter]);
        type_text(&mut app, "beautiful");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            clock(),
        );
        assert_eq!(app.model.board.task_count(), 1);
        assert_eq!(app.model.board.columns[0].tasks[0].title, "Ship it");
        assert_eq!(
            app.model.board.columns[0].tasks[0].description,
            "Make it\nbeautiful"
        );
        assert!(app.model.ui.screens.is_empty());
        app.flush(clock());
        assert_eq!(app.store().load().unwrap().unwrap(), app.model.board);
    }

    #[test]
    fn pasting_inserts_text_where_the_user_is_typing() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('n')]);
        app.dispatch(Action::Paste("Two\nlines".to_owned()), clock());
        press(&mut app, &[KeyCode::Tab]);
        app.dispatch(Action::Paste("Line one\r\nLine two".to_owned()), clock());
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::BackTab, KeyCode::Enter],
        );
        let task = &app.model.board.columns[0].tasks[0];
        assert_eq!(task.title, "Two lines");
        assert_eq!(task.description, "Line one\nLine two");
        press(&mut app, &[KeyCode::Char('/')]);
        app.dispatch(Action::Paste("two".to_owned()), clock());
        assert_eq!(app.model.ui.search.query, "two");
    }

    #[test]
    fn cancelling_a_changed_draft_asks_first() {
        // #23: Esc and Ctrl+C used to throw a draft away without asking.
        let (_directory, mut app) = test_app();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        // An untouched draft closes straight away.
        press(&mut app, &[KeyCode::Char('n'), KeyCode::Esc]);
        assert!(app.model.ui.screens.is_empty());
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Half-written");
        app.handle_key(ctrl_c, clock());
        assert!(!app.should_quit());
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmDiscard)));
        // n keeps editing, with the draft intact.
        press(&mut app, &[KeyCode::Char('n')]);
        match top_screen(&app) {
            Some(Screen::Editor(editor)) => assert_eq!(editor.title_text(), "Half-written"),
            other => panic!("expected the editor, got {other:?}"),
        }
        press(&mut app, &[KeyCode::Esc, KeyCode::Char('y')]);
        assert!(app.model.ui.screens.is_empty());
        assert_eq!(app.model.board.task_count(), 0);
    }

    #[test]
    fn descriptions_can_be_edited_in_an_external_editor() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Write it up");
        press(&mut app, &[KeyCode::Char('E')]);
        let (text, target) = app.take_external_edit().expect("an edit request");
        assert_eq!((text.as_str(), target), ("", ExternalTarget::Task(id)));
        let finished = |result| Action::ExternalEditFinished { target, result };
        app.dispatch(finished(Ok("# Plan\n\n- [ ] draft\n".to_owned())), clock());
        assert_eq!(
            app.model.board.task(id).unwrap().description,
            "# Plan\n\n- [ ] draft"
        );
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(app.model.board.task(id).unwrap().description, "");
        app.dispatch(finished(Err("vi exited with 1".to_owned())), clock());
        assert_eq!(toast_message(&app), "Editor: vi exited with 1");

        // From the editor overlay, the text goes back into the draft.
        press(&mut app, &[KeyCode::Char('e')]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
            clock(),
        );
        let (_, target) = app.take_external_edit().expect("an edit request");
        assert_eq!(target, ExternalTarget::Draft);
        app.dispatch(
            Action::ExternalEditFinished {
                target,
                result: Ok("From vim".to_owned()),
            },
            clock(),
        );
        match top_screen(&app) {
            Some(Screen::Editor(editor)) => {
                assert_eq!(editor.description_text(), "From vim");
                assert_eq!(editor.field, EditorField::Description);
            }
            other => panic!("expected the editor, got {other:?}"),
        }
    }

    #[test]
    fn space_ticks_checklist_items_in_the_drawer() {
        let (_directory, mut app) = test_app();
        let id = app
            .model
            .board
            .add_task(
                0,
                "Release",
                "Steps:\n- [ ] tag\n- [ ] build\n- [x] notes",
                0,
            )
            .unwrap();
        app.model.reconcile_selection();
        let description = |app: &App| app.model.board.task(id).unwrap().description.clone();
        press(&mut app, &[KeyCode::Enter, KeyCode::Char(' ')]);
        assert!(description(&app).contains("- [x] tag"));
        assert_eq!(toast_message(&app), "");
        press(&mut app, &[KeyCode::Tab, KeyCode::Char(' ')]);
        assert_eq!(crate::markdown::progress(&description(&app)), Some((3, 3)));
        // Shift+Tab wraps around to the last item.
        press(
            &mut app,
            &[KeyCode::BackTab, KeyCode::BackTab, KeyCode::Char(' ')],
        );
        assert!(description(&app).contains("- [ ] notes"));
        press(&mut app, &[KeyCode::Char('u')]);
        assert!(description(&app).contains("- [x] notes"));
        assert_eq!(
            toast_message(&app),
            "Undid: tick 'notes'".replace("tick", "untick")
        );
    }

    #[test]
    fn empty_title_keeps_the_editor_open() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('n'), KeyCode::Enter]);
        match top_screen(&app) {
            Some(Screen::Editor(editor)) => {
                assert_eq!(editor.error.as_deref(), Some("Title cannot be empty"));
            }
            other => panic!("expected the editor, got {other:?}"),
        }
        assert_eq!(app.model.board.task_count(), 0);
    }

    #[test]
    fn search_escape_clears_the_query() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Design cards");
        add(&mut app, 0, "Write tests");
        press(&mut app, &[KeyCode::Char('/')]);
        type_text(&mut app, "design");
        assert_eq!(app.model.visible_task_indices(0), vec![0]);
        press(&mut app, &[KeyCode::Esc]);
        assert_eq!(app.model.ui.search.query, "");
        assert_eq!(app.model.visible_task_indices(0), vec![0, 1]);
        assert!(!app.model.ui.search.is_typing());
    }

    #[test]
    fn arrows_move_through_matches_while_typing() {
        // #28: ↑/↓ and Ctrl+N/Ctrl+P used to do nothing until Enter.
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 0, "Fix the login bug");
        add(&mut app, 0, "Write docs");
        let second = add(&mut app, 2, "Fix typo");
        press(&mut app, &[KeyCode::Char('/')]);
        type_text(&mut app, "fix");
        assert_eq!(app.model.selected_task_id(), Some(first));
        press(&mut app, &[KeyCode::Down]);
        assert_eq!(app.model.selected_task_id(), Some(second));
        assert_eq!(app.model.ui.active_column, 2);
        assert!(app.model.ui.search.is_typing());
        // It wraps around.
        press(&mut app, &[KeyCode::Down]);
        assert_eq!(app.model.selected_task_id(), Some(first));
        // After applying, Ctrl+N / Ctrl+P do the same.
        press(&mut app, &[KeyCode::Enter]);
        let ctrl = |character| KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL);
        app.handle_key(ctrl('p'), clock());
        assert_eq!(app.model.selected_task_id(), Some(second));
        app.handle_key(ctrl('n'), clock());
        assert_eq!(app.model.selected_task_id(), Some(first));
    }

    #[test]
    fn backspace_removes_the_last_filter_term() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Fix the login bug");
        add(&mut app, 1, "Fix typo");
        press(&mut app, &[KeyCode::Char('/')]);
        type_text(&mut app, "in:backlog fix");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.visible_tasks().len(), 1);
        press(&mut app, &[KeyCode::Backspace]);
        assert_eq!(app.model.ui.search.query, "in:backlog");
        press(&mut app, &[KeyCode::Backspace]);
        assert_eq!(app.model.ui.search.query, "");
        assert_eq!(app.model.visible_tasks().len(), 2);
    }

    #[test]
    fn moving_a_task_changes_column_and_selection() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Move me");
        press(&mut app, &[KeyCode::Char('L')]);
        assert_eq!(app.model.board.task_location(id), Some((1, 0)));
        assert_eq!(app.model.ui.active_column, 1);
        assert_eq!(app.model.selected_task_id(), Some(id));
    }

    #[test]
    fn delete_requires_confirmation() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Delete me");
        press(&mut app, &[KeyCode::Char('d')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::ConfirmDelete { .. })
        ));
        press(&mut app, &[KeyCode::Esc]);
        assert_eq!(app.model.board.task_count(), 1);
        assert!(app.model.ui.screens.is_empty());
        press(&mut app, &[KeyCode::Char('d'), KeyCode::Char('y')]);
        assert_eq!(app.model.board.task_count(), 0);
    }

    #[test]
    fn toggles_all_tasks_and_rail_focus() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('v')]);
        assert_eq!(app.model.ui.view, ViewMode::AllTasks);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
        press(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.model.ui.focus, FocusRegion::Rail);
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
    }

    #[test]
    fn all_tasks_search_selects_the_first_match_anywhere() {
        // #4: a search that filters out the selected column used to clear
        // the selection, and Down then skipped the first match.
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "beta");
        let first = add(&mut app, 1, "alpha one");
        let second = add(&mut app, 1, "alpha two");
        press(&mut app, &[KeyCode::Char('v'), KeyCode::Char('/')]);
        type_text(&mut app, "alpha");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.selected_task_id(), Some(first));
        assert_eq!(app.model.ui.active_column, 1);
        // Both matches share a row of the grid.
        press(&mut app, &[KeyCode::Right]);
        assert_eq!(app.model.selected_task_id(), Some(second));
    }

    #[test]
    fn all_tasks_grid_moves_spatially() {
        // #27: j/k move by row keeping the position in the row, h/l move
        // within the row, and [ / ] jump between column groups.
        let (_directory, mut app) = test_app();
        app.resize(160, 45);
        let first: Vec<Uuid> = (0..5).map(|n| add(&mut app, 0, &format!("a{n}"))).collect();
        let second: Vec<Uuid> = (0..2).map(|n| add(&mut app, 1, &format!("b{n}"))).collect();
        // Rows of three: [a0 a1 a2] [a3 a4] [b0 b1].
        press(&mut app, &[KeyCode::Char('v'), KeyCode::Char('l')]);
        assert_eq!(app.model.selected_task_id(), Some(first[1]));
        let steps = [
            ('j', first[4]),
            ('j', second[1]),
            ('k', first[4]),
            ('l', first[4]),
            ('h', first[3]),
            ('k', first[0]),
            (']', second[0]),
            ('[', first[0]),
        ];
        for (key, expected) in steps {
            press(&mut app, &[KeyCode::Char(key)]);
            assert_eq!(app.model.selected_task_id(), Some(expected), "after {key}");
        }
    }

    #[test]
    fn down_with_nothing_selected_starts_at_the_first_task() {
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 0, "First");
        add(&mut app, 0, "Second");
        let last = add(&mut app, 0, "Last");
        app.model.ui.selected = None;
        press(&mut app, &[KeyCode::Down]);
        assert_eq!(app.model.selected_task_id(), Some(first));
        app.model.ui.selected = None;
        press(&mut app, &[KeyCode::Up]);
        assert_eq!(app.model.selected_task_id(), Some(last));
    }

    #[test]
    fn selection_follows_the_task_not_its_position() {
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 0, "First");
        let second = add(&mut app, 0, "Second");
        press(&mut app, &[KeyCode::Down]);
        assert_eq!(app.model.selected_task_id(), Some(second));
        // Moving another task out from above it doesn't change the selection.
        app.model.board.move_task(first, 2, None, 0).unwrap();
        app.model.reconcile_selection();
        assert_eq!(app.model.selected_task_id(), Some(second));
    }

    #[test]
    fn deleting_selects_the_next_task() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "First");
        add(&mut app, 0, "Second");
        let third = add(&mut app, 0, "Third");
        press(
            &mut app,
            &[KeyCode::Down, KeyCode::Char('d'), KeyCode::Char('y')],
        );
        assert_eq!(app.model.selected_task_id(), Some(third));
    }

    #[test]
    fn tab_does_not_focus_a_hidden_rail() {
        // #5: below the rail's breakpoint, Tab used to move focus to it.
        let (_directory, mut app) = test_app();
        app.resize(60, 20);
        press(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
        app.resize(100, 30);
        press(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.model.ui.focus, FocusRegion::Rail);
        // Shrinking the terminal hides the rail, so focus leaves it.
        app.resize(60, 20);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
    }

    #[test]
    fn all_tasks_navigation_keeps_task_identity() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "First");
        let second = add(&mut app, 1, "Second");
        press(&mut app, &[KeyCode::Char('v'), KeyCode::Down]);
        assert_eq!(app.model.selected_task_id(), Some(second));
        assert_eq!(app.model.ui.active_column, 1);
    }

    #[test]
    fn escape_from_a_dialog_returns_to_the_detail_drawer() {
        // #7: dialogs opened from the drawer used to return to the board.
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Inspect me");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::Char('d'), KeyCode::Esc],
        );
        assert!(matches!(top_screen(&app), Some(Screen::Detail { task, .. }) if *task == id));
        press(&mut app, &[KeyCode::Char('e'), KeyCode::Esc]);
        assert!(matches!(top_screen(&app), Some(Screen::Detail { .. })));
        press(&mut app, &[KeyCode::Char('?'), KeyCode::Esc]);
        assert!(matches!(top_screen(&app), Some(Screen::Detail { .. })));
        press(&mut app, &[KeyCode::Esc]);
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn saving_from_the_drawer_returns_to_it() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Old title");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::Char('e'), KeyCode::End],
        );
        type_text(&mut app, "!");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.task(id).unwrap().title, "Old title!");
        assert!(matches!(top_screen(&app), Some(Screen::Detail { .. })));
    }

    #[test]
    fn deleting_from_the_drawer_closes_it() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Doomed");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::Char('d'), KeyCode::Char('y')],
        );
        assert_eq!(app.model.board.task_count(), 0);
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn tiny_terminals_only_accept_quit() {
        // #16: modals too small to draw used to still accept keys, so "d"
        // then "y" deleted a task without showing anything.
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Keep me");
        app.resize(30, 8);
        press(&mut app, &[KeyCode::Char('d'), KeyCode::Char('y')]);
        assert_eq!(app.model.board.task_count(), 1);
        assert!(app.model.ui.screens.is_empty());
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(app.should_quit());
    }

    #[test]
    fn a_failed_save_keeps_the_change_and_retries() {
        // #18: a failed save used to throw the change away.
        let (directory, mut app) = test_app();
        // A directory where the board file should be makes every save fail.
        let path = directory.path().join("board.json");
        std::fs::create_dir(&path).unwrap();
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Unsaved");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.session.save_state, SaveState::Saving);
        app.flush(clock());
        assert_eq!(app.model.board.task_count(), 1);
        assert!(matches!(app.model.session.save_state, SaveState::Failed(_)));
        assert_eq!(
            app.model.session.toast.as_ref().map(|toast| toast.kind),
            Some(ToastKind::Error)
        );
        // The error stays up until the next key press.
        app.tick(clock().advance(Duration::from_secs(60)));
        assert!(app.model.session.toast.is_some());
        press(&mut app, &[KeyCode::Char('j')]);
        assert!(app.model.session.toast.is_none());
        // Quitting asks first, and finish() reports the lost change.
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(!app.should_quit());
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmQuit)));
        assert!(app.finish().is_err());
        press(&mut app, &[KeyCode::Char('n')]);
        assert!(app.model.ui.screens.is_empty());

        // Once the problem is fixed, the retry saves the change.
        std::fs::remove_dir(&path).unwrap();
        let later = clock().advance(RETRY_MAX);
        app.poll_saving(later);
        app.flush(later);
        assert_eq!(app.model.session.save_state, SaveState::Saved);
        assert_eq!(app.store().load().unwrap().unwrap(), app.model.board);
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(app.should_quit());
        assert!(app.finish().is_ok());
    }

    #[test]
    fn changes_are_saved_in_the_background_after_a_short_delay() {
        let (_directory, mut app) = test_app();
        let start = clock();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "One");
        app.handle_key(key(KeyCode::Enter), start);
        type_text(&mut app, "Two");
        app.handle_key(key(KeyCode::Enter), start);
        assert_eq!(app.model.session.save_state, SaveState::Saving);
        assert_eq!(app.next_timeout(start.instant), SAVE_DELAY);
        // Not yet due: nothing is written.
        app.poll_saving(start);
        assert!(app.store().load().unwrap().is_none());
        // Both changes are written in one save.
        app.poll_saving(start.advance(SAVE_DELAY));
        app.flush(start.advance(SAVE_DELAY));
        assert_eq!(app.model.session.save_state, SaveState::Saved);
        assert_eq!(app.store().load().unwrap().unwrap().task_count(), 2);
        assert_eq!(app.next_timeout(start.instant), IDLE_REFRESH);
    }

    #[test]
    fn a_save_of_an_older_change_leaves_the_newer_one_unsaved() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Task");
        press(&mut app, &[KeyCode::Char('J')]);
        let revision = app.model.session.revision;
        press(&mut app, &[KeyCode::Char('L')]);
        let finished = |revision| Action::SaveFinished {
            revision,
            result: Ok(()),
        };
        app.dispatch(finished(revision), clock());
        assert_eq!(app.model.session.save_state, SaveState::Saving);
        app.dispatch(finished(revision + 1), clock());
        assert_eq!(app.model.session.save_state, SaveState::Saved);
    }

    #[test]
    fn quitting_saves_pending_changes_first() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "Last-second change");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::Esc, KeyCode::Char('q')],
        );
        assert!(app.should_quit());
        assert!(app.finish().is_ok());
        assert_eq!(app.store().load().unwrap().unwrap(), app.model.board);
    }

    #[test]
    fn a_tiny_terminal_can_still_quit_with_unsaved_changes() {
        let (directory, mut app) = test_app();
        std::fs::create_dir(directory.path().join("board.json")).unwrap();
        add(&mut app, 0, "Unsaved");
        press(&mut app, &[KeyCode::Char('L')]);
        app.resize(30, 8);
        // The question can't be shown, so a second q answers it.
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(!app.should_quit());
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(app.should_quit());
    }

    /// Writes `board`, with one task renamed, as another program would.
    fn edit_on_disk(app: &App, title: &str) -> Board {
        let mut board = app.store().load().unwrap().unwrap();
        let id = board.columns[0].tasks[0].id;
        board.update_task(id, title, "", 1).unwrap();
        std::fs::write(
            app.store().path(),
            serde_json::to_string_pretty(&board).unwrap(),
        )
        .unwrap();
        board
    }

    fn saved_app() -> (TempDir, App, Uuid) {
        let (directory, mut app) = test_app();
        let id = add(&mut app, 0, "Mine");
        app.store().save(&app.model.board).unwrap();
        // Start from the file as saved.
        let mut app = App::new(app.store().clone(), Theme::mocha(), false).unwrap();
        app.resize(100, 30);
        (directory, app, id)
    }

    #[test]
    fn changes_on_disk_are_loaded_when_nothing_is_unsaved() {
        let (_directory, mut app, id) = saved_app();
        press(&mut app, &[KeyCode::Char('v')]);
        let theirs = edit_on_disk(&app, "Theirs");
        app.notice_disk_change(clock());
        // The file is read once it has settled.
        assert!(!app.poll_saving(clock()));
        assert!(app.poll_saving(clock().advance(SETTLE)));
        assert_eq!(app.model.board, theirs);
        assert_eq!(app.model.selected_task_id(), Some(id));
        assert_eq!(app.model.ui.view, ViewMode::AllTasks);
        assert_eq!(app.model.session.save_state, SaveState::Saved);
        assert_eq!(toast_message(&app), "Reloaded: the board changed on disk");
        // Our own saves aren't reloaded.
        press(&mut app, &[KeyCode::Char('u')]);
        app.flush(clock());
        app.notice_disk_change(clock());
        assert!(!app.poll_saving(clock().advance(SETTLE)));
        assert_eq!(app.model.board.task(id).unwrap().title, "Mine");
    }

    #[test]
    fn an_unreadable_change_on_disk_is_reported_once() {
        let (_directory, mut app, _) = saved_app();
        let board = app.model.board.clone();
        std::fs::write(app.store().path(), "<<<<<<< HEAD").unwrap();
        app.notice_disk_change(clock());
        assert!(app.poll_saving(clock().advance(SETTLE)));
        assert!(toast_message(&app).starts_with("Could not load the changed board"));
        assert_eq!(app.model.board, board);
        press(&mut app, &[KeyCode::Esc]);
        app.notice_disk_change(clock());
        assert!(!app.poll_saving(clock().advance(SETTLE)));
    }

    #[test]
    fn a_change_on_disk_with_unsaved_changes_here_asks_which_to_keep() {
        // #19: the last writer used to win silently.
        let (_directory, mut app, id) = saved_app();
        press(&mut app, &[KeyCode::Char('L')]);
        edit_on_disk(&app, "Theirs");
        app.notice_disk_change(clock());
        app.poll_saving(clock().advance(SETTLE));
        assert_eq!(app.model.session.save_state, SaveState::Conflict);
        assert!(matches!(top_screen(&app), Some(Screen::Conflict)));
        // Esc decides later; nothing is written meanwhile.
        press(&mut app, &[KeyCode::Esc]);
        assert!(app.model.ui.screens.is_empty());
        app.flush(clock());
        assert_eq!(
            app.store().load().unwrap().unwrap().columns[0].tasks[0].title,
            "Theirs"
        );
        // The next change asks again; "o" keeps ours.
        press(&mut app, &[KeyCode::Char('H')]);
        app.poll_saving(clock().advance(SAVE_DELAY));
        app.flush(clock());
        assert!(matches!(top_screen(&app), Some(Screen::Conflict)));
        press(&mut app, &[KeyCode::Char('o')]);
        app.flush(clock());
        assert_eq!(app.model.session.save_state, SaveState::Saved);
        let saved = app.store().load().unwrap().unwrap();
        assert_eq!(saved, app.model.board);
        assert_eq!(saved.task(id).unwrap().title, "Mine");
        assert_eq!(saved.task_location(id), Some((0, 0)));
    }

    #[test]
    fn two_instances_on_one_file_do_not_overwrite_each_other() {
        let (_directory, mut first, id) = saved_app();
        let mut second = App::new(first.store().clone(), Theme::mocha(), false).unwrap();
        second.resize(100, 30);
        press(&mut first, &[KeyCode::Char('L')]);
        first.poll_saving(clock().advance(SAVE_DELAY));
        first.flush(clock());
        press(&mut second, &[KeyCode::Char('d'), KeyCode::Char('y')]);
        second.poll_saving(clock().advance(SAVE_DELAY));
        second.flush(clock());
        assert!(matches!(top_screen(&second), Some(Screen::Conflict)));
        // "r" loads the first instance's board; u brings this one's back.
        press(&mut second, &[KeyCode::Char('r')]);
        assert_eq!(second.model.board, first.model.board);
        assert_eq!(second.model.board.task_location(id), Some((1, 0)));
        assert!(second.model.ui.screens.is_empty());
        assert_eq!(second.model.session.save_state, SaveState::Saved);
        press(&mut second, &[KeyCode::Char('u')]);
        assert_eq!(second.model.board.task_count(), 0);
    }

    #[test]
    fn quitting_with_a_conflict_asks_and_leaves_the_file_alone() {
        let (_directory, mut app, _) = saved_app();
        press(&mut app, &[KeyCode::Char('L')]);
        let theirs = edit_on_disk(&app, "Theirs");
        press(&mut app, &[KeyCode::Char('q')]);
        // Quitting saves first, which finds the conflict, so it asks,
        // above the choice between the two versions.
        assert!(!app.should_quit());
        assert!(matches!(
            app.model.ui.screens.as_slice(),
            [Screen::Conflict, Screen::ConfirmQuit]
        ));
        press(&mut app, &[KeyCode::Char('n')]);
        assert!(matches!(top_screen(&app), Some(Screen::Conflict)));
        press(&mut app, &[KeyCode::Esc, KeyCode::Char('q')]);
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmQuit)));
        // Ctrl+C a second time quits, too.
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            clock(),
        );
        assert!(app.should_quit());
        assert!(app.finish().is_ok());
        assert_eq!(app.store().load().unwrap().unwrap(), theirs);
    }

    /// Adds `count` tasks to the first column, each with a description.
    fn add_many(app: &mut App, count: usize) -> Vec<Uuid> {
        (0..count)
            .map(|index| {
                let title = format!("Task {index}");
                let id = app.model.board.add_task(0, &title, "details", 0).unwrap();
                app.model.reconcile_selection();
                id
            })
            .collect()
    }

    #[test]
    fn lanes_scroll_to_the_selection_and_remember_it() {
        let (_directory, mut app) = test_app();
        app.resize(100, 20);
        let tasks = add_many(&mut app, 10);
        assert_eq!(app.model.ui.scroll.lane(0), 0);
        press(&mut app, &[KeyCode::End]);
        assert_eq!(app.model.selected_task_id(), Some(tasks[9]));
        let scrolled = app.model.ui.scroll.lane(0);
        assert!(scrolled > 0);
        // #31: moving focus to the rail and back keeps the scroll position.
        press(&mut app, &[KeyCode::Tab]);
        assert_eq!(app.model.ui.scroll.lane(0), scrolled);
        press(
            &mut app,
            &[KeyCode::Tab, KeyCode::Char('l'), KeyCode::Char('h')],
        );
        assert_eq!(app.model.ui.scroll.lane(0), scrolled);
        // Coming back to the lane selects its first card on screen.
        assert_eq!(app.model.selected_task_id(), Some(tasks[scrolled]));
        // Moving up only scrolls once the selection reaches the top.
        press(&mut app, &[KeyCode::Up]);
        assert_eq!(app.model.ui.scroll.lane(0), scrolled - 1);
    }

    #[test]
    fn lanes_scroll_sideways_to_the_active_column() {
        // #25: with many columns, lanes keep a minimum width and scroll.
        let (_directory, mut app) = test_app();
        app.model.board.columns = (0..8)
            .map(|index| crate::domain::Column::new(format!("c{index}"), format!("Stage {index}")))
            .collect();
        app.resize(100, 30);
        assert_eq!(app.model.ui.scroll.first_lane, 0);
        press(&mut app, &[KeyCode::Char('l'); 3]);
        assert_eq!(app.model.ui.active_column, 3);
        // 76 cells fit three lanes of 24 or wider, so the view follows.
        assert_eq!(app.model.ui.scroll.first_lane, 1);
        press(&mut app, &[KeyCode::Char('h'); 2]);
        assert_eq!(app.model.ui.scroll.first_lane, 1);
        press(&mut app, &[KeyCode::Char('h')]);
        assert_eq!(app.model.ui.scroll.first_lane, 0);
    }

    #[test]
    fn detail_scroll_stops_at_the_end_of_the_text() {
        // #8: PgDn used to scroll past the description into blank space.
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Short");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::PageDown, KeyCode::Char('j')],
        );
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Detail { scroll: 0, .. })
        ));

        let long: String = (0..60).map(|line| format!("line {line}\n")).collect();
        app.model
            .board
            .update_task(id, "Long".to_owned(), long, 0)
            .unwrap();
        press(&mut app, &[KeyCode::Char('j'), KeyCode::Char('j')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Detail { scroll: 2, .. })
        ));
        press(&mut app, &[KeyCode::PageDown; 20]);
        let Some(Screen::Detail { scroll, .. }) = top_screen(&app) else {
            panic!("expected the detail drawer");
        };
        // 60 lines in a drawer of 30 rows: it stops with the last line at
        // the bottom.
        assert!(*scroll > 30 && *scroll < 60, "{scroll}");
        let end = *scroll;
        press(&mut app, &[KeyCode::Char('k')]);
        assert!(
            matches!(top_screen(&app), Some(Screen::Detail { scroll, .. }) if *scroll == end - 1)
        );
    }

    fn toast_message(app: &App) -> &str {
        app.model
            .session
            .toast
            .as_ref()
            .map_or("", |toast| toast.message.as_str())
    }

    #[test]
    fn undo_restores_a_deleted_task_and_its_selection() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "First");
        let doomed = add(&mut app, 0, "Doomed");
        press(
            &mut app,
            &[KeyCode::Down, KeyCode::Char('d'), KeyCode::Char('y')],
        );
        assert_eq!(app.model.board.task_count(), 1);
        assert_eq!(toast_message(&app), "Task deleted · u to undo");
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(app.model.board.task_count(), 2);
        assert_eq!(app.model.selected_task_id(), Some(doomed));
        assert_eq!(toast_message(&app), "Undid: delete 'Doomed'");
        // The undo was saved, too.
        app.flush(clock());
        assert_eq!(app.store().load().unwrap().unwrap(), app.model.board);
    }

    #[test]
    fn undo_and_redo_a_move() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Ship it");
        press(&mut app, &[KeyCode::Char('L'), KeyCode::Char('L')]);
        assert_eq!(app.model.board.task_location(id), Some((2, 0)));
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(app.model.board.task_location(id), Some((1, 0)));
        assert_eq!(toast_message(&app), "Undid: move 'Ship it' → Done");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            clock(),
        );
        assert_eq!(app.model.board.task_location(id), Some((0, 0)));
        press(&mut app, &[KeyCode::Char('U')]);
        assert_eq!(app.model.board.task_location(id), Some((1, 0)));
        assert_eq!(toast_message(&app), "Redid: move 'Ship it' → In Progress");
        app.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            clock(),
        );
        assert_eq!(app.model.board.task_location(id), Some((2, 0)));
        assert_eq!(app.model.selected_task_id(), Some(id));
    }

    #[test]
    fn undo_an_edit_and_an_add() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Draft");
        press(
            &mut app,
            &[KeyCode::Enter, KeyCode::Char('e'), KeyCode::End],
        );
        type_text(&mut app, " v2");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.columns[0].tasks[0].title, "Draft v2");
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(app.model.board.columns[0].tasks[0].title, "Draft");
        assert_eq!(toast_message(&app), "Undid: edit 'Draft v2'");
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(app.model.board.task_count(), 0);
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(toast_message(&app), "Nothing to undo");
    }

    fn titles(app: &App, column: usize) -> Vec<&str> {
        app.model.board.columns[column]
            .tasks
            .iter()
            .map(|task| task.title.as_str())
            .collect()
    }

    #[test]
    fn digits_and_g_jump_between_lanes_and_cards() {
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 0, "First");
        add(&mut app, 0, "Middle");
        let last = add(&mut app, 0, "Last");
        let done = add(&mut app, 2, "Shipped");
        press(&mut app, &[KeyCode::Char('3')]);
        assert_eq!(app.model.ui.active_column, 2);
        assert_eq!(app.model.selected_task_id(), Some(done));
        // A lane that doesn't exist does nothing.
        press(&mut app, &[KeyCode::Char('9')]);
        assert_eq!(app.model.ui.active_column, 2);
        // g + a lane's initial, in any case.
        press(&mut app, &[KeyCode::Char('g'), KeyCode::Char('b')]);
        assert_eq!(app.model.ui.active_column, 0);
        press(&mut app, &[KeyCode::Char('G')]);
        assert_eq!(app.model.selected_task_id(), Some(last));
        press(&mut app, &[KeyCode::Char('g'), KeyCode::Char('g')]);
        assert_eq!(app.model.selected_task_id(), Some(first));
        assert!(app.model.ui.pending.is_none());
        press(&mut app, &[KeyCode::Char('g'), KeyCode::Char('I')]);
        assert_eq!(app.model.ui.active_column, 1);
        press(&mut app, &[KeyCode::Char('g'), KeyCode::Char('x')]);
        assert_eq!(toast_message(&app), "No lane starts with \"x\"");
        // Esc cancels a prefix without doing anything else.
        press(
            &mut app,
            &[KeyCode::Char('g'), KeyCode::Esc, KeyCode::Char('j')],
        );
        assert!(app.model.ui.pending.is_none());
    }

    #[test]
    fn new_tasks_go_below_or_above_the_selection() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "One");
        add(&mut app, 0, "Two");
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Below one");
        press(&mut app, &[KeyCode::Enter, KeyCode::Char('N')]);
        type_text(&mut app, "Above below one");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(
            titles(&app, 0),
            ["One", "Above below one", "Below one", "Two"]
        );
        // In an empty lane, N adds at the top and n at the end.
        press(&mut app, &[KeyCode::Char('l'), KeyCode::Char('n')]);
        type_text(&mut app, "Only");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(titles(&app, 1), ["Only"]);
    }

    #[test]
    fn shift_j_and_k_reorder_within_the_lane() {
        let (_directory, mut app) = test_app();
        let one = add(&mut app, 0, "One");
        add(&mut app, 0, "Two");
        add(&mut app, 0, "Three");
        press(&mut app, &[KeyCode::Char('J'), KeyCode::Char('J')]);
        assert_eq!(titles(&app, 0), ["Two", "Three", "One"]);
        assert_eq!(app.model.selected_task_id(), Some(one));
        // At the end of the lane it stays put.
        press(&mut app, &[KeyCode::Char('J')]);
        assert_eq!(titles(&app, 0), ["Two", "Three", "One"]);
        press(&mut app, &[KeyCode::Char('K')]);
        assert_eq!(titles(&app, 0), ["Two", "One", "Three"]);
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(titles(&app, 0), ["Two", "Three", "One"]);
    }

    #[test]
    fn move_to_menu_moves_a_task_to_any_lane() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Travel");
        press(&mut app, &[KeyCode::Char('m')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::MoveTo { selected: 0, .. })
        ));
        press(
            &mut app,
            &[KeyCode::Char('j'), KeyCode::Char('j'), KeyCode::Enter],
        );
        assert_eq!(app.model.board.task_location(id), Some((2, 0)));
        assert!(app.model.ui.screens.is_empty());
        // A digit picks directly, and q closes without moving.
        press(&mut app, &[KeyCode::Char('m'), KeyCode::Char('2')]);
        assert_eq!(app.model.board.task_location(id), Some((1, 0)));
        press(&mut app, &[KeyCode::Char('m'), KeyCode::Char('q')]);
        assert_eq!(app.model.board.task_location(id), Some((1, 0)));
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn quick_add_stays_open_for_the_next_task() {
        let (_directory, mut app) = test_app();
        add(&mut app, 1, "Existing");
        press(&mut app, &[KeyCode::Char('l'), KeyCode::Char('a')]);
        type_text(&mut app, "First");
        press(&mut app, &[KeyCode::Enter]);
        type_text(&mut app, "Second");
        press(&mut app, &[KeyCode::Enter]);
        assert!(matches!(top_screen(&app), Some(Screen::QuickAdd { .. })));
        assert_eq!(titles(&app, 1), ["Existing", "First", "Second"]);
        // Enter on an empty prompt closes it, as do Esc and Ctrl+C.
        press(&mut app, &[KeyCode::Enter]);
        assert!(app.model.ui.screens.is_empty());
        press(&mut app, &[KeyCode::Char('a')]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            clock(),
        );
        assert!(app.model.ui.screens.is_empty());
        assert!(!app.should_quit());
    }

    #[test]
    fn duplicate_copies_below_and_can_be_undone() {
        let (_directory, mut app) = test_app();
        let original = add(&mut app, 0, "Template");
        add(&mut app, 0, "Other");
        press(&mut app, &[KeyCode::Char('y')]);
        assert_eq!(titles(&app, 0), ["Template", "Template", "Other"]);
        let copy = app.model.board.columns[0].tasks[1].id;
        assert_ne!(copy, original);
        assert_eq!(app.model.selected_task_id(), Some(copy));
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(titles(&app, 0), ["Template", "Other"]);
    }

    #[test]
    fn escape_goes_back_and_q_closes_overlays() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Task");
        press(&mut app, &[KeyCode::Enter, KeyCode::Char('q')]);
        assert!(app.model.ui.screens.is_empty());
        assert!(!app.should_quit());
        press(&mut app, &[KeyCode::Tab, KeyCode::Esc]);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
        press(&mut app, &[KeyCode::Esc, KeyCode::Esc]);
        assert!(!app.should_quit());
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(app.should_quit());
    }

    #[test]
    fn help_lists_the_keys_for_the_screen_it_opened_from() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Task");
        press(&mut app, &[KeyCode::Enter, KeyCode::Char('?')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Help {
                context: crate::command::Context::Detail,
                ..
            })
        ));
    }

    #[test]
    fn the_tip_stays_dismissed() {
        let (directory, app) = test_app();
        let marker = directory.path().join("state").join("tip-dismissed");
        let mut app = app.with_tip_marker(marker.clone());
        assert!(app.model.ui.tip);
        press(&mut app, &[KeyCode::Esc]);
        assert!(!app.model.ui.tip);
        assert!(marker.exists());
        let store = JsonStore::new(directory.path().join("board.json"));
        let app = App::new(store, Theme::mocha(), false)
            .unwrap()
            .with_tip_marker(marker);
        assert!(!app.model.ui.tip);
    }

    fn palette_labels(app: &App) -> Vec<String> {
        match top_screen(app) {
            Some(Screen::Palette(palette)) => palette_entries(&app.model, palette)
                .into_iter()
                .map(|entry| entry.label)
                .collect(),
            other => panic!("expected the palette, got {other:?}"),
        }
    }

    #[test]
    fn the_palette_runs_commands_and_remembers_them() {
        let (_directory, mut app) = test_app();
        let id = add(&mut app, 0, "Travel");
        press(&mut app, &[KeyCode::Char(':')]);
        type_text(&mut app, "move to");
        assert_eq!(palette_labels(&app)[0], "Move to…");
        press(&mut app, &[KeyCode::Enter]);
        assert!(matches!(top_screen(&app), Some(Screen::MoveTo { task, .. }) if *task == id));
        press(&mut app, &[KeyCode::Esc]);
        // Ctrl+K opens it too, with the command just used first.
        app.handle_key(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            clock(),
        );
        assert_eq!(palette_labels(&app)[0], "Move to…");
        // Ctrl+C closes it without quitting.
        app.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            clock(),
        );
        assert!(app.model.ui.screens.is_empty());
        assert!(!app.should_quit());
    }

    #[test]
    fn the_palette_goes_to_lanes_and_tasks() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Alpha");
        let hidden = add(&mut app, 2, "Needle in the haystack");
        // A search that hides the task is cleared to show it.
        press(&mut app, &[KeyCode::Char('/')]);
        type_text(&mut app, "alpha");
        press(&mut app, &[KeyCode::Enter, KeyCode::Char(':')]);
        type_text(&mut app, "needle");
        assert_eq!(
            palette_labels(&app)[0],
            "Go to task: Needle in the haystack"
        );
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.selected_task_id(), Some(hidden));
        assert!(!app.model.ui.search.is_active());
        press(&mut app, &[KeyCode::Char(':')]);
        type_text(&mut app, "lane in progress");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.ui.active_column, 1);
        // ↓ / ↑ move through the entries, wrapping around.
        press(&mut app, &[KeyCode::Char(':'), KeyCode::Up]);
        match top_screen(&app) {
            Some(Screen::Palette(palette)) => {
                let count = palette_entries(&app.model, palette).len();
                assert_eq!(palette.selected, count - 1);
            }
            other => panic!("expected the palette, got {other:?}"),
        }
    }

    #[test]
    fn space_passes_the_next_key_through() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Char(' ')]);
        assert!(app.model.ui.pending.is_some());
        press(&mut app, &[KeyCode::Char('n')]);
        assert!(matches!(top_screen(&app), Some(Screen::Editor(_))));
        press(&mut app, &[KeyCode::Esc, KeyCode::Char(' '), KeyCode::Esc]);
        assert!(app.model.ui.pending.is_none());
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn stray_keys_do_nothing_harmful() {
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 1, "Only task");
        // Backspace with no search doesn't move the selection.
        press(&mut app, &[KeyCode::Char('h'), KeyCode::Backspace]);
        assert_eq!(app.model.ui.active_column, 0);
        // A prefix left pending when the terminal shrinks does nothing.
        press(&mut app, &[KeyCode::Char('g')]);
        app.resize(30, 8);
        press(&mut app, &[KeyCode::Char('i')]);
        assert_eq!(app.model.ui.active_column, 0);
        app.resize(100, 30);
        press(&mut app, &[KeyCode::Char('2')]);
        assert_eq!(app.model.selected_task_id(), Some(first));
    }

    #[test]
    fn a_held_prefix_wakes_the_loop_for_which_key() {
        let (_directory, mut app) = test_app();
        let clock = clock();
        app.handle_key(key(KeyCode::Char('g')), clock);
        let timeout = app.next_timeout(clock.instant);
        assert!(timeout <= crate::ui::WHICH_KEY_DELAY && timeout > Duration::ZERO);
    }

    fn column_names(app: &App) -> Vec<&str> {
        app.model
            .board
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect()
    }

    #[test]
    fn columns_are_managed_from_the_rail() {
        let (_directory, mut app) = test_app();
        // a adds a column after the active one, and focuses it.
        press(&mut app, &[KeyCode::Tab, KeyCode::Char('a')]);
        type_text(&mut app, "Review");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(
            column_names(&app),
            ["Backlog", "Review", "In Progress", "Done"]
        );
        assert_eq!(app.model.ui.active_column, 1);
        assert_eq!(app.model.ui.focus, FocusRegion::Rail);
        // An empty name is refused, and the prompt stays open.
        press(&mut app, &[KeyCode::Char('a'), KeyCode::Enter]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Prompt { error: Some(_), .. })
        ));
        press(&mut app, &[KeyCode::Esc]);
        // r renames, keeping the id.
        press(&mut app, &[KeyCode::Char('r')]);
        app.handle_key(
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            clock(),
        );
        type_text(&mut app, "Code review");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.columns[1].name, "Code review");
        assert_eq!(app.model.board.columns[1].id, "review");
        // J and K reorder, and the column stays active.
        press(&mut app, &[KeyCode::Char('J')]);
        assert_eq!(
            column_names(&app),
            ["Backlog", "In Progress", "Code review", "Done"]
        );
        assert_eq!(app.model.ui.active_column, 2);
        press(&mut app, &[KeyCode::Char('K'), KeyCode::Char('K')]);
        assert_eq!(column_names(&app)[0], "Code review");
        // Every change can be undone.
        press(&mut app, &[KeyCode::Char('u'), KeyCode::Char('u')]);
        assert_eq!(column_names(&app)[2], "Code review");
        app.flush(clock());
        assert_eq!(app.store().load().unwrap().unwrap(), app.model.board);
    }

    #[test]
    fn deleting_a_column_moves_its_tasks_or_deletes_them() {
        let (_directory, mut app) = test_app();
        let task = add(&mut app, 1, "In flight");
        press(
            &mut app,
            &[KeyCode::Tab, KeyCode::Char('j'), KeyCode::Char('d')],
        );
        assert!(matches!(
            top_screen(&app),
            Some(Screen::DeleteColumn {
                column: 1,
                tasks_to: Some(0)
            })
        ));
        // l steps to the next column, then to deleting the tasks, then
        // wraps around.
        press(&mut app, &[KeyCode::Char('l')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::DeleteColumn {
                tasks_to: Some(2),
                ..
            })
        ));
        press(&mut app, &[KeyCode::Char('l')]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::DeleteColumn { tasks_to: None, .. })
        ));
        press(&mut app, &[KeyCode::Char('h'), KeyCode::Char('y')]);
        assert_eq!(column_names(&app), ["Backlog", "Done"]);
        assert_eq!(app.model.board.task_location(task), Some((1, 0)));
        assert_eq!(app.model.ui.active_column, 1);
        assert_eq!(
            toast_message(&app),
            "Deleted In Progress, its tasks moved to Done · u to undo"
        );
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(column_names(&app), ["Backlog", "In Progress", "Done"]);
        assert_eq!(app.model.board.task_location(task), Some((1, 0)));
        // The last column can't be deleted.
        press(&mut app, &[KeyCode::Char('d'), KeyCode::Char('l')]);
        press(&mut app, &[KeyCode::Char('l'), KeyCode::Char('y')]);
        press(&mut app, &[KeyCode::Char('d'), KeyCode::Char('y')]);
        assert_eq!(app.model.board.columns.len(), 1);
        assert!(app.model.board.task(task).is_none());
        press(&mut app, &[KeyCode::Char('d')]);
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn a_column_colour_can_be_picked_or_left_automatic() {
        let (_directory, mut app) = test_app();
        press(&mut app, &[KeyCode::Tab, KeyCode::Char('c')]);
        let names = app.model.ui.theme.accent_names();
        // The current colour, sapphire, is highlighted.
        let sapphire = names.iter().position(|name| *name == "sapphire").unwrap();
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Colors { selected, .. }) if *selected == sapphire + 1
        ));
        press(&mut app, &[KeyCode::Char('j'), KeyCode::Enter]);
        assert_eq!(
            app.model.board.columns[0].color.as_deref(),
            Some(names[sapphire + 1])
        );
        press(&mut app, &[KeyCode::Char('c'), KeyCode::Home]);
        // Home isn't a menu key; go to the top with k instead.
        press(&mut app, &[KeyCode::Char('k'); 20]);
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.columns[0].color, None);
    }

    #[test]
    fn a_full_column_asks_before_taking_another_task() {
        let (_directory, mut app) = test_app();
        let first = add(&mut app, 0, "First");
        let second = add(&mut app, 0, "Second");
        add(&mut app, 1, "Busy");
        // w sets a limit of 1 on In Progress, which is now full.
        press(
            &mut app,
            &[KeyCode::Tab, KeyCode::Char('j'), KeyCode::Char('w')],
        );
        type_text(&mut app, "x");
        press(&mut app, &[KeyCode::Enter]);
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Prompt { error: Some(_), .. })
        ));
        press(&mut app, &[KeyCode::Backspace]);
        type_text(&mut app, "1");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.columns[1].wip_limit, Some(1));
        // Moving a task in asks; n leaves it where it was.
        press(
            &mut app,
            &[KeyCode::Tab, KeyCode::Char('h'), KeyCode::Char('L')],
        );
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmWip { .. })));
        press(&mut app, &[KeyCode::Char('n')]);
        assert_eq!(app.model.board.task_location(first), Some((0, 0)));
        // y moves it anyway.
        press(&mut app, &[KeyCode::Char('L'), KeyCode::Char('y')]);
        assert_eq!(app.model.board.task_location(first), Some((1, 1)));
        assert_eq!(app.model.board.columns[1].wip(), crate::domain::Wip::Over);
        // So does the move-to menu.
        press(
            &mut app,
            &[KeyCode::Char('h'), KeyCode::Char('m'), KeyCode::Char('2')],
        );
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmWip { .. })));
        press(&mut app, &[KeyCode::Esc]);
        assert_eq!(app.model.board.task_location(second), Some((0, 0)));
        // An empty answer removes the limit.
        press(
            &mut app,
            &[KeyCode::Tab, KeyCode::Char('j'), KeyCode::Char('w')],
        );
        app.handle_key(
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
            clock(),
        );
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.columns[1].wip_limit, None);
    }

    #[test]
    fn a_collapsed_lane_hides_its_cards_in_the_board_view() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Todo");
        let done = add(&mut app, 2, "Shipped");
        press(&mut app, &[KeyCode::Char('3')]);
        assert_eq!(app.model.selected_task_id(), Some(done));
        press(&mut app, &[KeyCode::Char('z')]);
        assert!(app.model.board.columns[2].collapsed);
        assert_eq!(app.model.selected_task_id(), None);
        assert_eq!(app.model.ui.active_column, 2);
        // Its tasks don't match searches, and h / l still reach it.
        press(&mut app, &[KeyCode::Char('h'), KeyCode::Char('l')]);
        assert_eq!(app.model.ui.active_column, 2);
        assert!(app.model.visible_task_indices(2).is_empty());
        // All tasks shows every column.
        press(&mut app, &[KeyCode::Char('v')]);
        assert_eq!(app.model.visible_task_indices(2), vec![0]);
        press(&mut app, &[KeyCode::Char('v')]);
        // Adding a task to it expands it, as one change.
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "More");
        press(&mut app, &[KeyCode::Enter, KeyCode::Esc]);
        assert!(!app.model.board.columns[2].collapsed);
        assert_eq!(titles(&app, 2), ["Shipped", "More"]);
        press(&mut app, &[KeyCode::Char('u')]);
        assert!(app.model.board.columns[2].collapsed);
        assert_eq!(titles(&app, 2), ["Shipped"]);
        // Going to one of its tasks from the palette expands it.
        press(&mut app, &[KeyCode::Char('1'), KeyCode::Char(':')]);
        type_text(&mut app, "shipped");
        press(&mut app, &[KeyCode::Enter]);
        assert!(!app.model.board.columns[2].collapsed);
        assert_eq!(app.model.selected_task_id(), Some(done));
    }

    /// Where `text` is on screen, rendering the app as it is now: its
    /// first appearance from the top.
    fn find(app: &App, text: &str) -> (u16, u16) {
        find_from(app, text, false)
    }

    /// The last appearance of `text`, from the bottom, as in the status
    /// line.
    fn find_last(app: &App, text: &str) -> (u16, u16) {
        find_from(app, text, true)
    }

    fn find_from(app: &App, text: &str, from_bottom: bool) -> (u16, u16) {
        let (width, height) = app.model.ui.viewport;
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| ui::render(frame, &app.model, clock()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<u16> = if from_bottom {
            (0..height).rev().collect()
        } else {
            (0..height).collect()
        };
        for y in rows {
            let row: Vec<&str> = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
            let line: String = row.concat();
            if let Some(byte) = line.find(text) {
                return (line[..byte].chars().count() as u16, y);
            }
        }
        panic!("{text:?} is not on screen");
    }

    fn mouse(app: &mut App, kind: MouseEventKind, (x, y): (u16, u16), clock: Clock) {
        use ratatui::crossterm::event::MouseEvent;
        app.dispatch(
            Action::Mouse(MouseEvent {
                kind,
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            }),
            clock,
        );
    }

    fn click(app: &mut App, at: (u16, u16)) {
        use ratatui::crossterm::event::MouseButton;
        mouse(app, MouseEventKind::Down(MouseButton::Left), at, clock());
        mouse(app, MouseEventKind::Up(MouseButton::Left), at, clock());
    }

    fn drag(app: &mut App, from: (u16, u16), to: (u16, u16)) {
        use ratatui::crossterm::event::MouseButton;
        mouse(app, MouseEventKind::Down(MouseButton::Left), from, clock());
        mouse(app, MouseEventKind::Drag(MouseButton::Left), to, clock());
        mouse(app, MouseEventKind::Up(MouseButton::Left), to, clock());
    }

    #[test]
    fn clicks_select_focus_and_open() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "First");
        let second = add(&mut app, 1, "Second card");
        let at = find(&app, "Second card");
        click(&mut app, at);
        assert_eq!(app.model.selected_task_id(), Some(second));
        assert_eq!(app.model.ui.active_column, 1);
        assert!(app.model.ui.screens.is_empty());
        // A second click soon after is a double click, which opens it.
        let at = find(&app, "Second card");
        click(&mut app, at);
        assert!(matches!(top_screen(&app), Some(Screen::Detail { task, .. }) if *task == second));
        // Clicking another card shows it in the drawer.
        let first = app.model.board.columns[0].tasks[0].id;
        let at = find(&app, "First");
        click(&mut app, at);
        assert!(matches!(top_screen(&app), Some(Screen::Detail { task, .. }) if *task == first));
        press(&mut app, &[KeyCode::Esc]);
        // A lane header focuses its lane, a rail entry the rail.
        let at = find_last(&app, "Done");
        click(&mut app, at);
        assert_eq!(app.model.ui.active_column, 2);
        assert_eq!(app.model.ui.focus, FocusRegion::Rail);
        let header = find(&app, "In Progress  1");
        click(&mut app, header);
        assert_eq!(app.model.ui.active_column, 1);
        assert_eq!(app.model.ui.focus, FocusRegion::Cards);
        // The view names in the top bar switch views.
        let at = find(&app, "All tasks");
        click(&mut app, at);
        assert_eq!(app.model.ui.view, ViewMode::AllTasks);
    }

    #[test]
    fn clicking_a_hint_runs_it() {
        let (_directory, mut app) = test_app();
        add(&mut app, 0, "Task");
        let at = find(&app, "n/N new");
        click(&mut app, at);
        assert!(matches!(top_screen(&app), Some(Screen::Editor(_))));
        // Only the status line responds while an overlay is open.
        let at = find_last(&app, "Esc cancel");
        click(&mut app, at);
        assert!(app.model.ui.screens.is_empty());
    }

    #[test]
    fn the_wheel_scrolls_lanes_and_the_drawer() {
        let (_directory, mut app) = test_app();
        app.resize(100, 20);
        let tasks = add_many(&mut app, 10);
        add_many_to(&mut app, 1, 10);
        let lane = find(&app, "Task 0");
        mouse(&mut app, MouseEventKind::ScrollDown, lane, clock());
        assert_eq!(app.model.selected_task_id(), Some(tasks[1]));
        // Another lane scrolls without taking the selection.
        let other = find(&app, "Other 0");
        mouse(&mut app, MouseEventKind::ScrollDown, other, clock());
        assert_eq!(app.model.ui.scroll.lane(1), 1);
        assert_eq!(app.model.selected_task_id(), Some(tasks[1]));
        mouse(&mut app, MouseEventKind::ScrollUp, other, clock());
        assert_eq!(app.model.ui.scroll.lane(1), 0);
        // Over the drawer, it scrolls the description.
        let long: String = (0..60).map(|line| format!("line {line}\n")).collect();
        app.model
            .board
            .update_task(tasks[1], "Long".to_owned(), long, 0)
            .unwrap();
        press(&mut app, &[KeyCode::Enter]);
        let text = find(&app, "line 0");
        mouse(&mut app, MouseEventKind::ScrollDown, text, clock());
        assert!(matches!(
            top_screen(&app),
            Some(Screen::Detail { scroll: 3, .. })
        ));
    }

    fn add_many_to(app: &mut App, column: usize, count: usize) -> Vec<Uuid> {
        (0..count)
            .map(|index| {
                let title = format!("Other {index}");
                app.model.board.add_task(column, &title, "", 0).unwrap()
            })
            .collect()
    }

    #[test]
    fn dragging_a_card_moves_it() {
        let (_directory, mut app) = test_app();
        let moving = add(&mut app, 0, "Moving");
        add(&mut app, 1, "Top");
        add(&mut app, 1, "Bottom");
        // Onto the top half of "Bottom": between the two.
        use ratatui::crossterm::event::MouseButton;
        let from = find(&app, "Moving");
        let to = find(&app, "Bottom");
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            from,
            clock(),
        );
        mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            to,
            clock(),
        );
        let marker = app.model.ui.mouse.drag.and_then(|drag| drag.target);
        assert!(matches!(
            marker,
            Some(crate::app::DropTarget {
                column: 1,
                index: Some(1),
                ..
            })
        ));
        assert!(find(&app, "━━━━").1 == to.1 - 1);
        mouse(&mut app, MouseEventKind::Up(MouseButton::Left), to, clock());
        assert_eq!(titles(&app, 1), ["Top", "Moving", "Bottom"]);
        assert_eq!(app.model.selected_task_id(), Some(moving));
        assert!(app.model.ui.mouse.drag.is_none());
        // Dropping it back on itself changes nothing, not even the undo
        // history.
        let here = find(&app, "Moving");
        drag(&mut app, here, (here.0 + 1, here.1));
        assert_eq!(titles(&app, 1), ["Top", "Moving", "Bottom"]);
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(titles(&app, 0), ["Moving"]);
        // A rail entry drops at the end of its column.
        let here = find(&app, "Moving");
        let to = find_last(&app, "Done");
        drag(&mut app, here, to);
        assert_eq!(titles(&app, 2), ["Moving"]);
    }

    #[test]
    fn dragging_into_a_full_column_asks() {
        let (_directory, mut app) = test_app();
        let moving = add(&mut app, 0, "Moving");
        add(&mut app, 1, "Busy");
        app.model.board.set_wip_limit(1, Some(1)).unwrap();
        let from = find(&app, "Moving");
        let to = find(&app, "Busy");
        drag(&mut app, from, to);
        assert!(matches!(top_screen(&app), Some(Screen::ConfirmWip { .. })));
        press(&mut app, &[KeyCode::Char('y')]);
        assert_eq!(app.model.board.task_location(moving), Some((1, 0)));
    }

    /// An app on `board.json` in a temporary directory, with recent boards
    /// and a personal board there too.
    fn app_with_boards() -> (TempDir, App) {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let recent = Recent::new(directory.path().join("state").join("recent-boards"));
        let personal = boards::named(&directory.path().join("data"), boards::PERSONAL);
        let mut app = App::new(store, Theme::mocha(), false)
            .unwrap()
            .with_boards(Some(recent), Some(personal));
        app.resize(100, 30);
        (directory, app)
    }

    #[test]
    fn the_switcher_opens_another_board_after_saving_this_one() {
        let (directory, mut app) = app_with_boards();
        let other = directory.path().join("other.json");
        let mut board = Board {
            name: "Other".to_owned(),
            ..Board::default()
        };
        board.add_task(0, "Theirs", "", 0).unwrap();
        JsonStore::new(&other).save(&board).unwrap();
        Recent::new(directory.path().join("state").join("recent-boards")).record(&other);

        add(&mut app, 0, "Mine");
        press(&mut app, &[KeyCode::Char('L'), KeyCode::Char('b')]);
        let Some(Screen::Boards { entries, selected }) = top_screen(&app) else {
            panic!("expected the switcher");
        };
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, ["Project Board", "Other", "Personal"]);
        assert_eq!(*selected, 0);
        assert!(entries[0].current);
        assert!(entries[1].detail.starts_with("1 task · "));
        assert!(entries[2].detail.starts_with("new · "));
        press(&mut app, &[KeyCode::Char('j'), KeyCode::Enter]);
        assert_eq!(app.model.board.name, "Other");
        assert_eq!(app.store().path(), other);
        assert!(app.model.ui.screens.is_empty());
        assert!(!app.model.session.history.can_undo());
        assert_eq!(toast_message(&app), "Opened Other");
        // The board left behind was saved first.
        let mine = JsonStore::new(directory.path().join("board.json"))
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(mine.columns[1].tasks[0].title, "Mine");
        // Opening a board makes it the most recent.
        let recent = Recent::new(directory.path().join("state").join("recent-boards")).load();
        assert_eq!(recent[0], boards::absolute(&other));
        // Changes are saved to the new board.
        press(&mut app, &[KeyCode::Char('d'), KeyCode::Char('y')]);
        app.flush(clock());
        assert_eq!(
            JsonStore::new(&other).load().unwrap().unwrap().task_count(),
            0
        );
    }

    #[test]
    fn the_switcher_stays_when_this_board_cannot_be_saved() {
        let (directory, mut app) = app_with_boards();
        std::fs::create_dir(directory.path().join("board.json")).unwrap();
        add(&mut app, 0, "Unsaved");
        press(
            &mut app,
            &[KeyCode::Char('L'), KeyCode::Char('b'), KeyCode::Char('j')],
        );
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.name, "Project Board");
        assert!(
            toast_message(&app).starts_with("Can't switch boards"),
            "{}",
            toast_message(&app)
        );
    }

    #[test]
    fn with_no_board_found_it_asks_where_to_make_one() {
        let (directory, app) = app_with_boards();
        let mut app = app.ask_for_board();
        assert!(matches!(top_screen(&app), Some(Screen::NoBoard { .. })));
        // Nothing is written until the user decides.
        app.flush(clock());
        assert!(!directory.path().join("board.json").exists());
        press(&mut app, &[KeyCode::Char('c')]);
        assert!(app.model.ui.screens.is_empty());
        app.flush(clock());
        assert!(directory.path().join("board.json").exists());

        // Or the personal board, which is new, so it starts empty.
        let (directory, app) = app_with_boards();
        let mut app = app.ask_for_board();
        press(&mut app, &[KeyCode::Char('p')]);
        let personal = boards::named(&directory.path().join("data"), boards::PERSONAL);
        assert_eq!(app.store().path(), personal);
        assert!(app.model.ui.screens.is_empty());
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "First");
        press(&mut app, &[KeyCode::Enter, KeyCode::Esc]);
        app.flush(clock());
        assert_eq!(
            JsonStore::new(&personal)
                .load()
                .unwrap()
                .unwrap()
                .task_count(),
            1
        );
        assert!(!directory.path().join("board.json").exists());

        // q quits without creating anything.
        let (directory, app) = app_with_boards();
        let mut app = app.ask_for_board();
        press(&mut app, &[KeyCode::Char('q')]);
        assert!(app.should_quit());
        assert!(!directory.path().join("board.json").exists());
    }

    #[test]
    fn new_boards_follow_the_settings() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("website");
        std::fs::create_dir(&project).unwrap();
        let settings = Settings {
            columns: vec!["Ideas".to_owned(), "Doing".to_owned()],
            name_new_boards: true,
            date_format: DateFormat::Pattern("%Y-%m-%d".to_owned()),
            ..Settings::new(Theme::mocha(), false)
        };
        let app =
            App::with_settings(JsonStore::new(project.join(boards::FILE_NAME)), settings).unwrap();
        assert_eq!(app.model.board.name, "website");
        let names: Vec<&str> = app
            .model
            .board
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect();
        assert_eq!(names, ["Ideas", "Doing"]);
        assert_eq!(
            app.model.ui.date_format,
            DateFormat::Pattern("%Y-%m-%d".to_owned())
        );
    }

    #[test]
    fn update_is_pure_and_returns_effects() {
        let mut model = Model::new(Default::default(), Theme::mocha(), false);
        model.ui.viewport = (100, 30);
        let effects = update(
            &mut model,
            Action::Command(crate::command::CommandId::NewTask),
            clock(),
        );
        assert!(effects.is_empty());
        for character in "Pure".chars() {
            update(
                &mut model,
                Action::Edit(key(KeyCode::Char(character))),
                clock(),
            );
        }
        let effects = update(
            &mut model,
            Action::Command(crate::command::CommandId::SaveTask),
            clock(),
        );
        assert_eq!(effects, vec![Effect::Save]);
        assert_eq!(model.board.columns[0].tasks[0].title, "Pure");
        let effects = update(
            &mut model,
            Action::Command(crate::command::CommandId::Quit),
            clock(),
        );
        assert_eq!(effects, vec![Effect::Quit]);
    }
}
