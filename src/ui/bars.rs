//! The top bar (board name, view, search) and the status line (mode, save
//! status, hints or a toast).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::text::{self, input_view, truncate_text};
use super::{AnimationKind, blend_color, faint, fg, fill, muted, progress, put};
use crate::app::{Model, SaveState, ToastKind, ViewMode};
use crate::clock::Clock;
use crate::command::{self, Context};

const SEARCH_MIN_WIDTH: usize = 16;

pub(crate) fn render_top(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    let right_width = render_search(frame, area, model);

    let view = |label: &'static str, current: bool| {
        if current {
            Span::styled(label, fg(theme.accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(label, faint(model))
        }
    };
    let views = [
        Span::raw("   "),
        view("Board", model.ui.view == ViewMode::Board),
        Span::styled(" · ", faint(model)),
        view("All tasks", model.ui.view == ViewMode::AllTasks),
    ];
    let views_width: usize = views.iter().map(Span::width).sum();
    let room = (area.width as usize).saturating_sub(right_width + 2);
    let name_room = room.saturating_sub(views_width).max(room.min(12));
    let mut spans = vec![Span::styled(
        truncate_text(&model.board.name, name_room),
        fg(theme.text).add_modifier(Modifier::BOLD),
    )];
    if name_room + views_width <= room {
        spans.extend(views);
    }
    put(frame, area.x + 1, area.y, room as u16, Line::from(spans));
}

/// Draws the search box or the applied filter on the right of the top
/// bar, and returns how many cells it took.
fn render_search(frame: &mut Frame<'_>, area: Rect, model: &Model) -> usize {
    let theme = &model.ui.theme;
    let search = &model.ui.search;
    let max_width = (area.width / 2) as usize;
    if let Some(input) = &search.input {
        let box_width = (text::width(&input.value) + 2)
            .max(SEARCH_MIN_WIDTH)
            .min(max_width.saturating_sub(2));
        let (visible, cursor) = input_view(&input.value, input.cursor, box_width - 2);
        let box_x = area.right().saturating_sub(1 + box_width as u16);
        put(
            frame,
            box_x.saturating_sub(2),
            area.y,
            2,
            Line::from(Span::styled("/ ", fg(theme.accent))),
        );
        let field = Rect::new(box_x, area.y, box_width as u16, 1);
        fill(
            frame,
            field,
            Style::default()
                .bg(theme.surface)
                .add_modifier(theme.active_modifier),
        );
        put(
            frame,
            box_x + 1,
            area.y,
            box_width as u16 - 2,
            Line::from(Span::styled(visible, fg(theme.text))),
        );
        frame.set_cursor_position((box_x + 1 + cursor as u16, area.y));
        return box_width + 3;
    }
    if search.query.is_empty() {
        return 0;
    }
    let matches = model.visible_tasks().len();
    let suffix = format!(
        " · {matches} {}",
        if matches == 1 { "match" } else { "matches" }
    );
    let query_room = max_width.saturating_sub(2 + suffix.len());
    let line = Line::from(vec![
        Span::styled("/ ", faint(model)),
        Span::styled(truncate_text(&search.query, query_room), fg(theme.text)),
        Span::styled(suffix, faint(model)),
    ]);
    let width = line.width();
    put(
        frame,
        area.right().saturating_sub(1 + width as u16),
        area.y,
        width as u16,
        line,
    );
    width + 1
}

fn mode_label(context: Context) -> &'static str {
    match context {
        Context::Board => "BOARD",
        Context::AllTasks => "ALL TASKS",
        Context::Rail => "RAIL",
        Context::Search => "SEARCH",
        Context::Detail => "DETAIL",
        Context::Editor => "EDIT",
        Context::Help => "HELP",
        Context::Confirm => "DELETE",
        Context::TooSmall => "",
    }
}

pub(crate) fn render_status(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    let context = model.context();
    let pill = if theme.is_monochrome() {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme.panel)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD)
    };
    let (icon, icon_color, status) = match model.session.save_state {
        SaveState::Saved => ("●", theme.success, "saved"),
        SaveState::Failed(_) => ("✕", theme.danger, "not saved"),
    };
    let mut spans = vec![
        Span::styled(format!(" {} ", mode_label(context)), pill),
        Span::raw("  "),
        Span::styled(icon, fg(icon_color)),
        Span::styled(format!(" {status}"), faint(model)),
        Span::raw("   "),
    ];
    let used: usize = spans.iter().map(Span::width).sum();
    let room = (area.width as usize).saturating_sub(used + 1);
    match &model.session.toast {
        Some(toast) => {
            let color = match toast.kind {
                ToastKind::Info => theme.info,
                ToastKind::Success => theme.success,
                ToastKind::Error => theme.danger,
            };
            let color = blend_color(
                theme.panel,
                color,
                progress(model, AnimationKind::Toast, clock),
            );
            spans.push(Span::styled(
                truncate_text(&toast.message, room),
                fg(color).add_modifier(Modifier::BOLD),
            ));
        }
        None => spans.extend(hint_line(model, context, room).spans),
    }
    put(frame, area.x, area.y, area.width, Line::from(spans));
}

/// The hints for a context from the command table, most important first,
/// cut to whole hints that fit in `max_width` cells.
pub(crate) fn hint_line(model: &Model, context: Context, max_width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    let mut used = 0;
    for (keys, label) in command::hints(context, |id| model.command_enabled(id)) {
        let separator = if spans.is_empty() { 0 } else { 3 };
        let width = text::width(&keys) + 1 + text::width(label);
        if used + separator + width > max_width {
            break;
        }
        if separator > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(keys, muted(model)));
        spans.push(Span::styled(format!(" {label}"), faint(model)));
        used += separator + width;
    }
    Line::from(spans)
}
