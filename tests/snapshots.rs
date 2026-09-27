//! Snapshot tests for every screen, recorded with a fixed clock, fixed task
//! ids and fixed timestamps so the output is stable.
//!
//! Update snapshots with `INSTA_UPDATE=always cargo test --test snapshots`
//! (or `cargo insta review`) and review the diff like any other change.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tempfile::TempDir;
use tui_kanban::app::{App, FocusRegion, Mode, Toast, ToastKind, ViewMode};
use tui_kanban::clock::Clock;
use tui_kanban::domain::{Board, Column, SCHEMA_VERSION, Task};
use tui_kanban::storage::JsonStore;
use tui_kanban::theme::Theme;
use uuid::Uuid;

/// 2026-08-29 10:40 UTC; every timestamp below is relative to this.
const NOW: i64 = 1_788_000_000_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

fn clock() -> Clock {
    Clock::fixed(NOW)
}

fn task(n: u128, title: &str, description: &str, age: i64) -> Task {
    Task {
        id: Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0000 | n),
        title: title.to_owned(),
        description: description.to_owned(),
        created_at: NOW - age - DAY,
        updated_at: NOW - age,
    }
}

fn column(id: &str, name: &str, tasks: Vec<Task>) -> Column {
    let mut column = Column::new(id, name);
    column.tasks = tasks;
    column
}

fn board(columns: Vec<Column>) -> Board {
    Board {
        schema_version: SCHEMA_VERSION,
        name: "Snapshot Board".to_owned(),
        columns,
    }
}

fn typical_board() -> Board {
    board(vec![
        column(
            "backlog",
            "Backlog",
            vec![
                task(
                    1,
                    "Write the README",
                    "Cover install, usage, keys and the file format.",
                    26 * HOUR,
                ),
                task(2, "Record a demo", "Commit the .tape file too.", 3 * DAY),
                task(3, "Package release binaries", "", 9 * DAY),
            ],
        ),
        column(
            "in-progress",
            "In Progress",
            vec![
                task(
                    4,
                    "Calmer card style",
                    "Tiles with an accent bar.",
                    12 * MINUTE,
                ),
                task(
                    5,
                    "Command table",
                    "Every keybinding defined once.",
                    95 * MINUTE,
                ),
            ],
        ),
        column(
            "done",
            "Done",
            vec![
                task(6, "Set up CI", "fmt, clippy and tests.", 5 * HOUR),
                task(7, "Snapshot tests", "", 30 * HOUR),
            ],
        ),
    ])
}

fn long_text_board() -> Board {
    board(vec![
        column(
            "backlog",
            "A column with a very long name that will not fit",
            vec![
                task(
                    1,
                    "A very long task title that keeps going well past the width of any card on screen",
                    "  - an indented list item\n  - and another one that is also long enough to wrap",
                    2 * MINUTE,
                ),
                task(
                    2,
                    "日本語のタスク名と絵文字 🦀🚀 が混ざったタイトル",
                    "全角文字の説明文です。幅の計算を確かめます。",
                    3 * HOUR,
                ),
            ],
        ),
        column("done", "Done", vec![task(3, "exact", "", DAY)]),
    ])
}

fn many_columns_board() -> Board {
    board(
        (1..=12)
            .map(|n| {
                column(
                    &format!("col-{n}"),
                    &format!("Stage {n}"),
                    vec![task(n as u128, &format!("Task in stage {n}"), "", HOUR)],
                )
            })
            .collect(),
    )
}

struct Harness {
    _directory: TempDir,
    app: App,
}

impl Harness {
    fn new(board: Board) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        store.save(&board).unwrap();
        let mut app = App::new(store, false, clock()).unwrap();
        // Tests must not depend on whether NO_COLOR is set in the environment.
        app.theme = Theme::mocha();
        Self {
            _directory: directory,
            app,
        }
    }

    fn keys(&mut self, keys: &[KeyCode]) -> &mut Self {
        for code in keys {
            self.app
                .handle_key(KeyEvent::new(*code, KeyModifiers::NONE), clock());
        }
        self
    }

    fn type_text(&mut self, text: &str) -> &mut Self {
        let keys: Vec<KeyCode> = text.chars().map(KeyCode::Char).collect();
        self.keys(&keys)
    }

    fn render(&self, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| tui_kanban::ui::render(frame, &self.app, clock()))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn screen(&self, width: u16, height: u16) -> String {
        buffer_text(&self.render(width, height))
    }
}

/// The buffer's symbols, one line per row, with trailing spaces trimmed so
/// snapshots stay readable.
fn buffer_text(buffer: &Buffer) -> String {
    let area = buffer.area;
    let mut lines = Vec::with_capacity(area.height as usize);
    for y in 0..area.height {
        let mut line = String::new();
        let mut skip = 0;
        for x in 0..area.width {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = buffer[(x, y)].symbol();
            skip = unicode_width(symbol).saturating_sub(1);
            line.push_str(symbol);
        }
        lines.push(line.trim_end().to_owned());
    }
    lines.join("\n")
}

fn unicode_width(symbol: &str) -> usize {
    ratatui::text::Span::raw(symbol).width()
}

macro_rules! screen_snapshot {
    ($name:expr, $harness:expr, $width:expr, $height:expr) => {
        insta::assert_snapshot!($name, $harness.screen($width, $height));
    };
}

const SIZES: [(u16, u16); 3] = [(60, 20), (100, 30), (160, 45)];

