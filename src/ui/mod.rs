mod cards;
mod dashboard;
mod detail;
mod help;
mod sidebar;
mod task_editor;

use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::{App, Mode, SaveState, ToastKind};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use std::time::Instant;

pub(crate) fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.background)),
        area,
    );

    let sections = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(area);
    let now = Instant::now();
    render_header(frame, sections[0], app);
    let content = sections[1];
    if content.width >= sidebar::WIDTH + 56 {
        let columns = Layout::horizontal([Constraint::Length(sidebar::WIDTH), Constraint::Min(1)])
            .split(content);
        sidebar::render(frame, columns[0], app);
        dashboard::render(frame, columns[1], app, now);
    } else {
        dashboard::render(frame, content, app, now);
    }
    render_footer(frame, sections[2], app, now);

    match &app.mode {
        Mode::Editor(editor) => task_editor::render(frame, area, app, editor, now),
        Mode::Detail(id) => detail::render(frame, area, app, *id, now),
        Mode::Help => help::render(frame, area, app, now),
        Mode::ConfirmDelete(id) => help::render_confirm(frame, area, app, *id, now),
        Mode::Dashboard => {}
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let left = Line::from(vec![
        Span::styled(
            " TUI KANBAN ",
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  ", app.board.name),
            Style::default().fg(app.theme.text),
        ),
        Span::styled(
            format!(" [{}] ", app.view_mode.label()),
            Style::default().fg(app.theme.accent_alt),
        ),
    ]);
    let searching = app.search_active;
    let right_text = if searching || !app.search_query.is_empty() {
        format!(" / {}", app.search_query)
    } else {
        format!("  {}  ", app.status_text())
    };
    let right_color = match &app.save_state {
        SaveState::Error(_) => app.theme.error,
        SaveState::Dirty => app.theme.warning,
        SaveState::Clean => app.theme.muted,
    };
    let right = Line::from(Span::styled(right_text, Style::default().fg(right_color))).alignment(
        if searching {
            Alignment::Left
        } else {
            Alignment::Right
        },
    );
    let columns = Layout::horizontal([Constraint::Min(1), Constraint::Length(32)]).split(area);
    frame.render_widget(Paragraph::new(left), columns[0]);
    frame.render_widget(Paragraph::new(right), columns[1]);
    if searching && columns[1].width > 0 {
        let cursor = app.search_input.cursor.min(app.search_input.value.len());
        let prefix = format!(" /{}", &app.search_input.value[..cursor]);
        let offset = Line::from(prefix)
            .width()
            .min(columns[1].width.saturating_sub(1) as usize) as u16;
        frame.set_cursor_position((columns[1].x.saturating_add(offset), columns[1].y));
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App, now: Instant) {
    let hints = match &app.mode {
        Mode::Dashboard if app.search_active => "type to search  •  enter search  •  esc clear",
        Mode::Dashboard if app.focus == crate::app::FocusRegion::Rail => {
            "tab cards  •  h/l column  •  enter focus  •  v view  •  ? help"
        }
        Mode::Dashboard if app.view_mode == crate::app::ViewMode::AllTasks => {
            "tab rail  •  v view  •  j/k cards  •  h/l column  •  n new  •  ? help"
        }
        Mode::Dashboard => "tab rail  •  v view  •  hjkl move  •  n new  •  e edit  •  ? help",
        Mode::Editor(_) => "tab switch  •  enter save  •  esc cancel",
        Mode::Detail(_) => "e edit  •  H/L move  •  PgUp/PgDn scroll  •  esc close",
        Mode::Help => "any key close",
        Mode::ConfirmDelete(_) => "y confirm  •  n cancel",
    };
    let mut spans = vec![Span::styled(
        format!(" {hints} "),
        Style::default().fg(app.theme.muted),
    )];
    if let Some(toast) = &app.toast {
        let color = match toast.kind {
            ToastKind::Info => app.theme.accent_alt,
            ToastKind::Success => app.theme.success,
            ToastKind::Error => app.theme.error,
        };
        let progress = app
            .animations
            .progress(AnimationKind::Toast, now)
            .map(ease_out_cubic)
            .unwrap_or(1.0);
        let color = blend_color(app.theme.muted, color, progress);
        spans.push(Span::styled(
            format!("  {} ", toast.message),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub(crate) fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

pub(crate) fn truncate_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return text
            .chars()
            .next()
            .map(|character| character.to_string())
            .unwrap_or_default();
    }
    let limit = max_width - 1;
    let mut result = String::new();
    for character in text.chars() {
        let character_width = Line::from(character.to_string()).width();
        if Line::from(result.clone())
            .width()
            .saturating_add(character_width)
            > limit
        {
            result.push('…');
            break;
        }
        result.push(character);
    }
    result
}

pub(crate) fn short_id(id: uuid::Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

pub(crate) fn text_style(app: &App) -> Style {
    Style::default().fg(app.theme.text).bg(app.theme.background)
}

pub(crate) fn surface_style(app: &App) -> Style {
    Style::default().fg(app.theme.text).bg(app.theme.surface)
}

pub(crate) fn blend_color(from: Color, to: Color, progress: f32) -> Color {
    let progress = progress.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(from_r, from_g, from_b), Color::Rgb(to_r, to_g, to_b)) => {
            let mix = |start: u8, end: u8| {
                (f32::from(start) + (f32::from(end) - f32::from(start)) * progress).round() as u8
            };
            Color::Rgb(mix(from_r, to_r), mix(from_g, to_g), mix(from_b, to_b))
        }
        _ => to,
    }
}

pub(crate) fn render_clear(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
}

pub(crate) fn field_label(app: &App, _label: &str, active: bool) -> Style {
    if active {
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.muted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Mode, ViewMode};
    use crate::storage::JsonStore;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn truncation_respects_the_requested_width() {
        assert_eq!(truncate_text("abcdef", 4), "abc…");
        assert_eq!(truncate_text("abc", 1), "a");
        assert_eq!(truncate_text("abc", 0), "");
    }

    #[test]
    fn dashboard_renders_with_test_backend() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let mut app = App::new(store, false).unwrap();
        app.board.add_task(0, "Write docs", "Start here").unwrap();
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("TUI KANBAN"));
        assert!(rendered.contains("Write docs"));
        assert!(rendered.contains("Backlog"));
    }

    #[test]
    fn all_tasks_renders_cards_and_rail() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let mut app = App::new(store, false).unwrap();
        app.board
            .add_task(0, "Shape cards", "Make scanning easier")
            .unwrap();
        app.board
            .add_task(1, "Tune navigation", "Rail and focus")
            .unwrap();
        app.view_mode = ViewMode::AllTasks;
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("ALL TASKS"));
        assert!(rendered.contains("COLUMNS"));
        assert!(rendered.contains("Shape cards"));
        assert!(rendered.contains("Tune navigation"));
    }

    #[test]
    fn detail_drawer_renders_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let store = JsonStore::new(directory.path().join("board.json"));
        let mut app = App::new(store, false).unwrap();
        let id = app
            .board
            .add_task(0, "Inspect me", "A useful description")
            .unwrap();
        app.mode = Mode::Detail(id);
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Task details"));
        assert!(rendered.contains("created"));
        assert!(rendered.contains("updated"));
        assert!(rendered.contains("Inspect me"));
    }
}
