use super::{blend_color, surface_style, text_style, truncate_text};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, Paragraph, Wrap};
use std::time::Instant;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, app: &App, now: Instant) {
    if app.board.columns.is_empty() {
        frame.render_widget(
            Paragraph::new("This board has no columns.")
                .style(text_style(app))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    if area.width < 90 {
        render_column(frame, area, app, app.selected_column, now);
        return;
    }

    let count = app.board.columns.len() as u16;
    let gap = 1;
    let available = area.width.saturating_sub(gap * count.saturating_sub(1));
    let base_width = available / count;
    let remainder = available % count;
    let mut x = area.x;
    for index in 0..app.board.columns.len() {
        let width = base_width + u16::from((index as u16) < remainder);
        let column_area = Rect::new(x, area.y, width, area.height);
        render_column(frame, column_area, app, index, now);
        x += width + gap;
    }
}

fn render_column(frame: &mut Frame<'_>, area: Rect, app: &App, column_index: usize, now: Instant) {
    let Some(column) = app.board.columns.get(column_index) else {
        return;
    };
    let focused = app.selected_column == column_index;
    let accent = column_accent(app, column_index);
    let selection_progress = app
        .animations
        .progress(AnimationKind::Selection, now)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let border_color = if focused {
        blend_color(app.theme.border, accent, selection_progress)
    } else {
        app.theme.border
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.surface));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let visible = app.visible_task_indices(column_index);
    let total = column.tasks.len();
    let count_text = if app.search_query.is_empty() {
        format!("{total}")
    } else {
        format!("{} / {total}", visible.len())
    };
    let heading = Line::from(vec![
        Span::styled(
            format!(" {} ", column.name),
            Style::default()
                .fg(if focused {
                    app.theme.text
                } else {
                    app.theme.muted
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{count_text} "),
            Style::default().fg(if focused { accent } else { app.theme.muted }),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(heading).style(Style::default().bg(app.theme.surface)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(1),
    );
    if list_area.height == 0 {
        return;
    }
    if visible.is_empty() {
        let message = if app.search_query.is_empty() {
            "No cards yet\n\nPress n to create one"
        } else {
            "No matching cards\n\nPress esc to clear search"
        };
        frame.render_widget(
            Paragraph::new(Text::from(message))
                .style(Style::default().fg(app.theme.muted).bg(app.theme.surface))
                .wrap(Wrap { trim: true }),
            list_area,
        );
        return;
    }

    let card_height = 3usize;
    let capacity = (list_area.height as usize / card_height).max(1);
    let selected_visual = app.selected_task_visual_index(column_index);
    let start = if selected_visual < capacity / 2 {
        0
    } else {
        selected_visual.saturating_sub(capacity / 2)
    }
    .min(visible.len().saturating_sub(capacity));
    let end = (start + capacity).min(visible.len());
    let mut items = Vec::with_capacity(end - start);
    for task_index in &visible[start..end] {
        let Some(task) = column.tasks.get(*task_index) else {
            continue;
        };
        let selected = focused && *task_index == app.selected_task;
        let move_progress = app
            .animations
            .progress(AnimationKind::CardMove, now)
            .map(ease_out_cubic)
            .unwrap_or(0.0);
        let selected_background = if selected {
            let base = blend_color(app.theme.surface, app.theme.selection, selection_progress);
            blend_color(base, accent, move_progress * 0.45)
        } else {
            app.theme.surface
        };
        let title_style = if selected {
            Style::default()
                .fg(app.theme.text)
                .bg(selected_background)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text).bg(app.theme.surface)
        };
        let detail_style = if selected {
            Style::default().fg(app.theme.text).bg(selected_background)
        } else {
            Style::default().fg(app.theme.muted).bg(app.theme.surface)
        };
        let description = task
            .description
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("No description");
        let width = list_area.width.saturating_sub(5) as usize;
        let title = truncate_text(&task.title, width);
        let detail = truncate_text(description, width);
        items.push(ListItem::new(Text::from(vec![
            Line::from(Span::styled(format!(" {title}"), title_style)),
            Line::from(Span::styled(format!("   {detail}"), detail_style)),
            Line::from(Span::styled(" ", detail_style)),
        ])));
    }

    let move_highlight = app
        .animations
        .progress(AnimationKind::CardMove, now)
        .map(ease_out_cubic)
        .unwrap_or(0.0);
    let list = List::new(items)
        .style(surface_style(app))
        .highlight_style(
            Style::default()
                .bg(if focused {
                    let base =
                        blend_color(app.theme.surface, app.theme.selection, selection_progress);
                    blend_color(base, accent, move_highlight * 0.45)
                } else {
                    app.theme.selection
                })
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌ ");
    let mut state = ListState::default();
    if focused && selected_visual >= start && selected_visual < end {
        state.select(Some(selected_visual - start));
    }
    frame.render_stateful_widget(list, list_area, &mut state);

    if app.search_active {
        let search_width = list_area.width.saturating_sub(2);
        if search_width > 0 {
            frame.render_widget(
                Paragraph::new(format!(" /{}", app.search_query)).style(
                    Style::default()
                        .fg(app.theme.accent)
                        .bg(app.theme.surface_alt),
                ),
                Rect::new(list_area.x, list_area.y, search_width, 1),
            );
        }
    }
}

fn column_accent(app: &App, index: usize) -> ratatui::style::Color {
    match index {
        0 => app.theme.accent_alt,
        1 => app.theme.warning,
        _ => app.theme.success,
    }
}
