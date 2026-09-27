//! Confirmation dialogs: deleting a task, discarding an edited draft, a
//! board that changed on disk, and quitting with unsaved changes.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use uuid::Uuid;

use super::bars::hint_line;
use super::text::truncate_text;
use super::{AnimationKind, centered_rect, fade_in, fg, muted, overlay_block, progress, put};
use crate::app::{Model, SaveState};
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, model: &Model, task: Uuid, clock: Clock) {
    let title = model
        .board
        .task(task)
        .map(|task| task.title.as_str())
        .unwrap_or("this task");
    let question = |width: usize| {
        format!(
            "Delete \"{}\"?",
            truncate_text(title, width.saturating_sub(10))
        )
    };
    dialog(
        frame,
        area,
        model,
        "Delete task",
        &question,
        "You can undo this with u.",
        Context::Confirm,
        clock,
    );
}

/// "Discard changes?", when an edited draft is cancelled.
pub(crate) fn render_discard(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    dialog(
        frame,
        area,
        model,
        "Discard changes",
        &|_| "Discard your changes?".to_owned(),
        "They can't be undone.",
        Context::Discard,
        clock,
    );
}

/// The board file changed on disk while there were unsaved changes here.
pub(crate) fn render_conflict(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    dialog(
        frame,
        area,
        model,
        "Changed on disk",
        &|_| "Another program changed the board.".to_owned(),
        "Load it (u undoes), or keep your version.",
        Context::Conflict,
        clock,
    );
}

/// "Quit without saving?", with why the changes aren't saved.
pub(crate) fn render_quit(frame: &mut Frame<'_>, area: Rect, model: &Model, clock: Clock) {
    let reason = match &model.session.save_state {
        SaveState::Failed(message) => format!("Saving failed: {message}"),
        SaveState::Conflict => "The board changed on disk.".to_owned(),
        SaveState::Saving | SaveState::Saved => "Saving is taking too long.".to_owned(),
    };
    dialog(
        frame,
        area,
        model,
        "Quit",
        &|_| "Your changes aren't saved. Quit anyway?".to_owned(),
        &reason,
        Context::ConfirmQuit,
        clock,
    );
}

/// A small dialog with a `danger` border: a question sized to the
/// dialog's width, a line of detail, and the context's keys.
#[allow(clippy::too_many_arguments)]
fn dialog(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    title: &str,
    question: &dyn Fn(usize) -> String,
    detail: &str,
    context: Context,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let modal = centered_rect(area, 52, 8);
    frame.render_widget(Clear, modal);
    let block = overlay_block(model, title, theme.danger);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let inner = Rect::new(
        inner.x + 2,
        inner.y + 1,
        inner.width.saturating_sub(4),
        inner.height.saturating_sub(2),
    );
    let lines = [
        Line::from(Span::styled(
            question(inner.width as usize),
            fg(theme.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            truncate_text(detail, inner.width as usize),
            muted(model),
        )),
        Line::default(),
        hint_line(model, context, inner.width as usize),
    ];
    for (row, line) in lines.into_iter().enumerate() {
        if row as u16 >= inner.height {
            break;
        }
        put(frame, inner.x, inner.y + row as u16, inner.width, line);
    }
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
