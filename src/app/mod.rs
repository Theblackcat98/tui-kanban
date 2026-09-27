//! The app, structured as model → action → update → view:
//!
//! ```text
//! Event (key, resize, tick)
//!   → keymap(&Model, key) -> Action        (action.rs, via the command table)
//!   → update(&mut Model, Action) -> Effects (update.rs, pure)
//!   → the runtime runs the effects          (this file: save, quit)
//!   → ui::render(&Model, Clock)             (pure)
//! ```

mod action;
mod editor;
mod history;
mod input;
mod model;
mod update;

pub use action::{Action, Effect, ExternalTarget, keymap};
pub use editor::{EditorField, EditorState, Placement};
pub use history::History;
pub use input::TextInput;
pub use model::{
    FocusRegion, Model, SaveState, Screen, Scroll, Search, Session, Toast, ToastKind, Ui, ViewMode,
    VisibleTask,
};
pub use update::update;

use anyhow::{Context as _, Result};
use clap::Parser;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEvent,
};
use ratatui::crossterm::execute;
use std::collections::VecDeque;
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::clock::Clock;
use crate::storage::JsonStore;
use crate::theme::{self, Theme, reduce_motion};
use crate::ui;

/// How often the idle event loop wakes to refresh relative times.
const IDLE_REFRESH: Duration = Duration::from_secs(1);

#[derive(Debug, Parser)]
#[command(name = "tui-kanban", about = "A beautiful terminal Kanban board")]
pub struct Cli {
    #[arg(long, default_value = ".tui-kanban.json")]
    pub board: PathBuf,
    /// The colour theme: auto (Latte on light terminals, Mocha on dark
    /// ones), latte, frappe, macchiato, mocha, ansi, the name of a theme in
    /// ~/.config/tui-kanban/themes, or a path to a .toml theme file.
    #[arg(long, value_name = "NAME")]
    pub theme: Option<String>,
    /// Turn off animations (a non-empty REDUCE_MOTION does the same).
    #[arg(long)]
    pub no_animation: bool,
}

/// The runtime: the model plus the store it is saved to. It turns events
/// into actions, runs [`update`], and carries out the effects.
pub struct App {
    pub model: Model,
    pub store: JsonStore,
    /// Created when the first-run tip is dismissed, so it stays dismissed.
    /// Without one, the tip isn't shown.
    tip_marker: Option<PathBuf>,
    /// Text waiting to be edited in `$EDITOR`, which needs the terminal.
    external_edit: Option<(String, ExternalTarget)>,
    quit: bool,
}

