mod cards;
mod dashboard;
mod detail;
mod help;
mod sidebar;
mod task_editor;

use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::{Model, SaveState, Screen, ToastKind};
use crate::clock::Clock;
use crate::command::{self, Context};
use crate::layout::{self, Breakpoint};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame<'_>, model: &Model, clock: Clock) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(model.ui.theme.background)),
        area,
    );
    if !layout::fits(area.width, area.height) {
        render_too_small(frame, area, model);
        return;
    }

    let sections = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .split(area);
    render_header(frame, sections[0], model);
    let content = sections[1];
    // Worked out once per frame, from the same terminal width the key
    // handlers use, so what is drawn and what keys do always agree.
    let breakpoint = Breakpoint::from_width(area.width);
    if breakpoint.shows_rail() {
        let columns = Layout::horizontal([Constraint::Length(sidebar::WIDTH), Constraint::Min(1)])
            .split(content);
        sidebar::render(frame, columns[0], model);
        dashboard::render(frame, columns[1], model, breakpoint, clock);
    } else {
        dashboard::render(frame, content, model, breakpoint, clock);
    }
    render_footer(frame, sections[2], model, clock);

    // Screens are drawn bottom first, so a dialog opened from the detail
    // drawer appears on top of it.
    for screen in &model.ui.screens {
        match screen {
            Screen::Editor(editor) => task_editor::render(frame, area, model, editor, clock),
            Screen::Detail { task, scroll } => {
                detail::render(frame, area, model, *task, *scroll, breakpoint, clock)
            }
            Screen::Help { scroll } => help::render(frame, area, model, *scroll, clock),
            Screen::ConfirmDelete { task } => {
                help::render_confirm(frame, area, model, *task, clock)
            }
        }
    }
}

