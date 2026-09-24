use super::{cards, text_style, truncate_text};
use crate::app::{App, ViewMode};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use std::time::Instant;

const ALL_TASK_MIN_CARD_WIDTH: u16 = 30;
const ALL_TASK_MAX_CARD_COLUMNS: u16 = 3;
const CARD_GAP: u16 = 1;

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

    match app.view_mode {
        ViewMode::Board => render_board(frame, area, app, now),
        ViewMode::AllTasks => render_all_tasks(frame, area, app, now),
    }
}

fn render_board(frame: &mut Frame<'_>, area: Rect, app: &App, now: Instant) {
    if area.width < 90 {
        render_column(frame, area, app, app.selected_column, now);
        return;
    }

    let count = app.board.columns.len() as u16;
    let available = area
        .width
        .saturating_sub(CARD_GAP * count.saturating_sub(1));
    let base_width = available / count;
    let remainder = available % count;
    let mut x = area.x;
    for index in 0..app.board.columns.len() {
        let width = base_width + u16::from((index as u16) < remainder);
        let column_area = Rect::new(x, area.y, width, area.height);
        render_column(frame, column_area, app, index, now);
        x = x.saturating_add(width).saturating_add(CARD_GAP);
    }
}

fn render_column(frame: &mut Frame<'_>, area: Rect, app: &App, column_index: usize, now: Instant) {
    let Some(column) = app.board.columns.get(column_index) else {
        return;
    };
    let focused =
        app.focus == crate::app::FocusRegion::Cards && app.selected_column == column_index;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused {
            cards::column_accent(app, column_index)
        } else {
            app.theme.border
        }))
        .style(Style::default().bg(app.theme.surface));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible = app.visible_task_indices(column_index);
    let total = column.tasks.len();
    let count_text = if app.search_query.is_empty() {
        total.to_string()
    } else {
        format!("{} / {total}", visible.len())
    };
    let heading = Line::from(vec![
        Span::styled(
            format!(" {} ", truncate_text(&column.name, inner.width as usize)),
            Style::default()
                .fg(if focused {
                    app.theme.text
                } else {
                    app.theme.muted
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {count_text}"),
            Style::default().fg(if focused {
                cards::column_accent(app, column_index)
            } else {
                app.theme.muted
            }),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(heading).style(Style::default().bg(app.theme.surface)),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let list_area = Rect::new(
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        inner.height.saturating_sub(1),
    );
    if list_area.width == 0 || list_area.height == 0 {
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

    let card_height = cards::CARD_HEIGHT.min(list_area.height);
    let capacity = (list_area.height / cards::CARD_HEIGHT).max(1) as usize;
    let selected_visual = if focused {
        app.selected_task_visual_index(column_index)
    } else {
        0
    };
    let start = if selected_visual < capacity / 2 {
        0
    } else {
        selected_visual.saturating_sub(capacity / 2)
    }
    .min(visible.len().saturating_sub(capacity));
    let end = (start + capacity).min(visible.len());
    let selected_id = app.selected_task_id();
    for (offset, task_index) in visible[start..end].iter().enumerate() {
        let Some(task) = column.tasks.get(*task_index) else {
            continue;
        };
        let card_area = Rect::new(
            list_area.x,
            list_area
                .y
                .saturating_add((offset as u16).saturating_mul(card_height)),
            list_area.width,
            card_height,
        );
        cards::render_card(
            frame,
            card_area,
            app,
            column_index,
            task,
            focused && selected_id == Some(task.id),
            now,
        );
    }
}

struct AllTaskSection {
    column: usize,
    visible: Vec<usize>,
    start: usize,
}

fn render_all_tasks(frame: &mut Frame<'_>, area: Rect, app: &App, now: Instant) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let per_row = all_task_card_columns(area.width);
    let mut sections = Vec::with_capacity(app.board.columns.len());
    let mut line = 0usize;
    for (column_index, _column) in app.board.columns.iter().enumerate() {
        let visible = app.visible_task_indices(column_index);
        let height = if visible.is_empty() {
            3
        } else {
            let rows = visible.len().div_ceil(per_row as usize);
            1 + rows * (cards::CARD_HEIGHT as usize + 1)
        };
        sections.push(AllTaskSection {
            column: column_index,
            visible,
            start: line,
        });
        line += height;
    }

    let viewport = area.height as usize;
    let mut scroll = app.all_tasks_scroll as usize;
    let selected_line = selected_line(&sections, app, per_row);
    if let Some(selected_line) = selected_line {
        if selected_line < scroll {
            scroll = selected_line;
        } else if selected_line.saturating_add(cards::CARD_HEIGHT as usize)
            > scroll.saturating_add(viewport)
        {
            scroll = selected_line + cards::CARD_HEIGHT as usize - viewport;
        }
    }
    if viewport < line {
        scroll = scroll.min(line - viewport);
    } else {
        scroll = 0;
    }

    for section in sections {
        render_all_section(frame, area, app, &section, per_row, scroll, now);
    }
}

fn render_all_section(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    section: &AllTaskSection,
    per_row: u16,
    scroll: usize,
    now: Instant,
) {
    let Some(column) = app.board.columns.get(section.column) else {
        return;
    };
    let heading_line = section.start;
    if heading_line >= scroll && heading_line < scroll.saturating_add(area.height as usize) {
        let y = area.y.saturating_add((heading_line - scroll) as u16);
        let count_text = if app.search_query.is_empty() {
            section.visible.len().to_string()
        } else {
            format!("{} / {}", section.visible.len(), column.tasks.len())
        };
        let heading = Line::from(vec![
            Span::styled(
                format!(" {} ", truncate_text(&column.name, area.width as usize)),
                Style::default()
                    .fg(cards::column_accent(app, section.column))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {count_text} tasks"),
                Style::default().fg(app.theme.muted),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(heading)
                .style(Style::default().bg(app.theme.background))
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, y, area.width, 1),
        );
    }

    if section.visible.is_empty() {
        let line = section.start.saturating_add(1);
        if line >= scroll && line < scroll.saturating_add(area.height as usize) {
            let y = area.y.saturating_add((line - scroll) as u16);
            let message = if app.search_query.is_empty() {
                "No tasks"
            } else {
                "No matching tasks"
            };
            frame.render_widget(
                Paragraph::new(format!("  {message}")).style(
                    Style::default()
                        .fg(app.theme.muted)
                        .bg(app.theme.background),
                ),
                Rect::new(area.x, y, area.width, 1),
            );
        }
        return;
    }

    let available = area
        .width
        .saturating_sub(CARD_GAP * per_row.saturating_sub(1));
    let base_width = available / per_row;
    let remainder = available % per_row;
    let selected_id = app.selected_task_id();
    for (row, row_tasks) in section.visible.chunks(per_row as usize).enumerate() {
        let line = section
            .start
            .saturating_add(1 + row * (cards::CARD_HEIGHT as usize + 1));
        if line < scroll || line >= scroll.saturating_add(area.height as usize) {
            continue;
        }
        let y = area.y.saturating_add((line - scroll) as u16);
        let mut x = area.x;
        for (card_index, task_index) in row_tasks.iter().enumerate() {
            let Some(task) = column.tasks.get(*task_index) else {
                continue;
            };
            let width = base_width + u16::from((card_index as u16) < remainder);
            let card_area = Rect::new(x, y, width, cards::CARD_HEIGHT);
            cards::render_card(
                frame,
                card_area,
                app,
                section.column,
                task,
                selected_id == Some(task.id),
                now,
            );
            x = x.saturating_add(width).saturating_add(CARD_GAP);
        }
    }
}

fn selected_line(sections: &[AllTaskSection], app: &App, per_row: u16) -> Option<usize> {
    let selected_id = app.selected_task_id()?;
    for section in sections {
        let Some(column) = app.board.columns.get(section.column) else {
            continue;
        };
        let Some(position) = section.visible.iter().position(|task_index| {
            column
                .tasks
                .get(*task_index)
                .is_some_and(|task| task.id == selected_id)
        }) else {
            continue;
        };
        return Some(section.start.saturating_add(
            1 + (position / per_row as usize) * (cards::CARD_HEIGHT as usize + 1),
        ));
    }
    None
}

fn all_task_card_columns(width: u16) -> u16 {
    let width = width.max(1);
    let columns = (width + CARD_GAP) / (ALL_TASK_MIN_CARD_WIDTH + CARD_GAP);
    columns.clamp(1, ALL_TASK_MAX_CARD_COLUMNS)
}
