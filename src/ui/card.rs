//! A card: a tile on `surface` with a bar in its lane's colour.
//!
//! ```text
//! ▎ Write the README
//! ▎ Cover install, usage, keys…
//! ▎ ◷ 1d ago
//! ```

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::geometry::card_text;
use super::text::relative_time;
use super::{AnimationKind, blend_color, faint, fg, fill, muted, progress, put};
use crate::app::Model;
use crate::clock::Clock;
use crate::domain::Task;

/// Draws a card into `area`, which may be shorter than the card if it is
/// cut off at the bottom of a list.
pub(crate) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    model: &Model,
    column: usize,
    task: &Task,
    selected: bool,
    clock: Clock,
) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    let theme = &model.ui.theme;
    let lane = theme.lane(column);
    let mut background = theme.surface;
    let mut tile = Style::default();
    if selected {
        background = blend_color(
            theme.surface,
            theme.selection,
            progress(model, AnimationKind::Selection, clock),
        );
        // A card that was just moved or saved glows in its lane colour.
        if let Some(glow) = model
            .ui
            .animations
            .progress(AnimationKind::CardMove, clock.instant)
        {
            let start = blend_color(theme.surface, lane, 0.35);
            background = blend_color(start, background, glow);
        }
        tile = tile.add_modifier(theme.selected_modifier);
    }
    fill(frame, area, tile.bg(background));

    let (bar, bar_style) = if selected {
        ("▌", fg(lane).add_modifier(Modifier::BOLD))
    } else {
        ("▎", fg(lane))
    };
    for y in area.top()..area.bottom() {
        put(
            frame,
            area.x,
            y,
            1,
            Line::from(Span::styled(bar, bar_style)),
        );
    }

    let text = card_text(task, area.width);
    let mut lines: Vec<Line> = text
        .title
        .into_iter()
        .map(|title| {
            Line::from(Span::styled(
                title,
                fg(theme.text).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();
    if let Some(description) = text.description {
        lines.push(Line::from(Span::styled(description, muted(model))));
    }
    lines.push(Line::from(Span::styled(
        format!("◷ {}", relative_time(task.updated_at, clock.wall_millis)),
        faint(model),
    )));
    let x = area.x.saturating_add(2);
    let width = area.width.saturating_sub(3);
    for (row, line) in lines.into_iter().enumerate() {
        let y = area.y + row as u16;
        if y >= area.bottom() {
            break;
        }
        put(frame, x, y, width, line);
    }
}
