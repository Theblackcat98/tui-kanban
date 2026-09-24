use super::{cards, truncate_text};
use crate::app::{App, FocusRegion};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub(crate) const WIDTH: u16 = 20;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.surface)),
        area,
    );
    let heading = Paragraph::new(Line::from(Span::styled(
        " COLUMNS",
        Style::default()
            .fg(app.theme.muted)
            .add_modifier(Modifier::BOLD),
    )))
    .style(Style::default().bg(app.theme.surface));
    frame.render_widget(heading, Rect::new(area.x, area.y, area.width, 1));

    let mut y = area.y.saturating_add(2);
    for (index, column) in app.board.columns.iter().enumerate() {
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        let selected = index == app.selected_column;
        let focused = app.focus == FocusRegion::Rail;
        let background = if selected && focused {
            app.theme.selection
        } else {
            app.theme.surface
        };
        let text_color = if selected {
            app.theme.text
        } else {
            app.theme.muted
        };
        let marker = if selected { "▌" } else { " " };
        let visible = app.visible_task_indices(index);
        let count = if app.search_query.is_empty() {
            column.tasks.len().to_string()
        } else {
            format!("{}/{}", visible.len(), column.tasks.len())
        };
        let count_width = (area.width as usize).saturating_sub(5).min(8);
        let count_area = Rect::new(
            area.x
                .saturating_add(area.width.saturating_sub(count_width as u16 + 1)),
            y,
            count_width as u16,
            1,
        );
        let name_area = Rect::new(
            area.x.saturating_add(1),
            y,
            area.width.saturating_sub(count_width as u16 + 2),
            1,
        );
        let name = truncate_text(
            &format!("{marker} {}", column.name),
            name_area.width as usize,
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                name,
                Style::default().fg(text_color).bg(background).add_modifier(
                    if selected && focused {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                ),
            )))
            .style(Style::default().fg(text_color).bg(background)),
            name_area,
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                truncate_text(&count, count_area.width as usize),
                Style::default()
                    .fg(cards::column_accent(app, index))
                    .bg(background),
            )))
            .style(Style::default().bg(background)),
            count_area,
        );
        y = y.saturating_add(1);
    }

    if app.board.columns.is_empty() {
        frame.render_widget(
            Paragraph::new(" No columns")
                .style(Style::default().fg(app.theme.muted).bg(app.theme.surface)),
            Rect::new(area.x, area.y.saturating_add(2), area.width, 1),
        );
    }

    if area.width > 1 {
        frame.render_widget(
            Paragraph::new("│").style(Style::default().fg(app.theme.border).bg(app.theme.surface)),
            Rect::new(
                area.x.saturating_add(area.width - 1),
                area.y,
                1,
                area.height,
            ),
        );
    }
}
