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
use tui_kanban::app::{Action, App, FocusRegion, SaveState, Toast, ToastKind, ViewMode};
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
        extra: Default::default(),
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
        extra: Default::default(),
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
        // Tests must not depend on whether NO_COLOR is set in the environment.
        let mut app = App::new(store, Theme::mocha(), false).unwrap();
        app.resize(100, 30);
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

    fn render(&mut self, width: u16, height: u16) -> Buffer {
        self.app.resize(width, height);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| tui_kanban::ui::render(frame, &self.app.model, clock()))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn screen(&mut self, width: u16, height: u16) -> String {
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
    let mut harness = Harness::new(typical_board());
    for (width, height) in SIZES {
        screen_snapshot!(format!("board_{width}x{height}"), harness, width, height);
    }
}

#[test]
fn all_tasks_view_at_each_size() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('v')]);
    assert_eq!(harness.app.model.ui.view, ViewMode::AllTasks);
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
    assert_eq!(harness.app.model.ui.focus, FocusRegion::Rail);
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
    assert!(harness.app.model.ui.screens.is_empty());
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
    harness.app.model.session.toast = Some(Toast {
        message: "Task saved".to_owned(),
        kind: ToastKind::Success,
        expires_at: Some(clock().instant + Duration::from_secs(3)),
    });
    screen_snapshot!("toast_100x30", harness, 100, 30);
    screen_snapshot!("toast_60x20", harness, 60, 20);
}

#[test]
fn empty_board() {
    let mut harness = Harness::new(Board::default());
    screen_snapshot!("empty_100x30", harness, 100, 30);
    screen_snapshot!("empty_60x20", harness, 60, 20);
}

#[test]
fn twelve_columns() {
    let mut harness = Harness::new(many_columns_board());
    screen_snapshot!("twelve_columns_160x45", harness, 160, 45);
    screen_snapshot!("twelve_columns_100x30", harness, 100, 30);
    // #25: the lanes scroll sideways to follow the active column.
    harness.keys(&[KeyCode::Char('l'); 5]);
    screen_snapshot!("twelve_columns_scrolled_100x30", harness, 100, 30);
    // #24: a narrow terminal shows one lane, headed by a tab strip.
    screen_snapshot!("twelve_columns_tabs_60x20", harness, 60, 20);
}

