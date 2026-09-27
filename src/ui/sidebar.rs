use super::muted_style;
use super::{cards, truncate_text};
use crate::app::{FocusRegion, Model};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

pub(crate) const WIDTH: u16 = 20;

pub(crate) fn render(frame: &mut Frame<'_>, area: Rect, model: &Model) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(
        Block::default().style(Style::default().bg(model.ui.theme.surface)),
        area,
    );
    let heading = Paragraph::new(Line::from(Span::styled(
        " COLUMNS",
        muted_style(model).add_modifier(Modifier::BOLD),
    )))
    .style(Style::default().bg(model.ui.theme.surface));
    frame.render_widget(heading, Rect::new(area.x, area.y, area.width, 1));

    let mut y = area.y.saturating_add(2);
    for (index, column) in model.board.columns.iter().enumerate() {
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        let selected = index == model.ui.active_column;
        let focused = model.ui.focus == FocusRegion::Rail;
        let background = if selected && focused {
            model.ui.theme.selection
        } else {
            model.ui.theme.surface
        };
        let text_color = if selected {
            model.ui.theme.text
        } else {
            model.ui.theme.muted
        };
        let marker = if selected { "▌" } else { " " };
        let visible = model.visible_task_indices(index);
        let count = if model.ui.search.query.is_empty() {
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
                        Modifier::BOLD | model.ui.theme.selected_modifier
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
                    .fg(cards::column_accent(model, index))
                    .bg(background),
            )))
            .style(Style::default().bg(background)),
            count_area,
        );
        y = y.saturating_add(1);
    }

    if model.board.columns.is_empty() {
        frame.render_widget(
            Paragraph::new(" No columns").style(muted_style(model).bg(model.ui.theme.surface)),
            Rect::new(area.x, area.y.saturating_add(2), area.width, 1),
        );
    }

    if area.width > 1 {
        frame.render_widget(
            Paragraph::new("│").style(
                Style::default()
                    .fg(model.ui.theme.border)
                    .bg(model.ui.theme.surface),
            ),
            Rect::new(
                area.x.saturating_add(area.width - 1),
                area.y,
                1,
                area.height,
            ),
        );
    }
}
