//! The command palette overlay.
//!
//! ```text
//! ╭ Commands ──────────────────────────────────────╮
//! │ › mov                                           │
//! │                                                 │
//! │▌Move to…                                     m │
//! │ Move task left                               H │
//! │                                                 │
//! │ Enter run   ↓/↑ choose   Esc close              │
//! ╰─────────────────────────────────────────────────╯
//! ```

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use unicode_segmentation::UnicodeSegmentation;

use super::bars::hint_line;
use super::text::{self, input_view, truncate_text};
use super::{AnimationKind, centered_rect, fade_in, faint, fg, fill, overlay_block, progress, put};
use crate::app::{Model, Palette, palette_entries};
use crate::clock::Clock;
use crate::command::Context;

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    palette: &Palette,
    clock: Clock,
) {
    let theme = &model.ui.theme;
    let entries = palette_entries(model, palette);
    let modal = centered_rect(area, 72, 22);
    frame.render_widget(Clear, modal);
    let block = overlay_block(model, "Commands", theme.border);
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let inner = Rect::new(
        inner.x + 1,
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    if inner.height < 4 {
        return;
    }

    // The query.
    let row = Rect::new(inner.x, inner.y, inner.width, 1);
    fill(
        frame,
        row,
        Style::default()
            .bg(theme.surface)
            .add_modifier(theme.active_modifier),
    );
    put(
        frame,
        row.x + 1,
        row.y,
        2,
        Line::from(Span::styled(
            "›",
            fg(theme.accent).add_modifier(Modifier::BOLD),
        )),
    );
    let input_x = row.x + 3;
    let input_width = row.width.saturating_sub(4);
    if palette.input.value.is_empty() {
        put(
            frame,
            input_x,
            row.y,
            input_width,
            Line::from(Span::styled(
                "Type a command, a lane or a task",
                faint(model),
            )),
        );
        frame.set_cursor_position((input_x, row.y));
    } else {
        let (visible, cursor) = input_view(
            &palette.input.value,
            palette.input.cursor,
            input_width as usize,
        );
        put(
            frame,
            input_x,
            row.y,
            input_width,
            Line::from(Span::styled(visible, fg(theme.text))),
        );
        frame.set_cursor_position((input_x + cursor as u16, row.y));
    }

    // The entries, scrolled to keep the highlighted one in view.
    let list = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 4);
    if entries.is_empty() {
        put(
            frame,
            list.x + 1,
            list.y,
            list.width.saturating_sub(1),
            Line::from(Span::styled("No matches", faint(model))),
        );
    }
    let selected = palette.selected.min(entries.len().saturating_sub(1));
    let first = selected.saturating_sub(list.height.saturating_sub(1) as usize);
    let highlight = if theme.is_monochrome() {
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        fg(theme.accent).add_modifier(Modifier::BOLD)
    };
    for (row, (index, entry)) in entries
        .iter()
        .enumerate()
        .skip(first)
        .take(list.height as usize)
        .enumerate()
    {
        let y = list.y + row as u16;
        let chosen = index == selected;
        if chosen {
            fill(
                frame,
                Rect::new(list.x, y, list.width, 1),
                Style::default()
                    .bg(theme.selection)
                    .add_modifier(theme.selected_modifier),
            );
            put(
                frame,
                list.x,
                y,
                1,
                Line::from(Span::styled("▌", fg(theme.accent))),
            );
        }
        let keys_width = text::width(&entry.keys) as u16;
        put(
            frame,
            list.right().saturating_sub(keys_width + 1),
            y,
            keys_width,
            Line::from(Span::styled(entry.keys.clone(), faint(model))),
        );
        let room = list.width.saturating_sub(keys_width + 4) as usize;
        let label = truncate_text(&entry.label, room);
        let base = if chosen {
            fg(theme.text).add_modifier(Modifier::BOLD)
        } else {
            fg(theme.text)
        };
        let mut spans: Vec<Span> = label
            .graphemes(true)
            .enumerate()
            .map(|(index, grapheme)| {
                let style = if entry.highlights.contains(&index) {
                    base.patch(highlight)
                } else {
                    base
                };
                Span::styled(grapheme.to_owned(), style)
            })
            .collect();
        let used = text::width(&label);
        if !entry.detail.is_empty() && used + 4 < room {
            spans.push(Span::styled(
                format!("  {}", truncate_text(&entry.detail, room - used - 2)),
                faint(model),
            ));
        }
        put(frame, list.x + 2, y, room as u16, Line::from(spans));
    }

    put(
        frame,
        inner.x + 1,
        inner.bottom().saturating_sub(1),
        inner.width.saturating_sub(1),
        hint_line(
            model,
            Context::Palette,
            inner.width.saturating_sub(1) as usize,
        ),
    );
    fade_in(
        frame.buffer_mut(),
        modal,
        theme.bg,
        progress(model, AnimationKind::Modal, clock),
    );
}
