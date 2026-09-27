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
use crate::markdown;
use unicode_segmentation::UnicodeSegmentation;

/// A line with the graphemes at `matched` in `highlight`.
fn highlighted(text: &str, matched: &[usize], style: Style, highlight: Style) -> Line<'static> {
    if matched.is_empty() {
        return Line::from(Span::styled(text.to_owned(), style));
    }
    let mut spans: Vec<Span> = Vec::new();
    let mut run = String::new();
    let mut run_matched = false;
    for (index, grapheme) in text.graphemes(true).enumerate() {
        let is_match = matched.contains(&index);
        if is_match != run_matched && !run.is_empty() {
            let run_style = if run_matched {
                style.patch(highlight)
            } else {
                style
            };
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        run_matched = is_match;
        run.push_str(grapheme);
    }
    if !run.is_empty() {
        let run_style = if run_matched {
            style.patch(highlight)
        } else {
            style
        };
        spans.push(Span::styled(run, run_style));
    }
    Line::from(spans)
}

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
    let lane = super::lane_color(model, column);
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
    let filter = &model.ui.search.filter;
    let highlight = if theme.is_monochrome() {
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        fg(theme.accent).add_modifier(Modifier::BOLD)
    };
    let title_style = fg(theme.text).add_modifier(Modifier::BOLD);
    // Matches are found in the title with its spaces collapsed, which is
    // what the wrapped lines are cut from.
    let normalised = task.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title_matches = if filter.is_empty() {
        Vec::new()
    } else {
        filter.title_highlights(&normalised)
    };
    let graphemes: Vec<&str> = normalised.graphemes(true).collect();
    let mut offset = 0;
    let mut lines: Vec<Line> = Vec::new();
    for title in text.title {
        let count = title.graphemes(true).count();
        let matched: Vec<usize> = title_matches
            .iter()
            .filter(|index| (offset..offset + count).contains(*index))
            .map(|index| index - offset)
            .collect();
        lines.push(highlighted(&title, &matched, title_style, highlight));
        offset += count;
        // A line break at a space drops that space.
        if graphemes.get(offset) == Some(&" ") {
            offset += 1;
        }
    }
    if let Some(description) = text.description {
        let matched = if filter.is_empty() {
            Vec::new()
        } else {
            filter.description_highlights(&description)
        };
        lines.push(highlighted(&description, &matched, muted(model), highlight));
    }
    let mut meta = vec![Span::styled(
        format!("◷ {}", relative_time(task.updated_at, clock.wall_millis)),
        faint(model),
    )];
    // Checklist progress, as "✓ 2/5", in `success` once all are done.
    if let Some((done, total)) = markdown::progress(&task.description) {
        let style = if done == total {
            fg(theme.success)
        } else {
            faint(model)
        };
        meta.push(Span::styled(format!("   ✓ {done}/{total}"), style));
    }
    lines.push(Line::from(meta));
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