#[test]
fn long_and_wide_character_text() {
    let mut harness = Harness::new(long_text_board());
    screen_snapshot!("long_text_board_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Char('j'), KeyCode::Enter]);
    screen_snapshot!("long_text_detail_100x30", harness, 100, 30);
}

fn nine_tasks_board() -> Board {
    board(vec![column(
        "backlog",
        "Backlog",
        (1..=9)
            .map(|n| {
                task(
                    n,
                    &format!("Task {n}"),
                    if n % 2 == 0 { "With a description" } else { "" },
                    n as i64 * HOUR,
                )
            })
            .collect(),
    )])
}

/// Long lists scroll by whole cards and say how many are hidden (#14, #31).
#[test]
fn scrolled_lists() {
    let mut harness = Harness::new(nine_tasks_board());
    harness.keys(&[KeyCode::Char('j'); 5]);
    screen_snapshot!("scrolled_lane_60x20", harness, 60, 20);
    harness.keys(&[KeyCode::Char('v')]);
    screen_snapshot!("scrolled_all_tasks_60x14", harness, 60, 14);
}

#[test]
fn detail_with_a_long_description() {
    let mut board = typical_board();
    board.columns[0].tasks[0].description = (1..=40)
        .map(|n| format!("  - line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut harness = Harness::new(board);
    harness.keys(&[KeyCode::Enter, KeyCode::Char('j'), KeyCode::Char('j')]);
    screen_snapshot!("detail_scrolled_100x30", harness, 100, 30);
    screen_snapshot!("detail_pinned_160x45", harness, 160, 45);
}

#[test]
fn tiny_terminal() {
    let mut harness = Harness::new(typical_board());
    screen_snapshot!("tiny_30x8", harness, 30, 8);
}

/// NO_COLOR is recorded with styles, since colour is the point of the test.
#[test]
fn no_color_uses_no_colours() {
    let mut harness = Harness::new(typical_board());
    harness.app.model.ui.theme = Theme::monochrome();
    insta::assert_debug_snapshot!("no_color_60x20", harness.render(60, 20));
}

/// Every built-in theme draws the page in its own colours.
#[test]
fn every_theme_renders() {
    for name in ["latte", "frappe", "macchiato", "mocha", "ansi"] {
        let mut harness = Harness::new(typical_board());
        let theme = Theme::built_in(name).unwrap();
        harness.app.model.ui.theme = theme.clone();
        let buffer = harness.render(100, 30);
        // A cell in the lane area, and the top bar.
        assert_eq!(buffer[(60, 20)].bg, theme.bg, "{name}");
        assert_eq!(buffer[(0, 0)].bg, theme.panel, "{name}");
    }
}

/// The colour of the underline below the lane headed `name`.
fn underline_colour(buffer: &Buffer, name: &str) -> ratatui::style::Color {
    let row: String = (0..buffer.area.width)
        .map(|x| buffer[(x, 2)].symbol().to_owned())
        .collect();
    let x = row
        .find(name)
        .unwrap_or_else(|| panic!("no lane {name}: {row}"));
    buffer[(x as u16, 3)].fg
}

/// #26: a column keeps its colour when columns move, and can set its own.
#[test]
fn column_colours_follow_the_column() {
    let mut columns = vec![
        column("alpha", "Alpha", vec![]),
        column("beta", "Beta", vec![]),
        column("gamma", "Gamma", vec![]),
    ];
    columns[1].color = Some("#ff8000".to_owned());
    // With the rail focused, no lane is focused, so every underline is
    // dimmed the same way.
    let mut harness = Harness::new(board(columns.clone()));
    harness.keys(&[KeyCode::Tab]);
    let buffer = harness.render(100, 30);
    let alpha = underline_colour(&buffer, "Alpha");
    let gamma = underline_colour(&buffer, "Gamma");
    assert_ne!(alpha, gamma);
    let ratatui::style::Color::Rgb(r, g, b) = underline_colour(&buffer, "Beta") else {
        panic!("expected an RGB colour");
    };
    assert!(r > g && g > b, "Beta should be orange: {r} {g} {b}");

    columns.reverse();
    let mut harness = Harness::new(board(columns));
    harness.keys(&[KeyCode::Tab]);
    let buffer = harness.render(100, 30);
    assert_eq!(underline_colour(&buffer, "Alpha"), alpha);
    assert_eq!(underline_colour(&buffer, "Gamma"), gamma);
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
        .type_text("Before Friday");
    harness.app.handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        clock(),
    );
    harness.keys(&[KeyCode::Char('L')]);
    harness.app.flush(clock());

    let board = &harness.app.model.board;
    assert_eq!(board.task_count(), 8);
    let created = board.columns[1].tasks.last().unwrap();
    assert_eq!(created.title, "Ship it");
    assert_eq!(created.description, "Before Friday");
    assert_eq!(created.created_at, NOW);
    assert_eq!(harness.app.model.selected_task_id(), Some(created.id));

    // The change was saved, too.
    let saved = harness.app.store().load().unwrap().unwrap();
    assert_eq!(&saved, board);
    screen_snapshot!("keys_create_and_move_100x30", harness, 100, 30);
}

#[test]
fn keys_delete_after_confirmation() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('j'), KeyCode::Char('d'), KeyCode::Char('y')]);
    let titles: Vec<&str> = harness.app.model.board.columns[0]
        .tasks
        .iter()
        .map(|task| task.title.as_str())
        .collect();
    assert_eq!(titles, ["Write the README", "Package release binaries"]);
    harness.app.flush(clock());
    screen_snapshot!("keys_delete_100x30", harness, 100, 30);
}

// Saving (#50, #18, #19)

#[test]
fn saving_status_and_dialogs() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('L')]);
    assert_eq!(harness.app.model.session.save_state, SaveState::Saving);
    screen_snapshot!("saving_100x30", harness, 100, 30);
    // Another program changed the file.
    harness.app.dispatch(Action::Conflict, clock());
    screen_snapshot!("conflict_100x30", harness, 100, 30);
    harness.app.dispatch(Action::FinishQuit, clock());
    screen_snapshot!("confirm_quit_100x30", harness, 100, 30);
    screen_snapshot!("confirm_quit_60x20", harness, 60, 20);
}

// Navigation (#45)

#[test]
fn move_to_menu() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('m'), KeyCode::Char('j')]);
    screen_snapshot!("move_to_100x30", harness, 100, 30);
}

#[test]
fn quick_add_prompt() {
    let mut harness = Harness::new(typical_board());
    harness
        .keys(&[KeyCode::Char('l'), KeyCode::Char('a')])
        .type_text("Review the diff");
    screen_snapshot!("quick_add_100x30", harness, 100, 30);
}

#[test]
fn first_run_tip() {
    let mut harness = Harness::new(typical_board());
    harness.app.model.ui.tip = true;
    screen_snapshot!("tip_100x30", harness, 100, 30);
    screen_snapshot!("tip_60x20", harness, 60, 20);
}

#[test]
fn help_for_the_detail_drawer() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Enter, KeyCode::Char('?')]);
    screen_snapshot!("help_detail_100x30", harness, 100, 30);
}

#[test]
fn discard_confirmation() {
    let mut harness = Harness::new(typical_board());
    harness
        .keys(&[KeyCode::Char('e'), KeyCode::End])
        .type_text(" now")
        .keys(&[KeyCode::Esc]);
    screen_snapshot!("confirm_discard_100x30", harness, 100, 30);
}