#[test]
fn board_view_at_each_size() {
    let harness = Harness::new(typical_board());
    for (width, height) in SIZES {
        screen_snapshot!(format!("board_{width}x{height}"), harness, width, height);
    }
}

#[test]
fn all_tasks_view_at_each_size() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('v')]);
    assert_eq!(harness.app.view_mode, ViewMode::AllTasks);
    for (width, height) in SIZES {
        screen_snapshot!(
            format!("all_tasks_{width}x{height}"),
            harness,
            width,
            height
        );
    }
}

#[test]
fn rail_focused() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Tab, KeyCode::Char('j')]);
    assert_eq!(harness.app.focus, FocusRegion::Rail);
    screen_snapshot!("rail_focused_100x30", harness, 100, 30);
}

#[test]
fn detail_drawer() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Enter]);
    screen_snapshot!("detail_100x30", harness, 100, 30);
    screen_snapshot!("detail_60x20", harness, 60, 20);
}

#[test]
fn new_task_editor() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('n')]);
    screen_snapshot!("editor_new_100x30", harness, 100, 30);
    // Title left empty, so its placeholder shows.
    harness.keys(&[KeyCode::Tab]);
    screen_snapshot!("editor_new_description_focused_100x30", harness, 100, 30);
    harness.type_text("Some details");
    screen_snapshot!("editor_new_typed_60x20", harness, 60, 20);
}

#[test]
fn edit_task_editor() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('e')]);
    screen_snapshot!("editor_edit_100x30", harness, 100, 30);
}

#[test]
fn help_overlay() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('?')]);
    screen_snapshot!("help_100x30", harness, 100, 30);
    screen_snapshot!("help_100x40", harness, 100, 40);
    // Too short for all of it: the overlay scrolls.
    screen_snapshot!("help_60x20", harness, 60, 20);
    harness.keys(&[KeyCode::Char('j'); 30]);
    screen_snapshot!("help_60x20_scrolled_to_end", harness, 60, 20);
    harness.keys(&[KeyCode::Char('x')]);
    assert!(matches!(harness.app.mode, Mode::Dashboard));
}

#[test]
fn delete_confirmation() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('d')]);
    screen_snapshot!("confirm_delete_100x30", harness, 100, 30);
}

#[test]
fn search_active() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('/')]).type_text("ca");
    screen_snapshot!("search_active_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Enter]);
    screen_snapshot!("search_applied_100x30", harness, 100, 30);
}

#[test]
fn toast_showing() {
    let mut harness = Harness::new(typical_board());
    harness.app.toast = Some(Toast {
        message: "Task saved".to_owned(),
        kind: ToastKind::Success,
        expires_at: clock().instant + Duration::from_secs(3),
    });
    screen_snapshot!("toast_100x30", harness, 100, 30);
    screen_snapshot!("toast_60x20", harness, 60, 20);
}

#[test]
fn empty_board() {
    let harness = Harness::new(Board::default());
    screen_snapshot!("empty_100x30", harness, 100, 30);
    screen_snapshot!("empty_60x20", harness, 60, 20);
}

#[test]
fn twelve_columns() {
    let harness = Harness::new(many_columns_board());
    screen_snapshot!("twelve_columns_160x45", harness, 160, 45);
    screen_snapshot!("twelve_columns_100x30", harness, 100, 30);
}

#[test]
fn long_and_wide_character_text() {
    let mut harness = Harness::new(long_text_board());
    screen_snapshot!("long_text_board_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Char('j'), KeyCode::Enter]);
    screen_snapshot!("long_text_detail_100x30", harness, 100, 30);
}

#[test]
fn tiny_terminal() {
    let harness = Harness::new(typical_board());
    screen_snapshot!("tiny_30x8", harness, 30, 8);
}

/// NO_COLOR is recorded with styles, since colour is the point of the test.
#[test]
fn no_color_uses_no_colours() {
    let mut harness = Harness::new(typical_board());
    harness.app.theme = Theme::monochrome();
    insta::assert_debug_snapshot!("no_color_60x20", harness.render(60, 20));
}

// Key-sequence tests: drive the app with key presses and check both the
// resulting board and the screen.

#[test]
fn keys_create_a_task_and_move_it() {
    let mut harness = Harness::new(typical_board());
    harness
        .keys(&[KeyCode::Char('n')])
        .type_text("Ship it")
        .keys(&[KeyCode::Tab])
        .type_text("Before Friday")
        .keys(&[KeyCode::Enter, KeyCode::Char('L')]);

    let board = &harness.app.board;
    assert_eq!(board.task_count(), 8);
    let created = board.columns[1].tasks.last().unwrap();
    assert_eq!(created.title, "Ship it");
    assert_eq!(created.description, "Before Friday");
    assert_eq!(created.created_at, NOW);
    assert_eq!(harness.app.selected_task_id(), Some(created.id));

    // The change was saved, too.
    let saved = harness.app.store.load().unwrap().unwrap();
    assert_eq!(&saved, board);
    screen_snapshot!("keys_create_and_move_100x30", harness, 100, 30);
}

#[test]
fn keys_delete_after_confirmation() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('j'), KeyCode::Char('d'), KeyCode::Char('y')]);
    let titles: Vec<&str> = harness.app.board.columns[0]
        .tasks
        .iter()
        .map(|task| task.title.as_str())
        .collect();
    assert_eq!(titles, ["Write the README", "Package release binaries"]);
    screen_snapshot!("keys_delete_100x30", harness, 100, 30);
}
