use super::muted_style;
use super::{centered_rect, field_label, render_clear, surface_style, truncate_text};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::{EditorField, EditorState, Model};
use crate::clock::Clock;
use crate::command::Context;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    editor: &EditorState,
    clock: Clock,
) {
    let full_width = (area.width.saturating_mul(4) / 5)
        .clamp(28, 72)
        .min(area.width.saturating_sub(2));
    let full_height = 15.min(area.height.saturating_sub(2));
    let progress = model
        .ui
        .animations
        .progress(AnimationKind::Modal, clock.instant)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let width = ((full_width as f32 * progress).round() as u16).min(area.width);
    let height = ((full_height as f32 * progress).round() as u16).min(area.height);
    if width < 12 || height < 7 {
        return;
    }
    let modal = centered_rect(area, width, height);
    render_clear(frame, modal);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(model.ui.theme.accent))
        .style(surface_style(model))
        .title(Span::styled(
            if editor.task_id.is_some() {
                " Edit task "
            } else {
                " New task "
            },
            Style::default()
                .fg(model.ui.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    if inner.height < 5 || inner.width < 8 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(inner);
    render_input(
        frame,
        rows[0],
        "Title",
        "Task title",
        &editor.title,
        editor.field == EditorField::Title,
        model,
    );
    render_input(
        frame,
        rows[1],
        "Description",
        "Add details (optional)",
        &editor.description,
        editor.field == EditorField::Description,
        model,
    );
    let hints = super::hint_text(
        model,
        Context::Editor,
        inner.width.saturating_sub(1) as usize,
    );
    let message = editor.error.as_deref().unwrap_or(&hints);
    let color = if editor.error.is_some() {
        model.ui.theme.error
    } else {
        model.ui.theme.muted
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {message}"),
            Style::default().fg(color),
        )))
        .style(surface_style(model))
        .wrap(Wrap { trim: true }),
        rows[2],
    );
}

fn render_input(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    placeholder: &str,
    input: &crate::app::TextInput,
    active: bool,
    model: &Model,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let columns = Layout::horizontal([Constraint::Length(12), Constraint::Min(1)]).split(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {label}"),
            field_label(model, active),
        )))
        .style(surface_style(model)),
        columns[0],
    );
    let input_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active {
            model.ui.theme.accent
        } else {
            model.ui.theme.border
        }))
        .style(Style::default().bg(model.ui.theme.surface_alt));
    let input_inner = input_block.inner(columns[1]);
    frame.render_widget(input_block, columns[1]);
    if input_inner.width == 0 || input_inner.height == 0 {
        return;
    }
    let value = if input.value.is_empty() && !active {
        placeholder
    } else {
        &input.value
    };
    let value_style = if input.value.is_empty() && !active {
        muted_style(model).bg(model.ui.theme.surface_alt)
    } else {
        Style::default()
            .fg(model.ui.theme.text)
            .bg(model.ui.theme.surface_alt)
    };
    frame.render_widget(
        Paragraph::new(truncate_text(value, input_inner.width as usize))
            .style(value_style)
            .wrap(Wrap { trim: false }),
        input_inner,
    );
    if active {
        let cursor_offset = input.value[..input.cursor.min(input.value.len())]
            .chars()
            .count()
            .min(input_inner.width.saturating_sub(1) as usize) as u16;
        frame.set_cursor_position((input_inner.x + cursor_offset, input_inner.y));
    }
}