impl App {
    pub fn new(store: JsonStore, theme: Theme, animations_enabled: bool) -> Result<Self> {
        let board = store
            .load_or_default()
            .with_context(|| format!("could not load {}", store.path().display()))?;
        Ok(Self {
            model: Model::new(board, theme, animations_enabled),
            store,
            tip_marker: None,
            external_edit: None,
            quit: false,
        })
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

    /// Runs an action through `update`, then carries out its effects. A
    /// save's result is fed back in as another action.
    pub fn dispatch(&mut self, action: Action, clock: Clock) {
        let mut queue = VecDeque::from([action]);
        while let Some(action) = queue.pop_front() {
            for effect in update(&mut self.model, action, clock) {
                match effect {
                    Effect::Save => {
                        let result = self
                            .store
                            .save(&self.model.board)
                            .map_err(|error| error.to_string());
                        queue.push_back(Action::SaveFinished(result));
                    }
                    Effect::Quit => self.quit = true,
                    Effect::EditExternally { text, target } => {
                        self.external_edit = Some((text, target));
                    }
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

    /// Text the runtime should open in `$EDITOR`, if any.
    pub fn take_external_edit(&mut self) -> Option<(String, ExternalTarget)> {
        self.external_edit.take()
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Before exiting, tries once more to save a board whose last save
    /// failed, and reports the error if that fails too.
    pub fn finish(&mut self) -> Result<()> {
        if let SaveState::Failed(_) = self.model.session.save_state {
            self.store.save(&self.model.board).with_context(|| {
                format!(
                    "your last changes could not be saved to {}",
                    self.store.path().display()
                )
            })?;
        }
        Ok(())
    }

    /// How long the event loop may wait for input before it has to redraw:
    /// the next animation frame, the moment a toast expires, or an idle
    /// refresh so relative times such as "5m" stay current.
    pub fn next_timeout(&self, now: Instant) -> Duration {
        let mut timeout = IDLE_REFRESH;
        if let Some(frame) = self.model.ui.animations.next_frame_timeout(now) {
            timeout = timeout.min(frame);
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = JsonStore::new(cli.board);
    let animations = !cli.no_animation && !reduce_motion(env::var_os("REDUCE_MOTION").as_deref());
    let theme = theme::resolve(
        cli.theme.as_deref(),
        &theme::Environment::from_env(),
        theme::detect_light_background,
    )?;
    let mut app = App::new(store, theme, animations)?;
    if let Some(state) = crate::paths::state_dir() {
        app = app.with_tip_marker(state.join("tip-dismissed"));
    }
    let mut terminal = init_terminal()?;
    if let Ok(size) = terminal.size() {
        app.resize(size.width, size.height);
    }
    let result = run_loop(&mut terminal, &mut app);
    let restore_result = restore_terminal();
    result.and(restore_result).and(app.finish())
}

/// Enters the alternate screen and raw mode, with bracketed paste so a
/// multi-line paste arrives as one piece of text rather than as keys.
fn init_terminal() -> Result<DefaultTerminal> {
    let terminal = ratatui::try_init().context("could not initialize terminal")?;
    // Terminals without bracketed paste ignore the request.
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    Ok(terminal)
}

/// Leaves the TUI while the user edits `text` in their own editor, then
/// comes back to it.
fn edit_externally(
    terminal: &mut DefaultTerminal,
    text: &str,
) -> Result<std::result::Result<String, String>> {
    restore_terminal()?;
    let result = crate::tui::external::edit(text);
    *terminal = init_terminal()?;
    terminal.clear()?;
    Ok(result)
}

fn restore_terminal() -> Result<()> {
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::try_restore().context("could not restore terminal")
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    let mut needs_redraw = true;
    loop {
        // Tick on every iteration, not only when polling times out, so
        // toasts expire and animations finish while keys keep arriving.
        let clock = Clock::now();
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
                    if let Some((text, target)) = app.take_external_edit() {
                        let result = edit_externally(terminal, &text)?;
                        app.dispatch(
                            Action::ExternalEditFinished { target, result },
                            Clock::now(),
                        );
                    }
                    needs_redraw = true;
                }
                Event::Paste(text) => {
                    app.dispatch(Action::Paste(text), Clock::now());
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
    use crate::animation::{AnimationKind, FRAME_INTERVAL};
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
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
        assert_eq!(app.store.load().unwrap().unwrap(), app.model.board);
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
    fn a_failed_save_keeps_the_change_and_reports_it() {
        let directory = tempfile::tempdir().unwrap();
        // A directory where the board file should be makes every save fail.
        let path = directory.path().join("board.json");
        std::fs::create_dir(&path).unwrap();
        let store = JsonStore::new(&path);
        let mut app = App {
            model: Model::new(Default::default(), Theme::mocha(), false),
            store,
            tip_marker: None,
            external_edit: None,
            quit: false,
        };
        app.resize(100, 30);
        press(&mut app, &[KeyCode::Char('n')]);
        type_text(&mut app, "Unsaved");
        press(&mut app, &[KeyCode::Enter]);
        assert_eq!(app.model.board.task_count(), 1);
        assert!(matches!(app.model.session.save_state, SaveState::Failed(_)));
        assert_eq!(
            app.model.session.toast.as_ref().map(|toast| toast.kind),
            Some(ToastKind::Error)
        );
        assert!(app.finish().is_err());
        // The error stays up until the next key press.
        app.tick(clock().advance(Duration::from_secs(60)));
        assert!(app.model.session.toast.is_some());
        press(&mut app, &[KeyCode::Char('j')]);
        assert!(app.model.session.toast.is_none());
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
        assert_eq!(app.store.load().unwrap().unwrap(), app.model.board);
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
