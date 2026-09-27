//! The view: draws the model. Nothing here changes state. The layout is
//! worked out in `geometry`, and the look follows `docs/design.md`.

mod all_tasks;
mod bars;
mod card;
mod confirm;
mod detail;
mod editor;
mod geometry;
mod help;
mod lanes;
mod menu;
mod palette;
mod prompt;
mod rail;
mod rich;
mod text;
mod which_key;

pub(crate) use geometry::{all_tasks_rows, reveal_detail_item, sync_scroll};
pub use which_key::DELAY as WHICH_KEY_DELAY;

use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::{Model, Screen, ViewMode};
use crate::clock::Clock;
use crate::layout;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

pub fn render(frame: &mut Frame<'_>, model: &Model, clock: Clock) {
    let area = frame.area();
    fill(frame, area, Style::default().bg(model.ui.theme.bg));
    if !layout::fits(area.width, area.height) {
        render_too_small(frame, area, model);
        return;
    }

    let page = geometry::page(model, area);
    bars::render_top(frame, page.top, model);
    if let Some(filter) = page.filter {
        bars::render_filter(frame, filter, model);
    }
    if let Some(rail) = page.rail {
        rail::render(frame, rail, model);
    }
    match model.ui.view {
        ViewMode::Board => lanes::render(frame, &page, model, clock),
        ViewMode::AllTasks => all_tasks::render(frame, page.main, model, clock),
    }
    if let Some(tip) = page.tip {
        bars::render_tip(frame, tip, model);
    }
    let above_bars = page.tip.map_or(page.status.y, |tip| tip.y);
    which_key::render(frame, area, above_bars, model, clock);
    bars::render_status(frame, page.status, model, clock);

    // Screens are drawn bottom first, so a dialog opened from the detail
    // drawer appears on top of it.
    for screen in &model.ui.screens {
        match screen {
            Screen::Detail { task, scroll, item } => {
                if let Some(drawer) = page.drawer {
                    detail::render(frame, drawer, model, *task, *scroll, *item, clock);
                }
            }
            Screen::Editor(editor) => editor::render(frame, area, model, editor, clock),
            Screen::Help { scroll, context } => {
                help::render(frame, area, model, *scroll, *context, clock)
            }
            Screen::ConfirmDelete { task } => confirm::render(frame, area, model, *task, clock),
            Screen::ConfirmDiscard => confirm::render_discard(frame, area, model, clock),
            Screen::Conflict => confirm::render_conflict(frame, area, model, clock),
            Screen::ConfirmQuit => confirm::render_quit(frame, area, model, clock),
            Screen::Palette(state) => palette::render(frame, area, model, state, clock),
            Screen::MoveTo { task, selected } => {
                menu::render(frame, area, model, *task, *selected, clock)
            }
            Screen::QuickAdd { column, input } => {
                prompt::render(frame, area, model, *column, input, clock)
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
            fg(model.ui.theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{}×{}, need {}×{}",
                area.width,
                area.height,
                layout::MIN_WIDTH,
                layout::MIN_HEIGHT
            ),
            muted(model),
        )),
        Line::from(Span::styled("q quit", faint(model))),
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

pub(crate) fn help_line_count(context: crate::command::Context) -> usize {
    help::line_count(context)
}

/// A column's accent colour.
pub(crate) fn lane_color(model: &Model, column: usize) -> Color {
    match model.board.columns.get(column) {
        Some(column) => model
            .ui
            .theme
            .column_color(&column.id, column.color.as_deref()),
        None => model.ui.theme.lane(column),
    }
}

/// A text colour, leaving the background as it is.
pub(crate) fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

/// Descriptions and unfocused names.
pub(crate) fn muted(model: &Model) -> Style {
    fg(model.ui.theme.text_muted).add_modifier(model.ui.theme.muted_modifier)
}

/// Metadata, counts, hints and placeholders.
pub(crate) fn faint(model: &Model) -> Style {
    fg(model.ui.theme.text_faint).add_modifier(model.ui.theme.muted_modifier)
}

/// Clears `area` and fills it with `style`.
pub(crate) fn fill(frame: &mut Frame<'_>, area: Rect, style: Style) {
    let area = area.intersection(frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(style), area);
}

/// Draws one line of text at (x, y), cut to `width` cells.
pub(crate) fn put(frame: &mut Frame<'_>, x: u16, y: u16, width: u16, line: Line<'_>) {
    let area = Rect::new(x, y, width, 1).intersection(frame.area());
    if !area.is_empty() {
        frame.render_widget(Paragraph::new(line), area);
    }
}

/// An animation's eased progress, or 1.0 when it isn't running.
pub(crate) fn progress(model: &Model, kind: AnimationKind, clock: Clock) -> f32 {
    model
        .ui
        .animations
        .progress(kind, clock.instant)
        .map(ease_out_cubic)
        .unwrap_or(1.0)
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

/// Fades `area` in from `background`: every colour in it is blended from
/// the background towards its own value.
pub(crate) fn fade_in(buffer: &mut Buffer, area: Rect, background: Color, progress: f32) {
    if progress >= 1.0 {
        return;
    }
    let area = area.intersection(buffer.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buffer[(x, y)];
            cell.fg = blend_color(background, cell.fg, progress);
            cell.bg = blend_color(background, cell.bg, progress);
        }
    }
}

/// The overlay frame shared by help, the editor and the delete dialog:
/// centred, on `panel`, with a rounded border and a title.
pub(crate) fn overlay_block<'a>(model: &Model, title: &'a str, border: Color) -> Block<'a> {
    Block::bordered()
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(fg(border))
        .style(Style::default().bg(model.ui.theme.panel))
        .title(Span::styled(
            format!(" {title} "),
            fg(if model.ui.theme.is_monochrome() {
                border
            } else {
                model.ui.theme.accent
            })
            .add_modifier(Modifier::BOLD),
        ))
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

pub(crate) fn short_id(id: uuid::Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_text(model: &Model, width: u16, height: u16) -> String {
        let mut model = model.clone();
        model.ui.viewport = (width, height);
        let model = &model;
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
    fn board_renders_lanes_and_cards() {
        let mut model = model();
        model
            .board
            .add_task(0, "Write docs", "Start here", 0)
            .unwrap();
        model.reconcile_selection();
        let rendered = render_text(&model, 100, 30);
        assert!(rendered.contains("Write docs"));
        assert!(rendered.contains("Start here"));
        assert!(rendered.contains("Backlog"));
        assert!(rendered.contains("BOARD"));
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
        assert!(rendered.contains("Columns"));
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
        model.ui.screens.push(Screen::Detail {
            task,
            scroll: 0,
            item: 0,
        });
        let rendered = render_text(&model, 100, 30);
        assert!(rendered.contains("DETAIL"), "{rendered}");
        assert!(rendered.contains("created"));
        assert!(rendered.contains("updated"));
        assert!(rendered.contains("Inspect me"));
    }

    #[test]
    fn tiny_terminals_get_a_too_small_screen() {
        let rendered = render_text(&model(), 30, 8);
        assert!(rendered.contains("Terminal too small"));
        assert!(!rendered.contains("BOARD"));
    }

    #[test]
    fn fading_blends_from_the_background() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        buffer[(0, 0)].fg = Color::Rgb(200, 100, 0);
        let area = buffer.area;
        fade_in(&mut buffer, area, Color::Rgb(0, 0, 0), 0.5);
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(100, 50, 0));
    }
}