/// Shown instead of a broken layout when the terminal is below the
/// minimum size. Only quitting works until it grows.
fn render_too_small(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let lines = vec![
        Line::from(Span::styled(
            "Terminal too small",
            Style::default()
                .fg(model.ui.theme.text)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{}×{}, need {}×{}",
                area.width,
                area.height,
                layout::MIN_WIDTH,
                layout::MIN_HEIGHT
            ),
            muted_style(model),
        )),
        Line::from(Span::styled("q quit", muted_style(model))),
    ];
    let height = (lines.len() as u16).min(area.height);
    let top = area.y + area.height.saturating_sub(height) / 2;
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true }),
        Rect::new(area.x, top, area.width, height),
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let left = Line::from(vec![
        Span::styled(
            " TUI KANBAN ",
            Style::default()
                .fg(model.ui.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  ", model.board.name),
            Style::default().fg(model.ui.theme.text),
        ),
        Span::styled(
            format!(" [{}] ", model.ui.view.label()),
            Style::default().fg(model.ui.theme.accent_alt),
        ),
    ]);
    let searching = model.ui.search.is_typing();
    let right_text = if searching || !model.ui.search.query.is_empty() {
        format!(" / {}", model.ui.search.query)
    } else {
        format!("  {}  ", model.status_text())
    };
    let right_color = match &model.session.save_state {
        SaveState::Failed(_) => model.ui.theme.error,
        SaveState::Saved => model.ui.theme.muted,
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
    if let Some(input) = &model.ui.search.input
        && columns[1].width > 0
    {
        let cursor = input.cursor.min(input.value.len());
        let prefix = format!(" /{}", &input.value[..cursor]);
        let offset = Line::from(prefix)
            .width()
            .min(columns[1].width.saturating_sub(1) as usize) as u16;
        frame.set_cursor_position((columns[1].x.saturating_add(offset), columns[1].y));
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    let hints = hint_text(
        model,
        model.context(),
        area.width.saturating_sub(2) as usize,
    );
    let mut spans = vec![Span::styled(format!(" {hints} "), muted_style(model))];
    if let Some(toast) = &model.session.toast {
        let color = match toast.kind {
            ToastKind::Info => model.ui.theme.accent_alt,
            ToastKind::Success => model.ui.theme.success,
            ToastKind::Error => model.ui.theme.error,
        };
        let progress = model
            .ui
            .animations
            .progress(AnimationKind::Toast, clock.instant)
            .map(ease_out_cubic)
            .unwrap_or(1.0);
        let color = blend_color(model.ui.theme.muted, color, progress);
        spans.push(Span::styled(
            format!("  {} ", toast.message),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The footer hints for a context from the command table, joined with
/// " • " and cut to whole hints that fit in `max_width` cells.
pub(crate) fn hint_text(model: &Model, context: Context, max_width: usize) -> String {
    let mut text = String::new();
    for (keys, label) in command::hints(context, |id| model.command_enabled(id)) {
        let hint = format!("{keys} {label}");
        let separator = if text.is_empty() { "" } else { "  •  " };
        if text.width() + separator.width() + hint.width() > max_width {
            break;
        }
        text.push_str(separator);
        text.push_str(&hint);
    }
    text
}

pub(crate) fn help_line_count() -> usize {
    help::line_count()
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

/// Cuts `text` to at most `max_width` terminal cells, ending with "…" when
/// anything was removed. Text that fits is returned unchanged. Works on
/// grapheme clusters, so combining marks stay with their base character
/// and wide characters are never split.
pub(crate) fn truncate_text(text: &str, max_width: usize) -> String {
    if text.width() <= max_width {
        return text.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }
    let budget = max_width - 1;
    let mut result = String::with_capacity(text.len().min(max_width * 4));
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let width = grapheme.width();
        if used + width > budget {
            break;
        }
        used += width;
        result.push_str(grapheme);
    }
    result.push('…');
    result
}

pub(crate) fn short_id(id: uuid::Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

pub(crate) fn text_style(model: &Model) -> Style {
    Style::default()
        .fg(model.ui.theme.text)
        .bg(model.ui.theme.background)
}

pub(crate) fn muted_style(model: &Model) -> Style {
    Style::default()
        .fg(model.ui.theme.muted)
        .add_modifier(model.ui.theme.muted_modifier)
}

pub(crate) fn surface_style(model: &Model) -> Style {
    Style::default()
        .fg(model.ui.theme.text)
        .bg(model.ui.theme.surface)
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

pub(crate) fn field_label(model: &Model, active: bool) -> Style {
    if active {
        Style::default()
            .fg(model.ui.theme.accent)
            .add_modifier(Modifier::BOLD | model.ui.theme.active_modifier)
    } else {
        muted_style(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ViewMode;
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn truncation_respects_the_requested_width() {
        assert_eq!(truncate_text("abcdef", 4), "abc…");
        assert_eq!(truncate_text("abc", 1), "…");
        assert_eq!(truncate_text("abc", 0), "");
        assert_eq!(truncate_text("", 0), "");
    }

    #[test]
    fn truncation_keeps_text_that_fits_exactly() {
        assert_eq!(truncate_text("abcd", 4), "abcd");
        assert_eq!(truncate_text("abcd", 5), "abcd");
        assert_eq!(truncate_text("日本", 4), "日本");
    }

    #[test]
    fn truncation_measures_wide_characters() {
        // Each CJK character is two cells wide.
        assert_eq!(truncate_text("日本語", 5), "日本…");
        assert_eq!(truncate_text("日本語", 4), "日…");
        // A two-cell character never overflows a one-cell budget.
        assert_eq!(truncate_text("日本", 1), "…");
        assert_eq!(truncate_text("a🦀b", 3), "a…");
    }

    #[test]
    fn truncation_keeps_combining_marks_with_their_base() {
        // "e" + combining acute accent is one cell wide.
        let text = "e\u{301}e\u{301}e\u{301}";
        assert_eq!(truncate_text(text, 3), text);
        assert_eq!(truncate_text(text, 2), "e\u{301}…");
    }

    #[test]
    fn truncation_handles_long_text_quickly() {
        let text = "x".repeat(100_000);
        let cut = truncate_text(&text, 50_000);
        assert_eq!(cut.width(), 50_000);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn relative_time_uses_the_passed_in_clock() {
        assert_eq!(cards::relative_time(0, 30_000), "just now");
        assert_eq!(cards::relative_time(0, 5 * 60_000), "5m");
        assert_eq!(cards::relative_time(0, 3 * 86_400_000), "3d");
    }

    fn render_text(model: &Model, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, model, Clock::fixed(0)))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn model() -> Model {
        Model::new(Default::default(), Theme::mocha(), false)
    }

    #[test]
    fn dashboard_renders_with_test_backend() {
        let mut model = model();
        model
            .board
            .add_task(0, "Write docs", "Start here", 0)
            .unwrap();
        let rendered = render_text(&model, 100, 30);
        assert!(rendered.contains("TUI KANBAN"));
        assert!(rendered.contains("Write docs"));
        assert!(rendered.contains("Backlog"));
    }

    #[test]
    fn all_tasks_renders_cards_and_rail() {
        let mut model = model();
        model
            .board
            .add_task(0, "Shape cards", "Make scanning easier", 0)
            .unwrap();
        model
            .board
            .add_task(1, "Tune navigation", "Rail and focus", 0)
            .unwrap();
        model.ui.view = ViewMode::AllTasks;
        let rendered = render_text(&model, 120, 40);
        assert!(rendered.contains("ALL TASKS"));
        assert!(rendered.contains("COLUMNS"));
        assert!(rendered.contains("Shape cards"));
        assert!(rendered.contains("Tune navigation"));
    }

    #[test]
    fn detail_drawer_renders_metadata() {
        let mut model = model();
        let task = model
            .board
            .add_task(0, "Inspect me", "A useful description", 0)
            .unwrap();
        model.ui.screens.push(Screen::Detail { task, scroll: 0 });
        let rendered = render_text(&model, 100, 30);
        assert!(rendered.contains("Task details"));
        assert!(rendered.contains("created"));
        assert!(rendered.contains("updated"));
        assert!(rendered.contains("Inspect me"));
    }

    #[test]
    fn tiny_terminals_get_a_too_small_screen() {
        let rendered = render_text(&model(), 30, 8);
        assert!(rendered.contains("Terminal too small"));
        assert!(!rendered.contains("TUI KANBAN"));
    }
}
