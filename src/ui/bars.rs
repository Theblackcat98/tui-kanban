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
use crate::command::{self, CommandId, Context};

pub(crate) fn render_top(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    let view = |label: &'static str, current: bool| {
        if current {
            Span::styled(label, fg(theme.accent).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(label, faint(model))
        }
    };
    let (name, show_views) = top_name(model, area);
    let mut spans = vec![Span::styled(
        name,
        fg(theme.text).add_modifier(Modifier::BOLD),
    )];
    if show_views {
        spans.extend([
            Span::raw(VIEW_GAP),
            view(VIEWS[0].0, model.ui.view == ViewMode::Board),
            Span::styled(VIEW_SEPARATOR, faint(model)),
            view(VIEWS[1].0, model.ui.view == ViewMode::AllTasks),
        ]);
    }
    let room = area.width.saturating_sub(2);
    put(frame, area.x + 1, area.y, room, Line::from(spans));
}

const VIEWS: [(&str, ViewMode); 2] = [
    ("Board", ViewMode::Board),
    ("All tasks", ViewMode::AllTasks),
];
const VIEW_GAP: &str = "   ";
const VIEW_SEPARATOR: &str = " · ";

/// The board name as the top bar shows it, cut to fit, and whether the
/// view names fit after it.
fn top_name(model: &Model, area: Rect) -> (String, bool) {
    let views_width = VIEW_GAP.len()
        + text::width(VIEWS[0].0)
        + VIEW_SEPARATOR.chars().count()
        + text::width(VIEWS[1].0);
    let room = (area.width as usize).saturating_sub(2);
    let name_room = room.saturating_sub(views_width).max(room.min(12));
    (
        truncate_text(&model.board.name, name_room),
        name_room + views_width <= room,
    )
}

/// Where the top bar's view names are, for the mouse.
pub(crate) fn view_tabs(model: &Model, area: Rect) -> Vec<(Rect, ViewMode)> {
    let (name, show_views) = top_name(model, area);
    if !show_views {
        return Vec::new();
    }
    let mut x = area.x + 1 + (text::width(&name) + VIEW_GAP.len()) as u16;
    VIEWS
        .iter()
        .map(|(label, view)| {
            let width = text::width(label) as u16;
            let tab = Rect::new(x, area.y, width, 1);
            x += width + VIEW_SEPARATOR.chars().count() as u16;
            (tab, *view)
        })
        .collect()
}

