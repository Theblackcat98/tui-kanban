use super::muted_style;
use super::{blend_color, truncate_text};
use crate::animation::{AnimationKind, ease_out_cubic};
use crate::app::{FocusRegion, Model};
use crate::clock::Clock;
use crate::domain::Task;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

pub(crate) const CARD_HEIGHT: u16 = 6;

pub(crate) fn render_card(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    column_index: usize,
    task: &Task,
    selected: bool,
    clock: Clock,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let focused = model.ui.focus == FocusRegion::Cards;
    let selection_progress = model
        .ui
        .animations
        .progress(AnimationKind::Selection, clock.instant)
        .map(ease_out_cubic)
        .unwrap_or(1.0);
    let accent = column_accent(model, column_index);
    let selected_and_focused = selected && focused;
    let background = if selected_and_focused {
        blend_color(
            model.ui.theme.surface_alt,
            model.ui.theme.selection,
            selection_progress,
        )
    } else {
        model.ui.theme.surface_alt
    };
    let border = if selected_and_focused {
        blend_color(model.ui.theme.border, accent, selection_progress)
    } else {
        model.ui.theme.border
    };
    let selected_modifier = if selected_and_focused {
        model.ui.theme.selected_modifier
    } else {
        Modifier::empty()
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(
            Style::default()
                .bg(background)
                .add_modifier(selected_modifier),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height < 3 {
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let title_style = Style::default()
        .fg(model.ui.theme.text)
        .bg(background)
        .add_modifier(if selected_and_focused {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });
    let detail_style = muted_style(model).bg(background);
    let metadata_style = Style::default().fg(accent).bg(background);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", task.title),
            title_style,
        )))
        .style(title_style)
        .wrap(Wrap { trim: true }),
        rows[0],
    );
    let description = first_description_line(task);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", description),
            detail_style,
        )))
        .style(detail_style)
        .wrap(Wrap { trim: true }),
        rows[1],
    );
    let metadata = format!(
        " {}  •  updated {}",
        task_column_name(model, column_index),
        relative_time(task.updated_at, clock.wall_millis)
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            truncate_text(&metadata, rows[2].width as usize),
            metadata_style,
        )))
        .style(metadata_style),
        rows[2],
    );
}

pub(crate) fn column_accent(model: &Model, index: usize) -> ratatui::style::Color {
    match index {
        0 => model.ui.theme.accent_alt,
        1 => model.ui.theme.warning,
        _ => model.ui.theme.success,
    }
}

pub(crate) fn relative_time(timestamp: i64, now: i64) -> String {
    let elapsed = now.saturating_sub(timestamp).max(0);
    match elapsed {
        0..=59_999 => "just now".to_owned(),
        60_000..=3_599_999 => format!("{}m", elapsed / 60_000),
        3_600_000..=86_399_999 => format!("{}h", elapsed / 3_600_000),
        _ => format!("{}d", elapsed / 86_400_000),
    }
}

fn task_column_name(model: &Model, column_index: usize) -> &str {
    model
        .board
        .columns
        .get(column_index)
        .map(|column| column.name.as_str())
        .unwrap_or("No column")
}

fn first_description_line(task: &Task) -> String {
    task.description
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("No description")
        .to_owned()
}