#[test]
fn markdown_description_with_a_checklist() {
    let mut board = typical_board();
    board.columns[0].tasks[0].description = "## Before the release\n\
        Write it for **new users** first, then *everyone*.\n\n\
        - [x] Install with `cargo install`\n\
        - [ ] Usage, with a recorded demo that shows the board and the palette\n\
        - [ ] Keys\n\n\
        > Keep it short.\n\n\
        ```\ncargo run -- --board demo.json\n```"
        .to_owned();
    let mut harness = Harness::new(board);
    screen_snapshot!("markdown_card_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Enter, KeyCode::Tab]);
    screen_snapshot!("markdown_detail_100x30", harness, 100, 30);
}

// Search (#47, #28)

#[test]
fn search_with_filter_terms() {
    let mut harness = Harness::new(typical_board());
    harness
        .keys(&[KeyCode::Char('/')])
        .type_text("in:progress updated:<1d cmd");
    screen_snapshot!("search_terms_typing_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Enter]);
    screen_snapshot!("search_terms_applied_100x30", harness, 100, 30);
    screen_snapshot!("search_terms_applied_60x20", harness, 60, 20);
}

#[test]
fn search_highlights_matched_characters() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char('/')]).type_text("rdme");
    let buffer = harness.render(100, 30);
    let accent = harness.app.model.ui.theme.accent;
    // "Write the README" in the first lane: R, M and E are matched.
    let row: String = (0..100)
        .map(|x| buffer[(x, 6)].symbol().to_owned())
        .collect();
    let byte = row.find("Write the README").expect("the card is shown");
    let start = row[..byte].chars().count() as u16;
    let colours: Vec<bool> = (0..16)
        .map(|x| buffer[(start + x, 6)].fg == accent)
        .collect();
    let expected: Vec<bool> = "Write the README"
        .chars()
        .enumerate()
        .map(|(index, _)| [10, 13, 14, 15].contains(&index))
        .collect();
    assert_eq!(colours, expected, "{row}");
}

// Command palette and which-key (#46)

#[test]
fn command_palette() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char(':')]);
    screen_snapshot!("palette_100x30", harness, 100, 30);
    harness.type_text("snap");
    screen_snapshot!("palette_query_100x30", harness, 100, 30);
    screen_snapshot!("palette_query_60x20", harness, 60, 20);
}

#[test]
fn which_key_panels() {
    let mut harness = Harness::new(typical_board());
    harness.keys(&[KeyCode::Char(' ')]);
    screen_snapshot!("which_key_space_100x30", harness, 100, 30);
    screen_snapshot!("which_key_space_60x20", harness, 60, 20);
    harness.keys(&[KeyCode::Esc, KeyCode::Char('g')]);
    // Not straight away, so typing g g quickly doesn't flash it up.
    assert!(!harness.screen(100, 30).contains("first card"));
    let later = clock().advance(Duration::from_millis(400));
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| tui_kanban::ui::render(frame, &harness.app.model, later))
        .unwrap();
    insta::assert_snapshot!(
        "which_key_g_100x30",
        buffer_text(terminal.backend().buffer())
    );
}

// Columns (#52)

fn board_with_limits() -> Board {
    let mut board = typical_board();
    board.columns[1].wip_limit = Some(2);
    board.columns[2].wip_limit = Some(1);
    board.columns[2].collapsed = true;
    board
}

#[test]
fn limits_and_a_collapsed_lane() {
    let mut harness = Harness::new(board_with_limits());
    screen_snapshot!("limits_collapsed_100x30", harness, 100, 30);
    screen_snapshot!("limits_collapsed_160x45", harness, 160, 45);
    // The collapsed lane, active on a narrow terminal.
    harness.keys(&[KeyCode::Char('3')]);
    screen_snapshot!("limits_collapsed_active_60x20", harness, 60, 20);
}

#[test]
fn column_dialogs() {
    let mut harness = Harness::new(board_with_limits());
    harness.keys(&[KeyCode::Tab, KeyCode::Char('j'), KeyCode::Char('d')]);
    screen_snapshot!("delete_column_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Esc, KeyCode::Char('c')]);
    screen_snapshot!("column_colours_100x30", harness, 100, 30);
    harness.keys(&[KeyCode::Esc, KeyCode::Char('w')]);
    screen_snapshot!("wip_prompt_100x30", harness, 100, 30);
    harness
        .keys(&[KeyCode::Esc, KeyCode::Char('a')])
        .type_text("Review");
    screen_snapshot!("add_column_100x30", harness, 100, 30);
    harness.keys(&[
        KeyCode::Esc,
        KeyCode::Tab,
        KeyCode::Char('h'),
        KeyCode::Char('L'),
    ]);
    screen_snapshot!("wip_confirm_100x30", harness, 100, 30);
}