/// The filter bar, below the top bar while a search is active: the search
/// box while typing, or the applied terms as chips, and the match count.
///
/// ```text
/// / in:progress card▏                              2 of 7
///   in:progress ×  card ×   Backspace remove       2 of 7
/// ```
pub(crate) fn render_filter(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let theme = &model.ui.theme;
    let search = &model.ui.search;
    fill(frame, area, Style::default().bg(theme.bg));
    let count = format!(
        "{} of {}",
        model.visible_tasks().len(),
        model.board.task_count()
    );
    let count_width = text::width(&count) as u16;
    put(
        frame,
        area.right().saturating_sub(count_width + 1),
        area.y,
        count_width,
        Line::from(Span::styled(count, faint(model))),
    );
    let room = area.width.saturating_sub(count_width + 4);
    let x = area.x + 1;
    put(
        frame,
        x,
        area.y,
        2,
        Line::from(Span::styled(
            "/ ",
            fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
    );
    let field = Rect::new(x + 2, area.y, room.saturating_sub(2), 1);
    if let Some(input) = &search.input {
        fill(
            frame,
            field,
            Style::default()
                .bg(theme.surface)
                .add_modifier(theme.active_modifier),
        );
        let inner = field.width.saturating_sub(2);
        if input.value.is_empty() {
            put(
                frame,
                field.x + 1,
                area.y,
                inner,
                Line::from(Span::styled(
                    "text, in:column, #tag, updated:<7d",
                    faint(model),
                )),
            );
            frame.set_cursor_position((field.x + 1, area.y));
            return;
        }
        let (visible, cursor) = input_view(&input.value, input.cursor, inner as usize);
        put(
            frame,
            field.x + 1,
            area.y,
            inner,
            Line::from(Span::styled(visible, fg(theme.text))),
        );
        frame.set_cursor_position((field.x + 1 + cursor as u16, area.y));
        return;
    }
    // Applied: each term as a chip, as many as fit.
    let chip = Style::default().bg(theme.surface).fg(theme.text);
    let mut spans = Vec::new();
    let mut used = 0;
    let terms: Vec<&str> = search.filter.terms().map(|(text, _)| text).collect();
    for (index, term) in terms.iter().enumerate() {
        let label = format!(" {term} ×");
        let width = text::width(&label) + 2;
        if used + width > field.width as usize {
            let hidden = terms.len() - index;
            spans.push(Span::styled(format!("+{hidden}"), faint(model)));
            break;
        }
        spans.push(Span::styled(format!(" {term}"), chip));
        spans.push(Span::styled(" × ", chip.fg(theme.text_faint)));
        spans.push(Span::raw(" "));
        used += width;
    }
    let hint = "Backspace removes the last term";
    if used + text::width(hint) < field.width as usize {
        spans.push(Span::styled(hint, faint(model)));
    }
    put(frame, field.x, area.y, field.width, Line::from(spans));
}

pub(crate) fn render_status(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    let context = model.context();
    let mut spans = status_prefix(model);
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

/// The start of the status line: the mode pill and the save status.
fn status_prefix(model: &Model) -> Vec<Span<'static>> {
    let theme = &model.ui.theme;
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
        SaveState::Saving => ("◌", theme.text_muted, "saving"),
        SaveState::Failed(_) => ("✕", theme.danger, "not saved"),
        SaveState::Conflict => ("✕", theme.warning, "changed on disk"),
    };
    vec![
        Span::styled(format!(" {} ", model.context().label()), pill),
        Span::raw("  "),
        Span::styled(icon, fg(icon_color)),
        Span::styled(format!(" {status}"), faint(model)),
        Span::raw("   "),
    ]
}

/// Where the status line's hints are, and the command each runs, for the
/// mouse. There are none while a toast shows.
pub(crate) fn status_hints(model: &Model, area: Rect) -> Vec<(Rect, CommandId)> {
    if model.session.toast.is_some() {
        return Vec::new();
    }
    let used: usize = status_prefix(model).iter().map(Span::width).sum();
    let room = (area.width as usize).saturating_sub(used + 1);
    fitted_hints(model, model.context(), room)
        .into_iter()
        .map(|hint| {
            let x = area.x + (used + hint.offset) as u16;
            (Rect::new(x, area.y, hint.width as u16, 1), hint.id)
        })
        .collect()
}

/// The first-run tip, above the status line, until it is dismissed.
pub(crate) fn render_tip(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    let theme = &model.ui.theme;
    fill(frame, area, Style::default().bg(theme.panel));
    let line = Line::from(vec![
        Span::styled(" Tip ", fg(theme.info).add_modifier(Modifier::BOLD)),
        Span::styled(" Press ", faint(model)),
        Span::styled("?", fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" for keys, ", faint(model)),
        Span::styled(":", fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" for commands · ", faint(model)),
        Span::styled("Esc", fg(theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" to dismiss", faint(model)),
    ]);
    put(frame, area.x, area.y, area.width, line);
}

/// A footer hint that fits, and where it goes.
struct FittedHint {
    keys: String,
    label: &'static str,
    id: CommandId,
    /// Cells from the start of the hints.
    offset: usize,
    width: usize,
}

/// The hints for a context from the command table, most important first,
/// cut to whole hints that fit in `max_width` cells, three cells apart.
fn fitted_hints(model: &Model, context: Context, max_width: usize) -> Vec<FittedHint> {
    let mut fitted = Vec::new();
    let mut used = 0;
    for (keys, label, id) in command::hint_commands(context, |id| model.command_enabled(id)) {
        let separator = if fitted.is_empty() { 0 } else { 3 };
        let width = text::width(&keys) + 1 + text::width(label);
        if used + separator + width > max_width {
            break;
        }
        fitted.push(FittedHint {
            keys,
            label,
            id,
            offset: used + separator,
            width,
        });
        used += separator + width;
    }
    fitted
}

/// The hints for a context from the command table, most important first,
/// cut to whole hints that fit in `max_width` cells.
pub(crate) fn hint_line(model: &Model, context: Context, max_width: usize) -> Line<'static> {
    let mut spans = Vec::new();
    for hint in fitted_hints(model, context, max_width) {
        if !spans.is_empty() {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(hint.keys, muted(model)));
        spans.push(Span::styled(format!(" {}", hint.label), faint(model)));
    }
    Line::from(spans)
}
