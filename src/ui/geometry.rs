//! Where everything goes on screen, as pure functions of the model and the
//! terminal area. The renderer draws into these rectangles, and `update`
//! uses the same functions to keep scroll positions valid, so what is drawn
//! and what is scrolled always agree.

use ratatui::layout::Rect;
use uuid::Uuid;

use super::rich::{self, RichLine};
use super::text::{truncate_text, wrap};
use crate::app::{Model, Screen, ViewMode};
use crate::domain::Task;
use crate::layout::{self, Breakpoint, CARD_GAP, LANE_GUTTER, PAGE_MARGIN, RAIL_WIDTH};

/// The page's regions.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Page {
    pub breakpoint: Breakpoint,
    pub top: Rect,
    pub rail: Option<Rect>,
    /// Where the lanes or the All tasks list go.
    pub main: Rect,
    /// Where the detail drawer goes, while one is open.
    pub drawer: Option<Rect>,
    /// The first-run tip, above the status line, while it shows.
    pub tip: Option<Rect>,
    pub status: Rect,
}

pub(crate) fn page(model: &Model, area: Rect) -> Page {
    let breakpoint = Breakpoint::from_width(area.width);
    let top = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    let status = Rect::new(
        area.x,
        area.bottom().saturating_sub(1),
        area.width,
        1.min(area.height),
    );
    let tip_rows = u16::from(model.ui.tip);
    let tip = model.ui.tip.then(|| {
        Rect::new(
            area.x,
            area.bottom().saturating_sub(2),
            area.width,
            1.min(area.height),
        )
    });
    let body = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(2 + tip_rows),
    );
    let (rail, rest) = if breakpoint.shows_rail() {
        let (rail, rest) = split_left(body, RAIL_WIDTH);
        (Some(rail), rest)
    } else {
        (None, body)
    };
    let detail_open = model
        .ui
        .screens
        .iter()
        .any(|screen| matches!(screen, Screen::Detail { .. }));
    let (main, drawer) = match breakpoint {
        _ if !detail_open => (rest, None),
        Breakpoint::Compact => (rest, Some(body)),
        Breakpoint::Regular => {
            let width = (body.width * 2 / 5).clamp(40, 60).min(body.width);
            (rest, Some(split_right(body, width).1))
        }
        Breakpoint::Wide => {
            let width = layout::PINNED_DRAWER_WIDTH.min(rest.width / 2);
            let (main, drawer) = split_right(rest, width);
            (main, Some(drawer))
        }
    };
    Page {
        breakpoint,
        top,
        rail,
        main,
        drawer,
        tip,
        status,
    }
}

fn split_left(area: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(area.width);
    (
        Rect::new(area.x, area.y, width, area.height),
        Rect::new(area.x + width, area.y, area.width - width, area.height),
    )
}

fn split_right(area: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(area.width);
    (
        Rect::new(area.x, area.y, area.width - width, area.height),
        Rect::new(area.right() - width, area.y, width, area.height),
    )
}

/// `main` without the page margins, and a blank row at the top.
fn content(main: Rect) -> Rect {
    Rect::new(
        main.x.saturating_add(PAGE_MARGIN),
        main.y.saturating_add(1),
        main.width.saturating_sub(2 * PAGE_MARGIN),
        main.height.saturating_sub(1),
    )
}

/// Splits `width` into `count` parts separated by `gap`, spreading the
/// remainder over the first parts. Returns each part's (x offset, width).
pub(crate) fn split_evenly(width: u16, count: u16, gap: u16) -> Vec<(u16, u16)> {
    if count == 0 {
        return Vec::new();
    }
    let available = width.saturating_sub(gap * (count - 1));
    let base = available / count;
    let remainder = available % count;
    let mut x = 0;
    (0..count)
        .map(|index| {
            let part = base + u16::from(index < remainder);
            let result = (x, part);
            x += part + gap;
            result
        })
        .collect()
}

/// The lanes shown in the Board view, as (column index, area).
pub(crate) fn lanes(model: &Model, page: &Page) -> Vec<(usize, Rect)> {
    let area = content(page.main);
    let range = lane_range(model, page);
    split_evenly(area.width, range.len() as u16, LANE_GUTTER)
        .into_iter()
        .zip(range)
        .map(|((x, width), column)| (column, Rect::new(area.x + x, area.y, width, area.height)))
        .collect()
}

/// The columns shown as lanes: as many as fit at [`layout::LANE_MIN_WIDTH`] or
/// wider (one at the Compact breakpoint), starting from the remembered
/// first lane and scrolled sideways just enough to show the active one.
pub(crate) fn lane_range(model: &Model, page: &Page) -> std::ops::Range<usize> {
    let columns = model.board.columns.len();
    if columns == 0 {
        return 0..0;
    }
    let count = if page.breakpoint.shows_several_lanes() {
        layout::lanes_that_fit(content(page.main).width).min(columns)
    } else {
        1
    };
    let active = model.ui.active_column.min(columns - 1);
    let mut first = model.ui.scroll.first_lane.min(columns - count);
    if active < first {
        first = active;
    } else if active >= first + count {
        first = active + 1 - count;
    }
    first..first + count
}

/// The row above the lanes, where "‹ N more" and "N more ›" go.
pub(crate) fn lane_overflow_row(page: &Page) -> Rect {
    let area = content(page.main);
    Rect::new(area.x, page.main.y, area.width, 1.min(page.main.height))
}

/// The rows of a lane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LaneParts {
    pub header: Rect,
    pub underline: Rect,
    /// The "↑ N more" row.
    pub above: Rect,
    pub cards: Rect,
    /// The "↓ N more" row.
    pub below: Rect,
}

pub(crate) fn lane_parts(area: Rect) -> LaneParts {
    let row = |offset: u16| {
        Rect::new(
            area.x,
            area.y.saturating_add(offset),
            area.width,
            u16::from(offset < area.height),
        )
    };
    let cards_height = area.height.saturating_sub(4);
    LaneParts {
        header: row(0),
        underline: row(1),
        above: row(2),
        cards: Rect::new(area.x, area.y.saturating_add(3), area.width, cards_height),
        below: Rect::new(
            area.x,
            area.y.saturating_add(3 + cards_height),
            area.width,
            u16::from(area.height >= 4),
        ),
    }
}

/// What a card shows, fitted to its width.
#[derive(Clone, Debug)]
pub(crate) struct CardText {
    /// One or two lines, the last ending in "…" if the title was cut.
    pub title: Vec<String>,
    /// The first non-empty line of the description, cut to fit.
    pub description: Option<String>,
}

impl CardText {
    /// Title, description and a metadata row.
    pub fn height(&self) -> u16 {
        self.title.len() as u16 + u16::from(self.description.is_some()) + 1
    }
}

/// The text width inside a card: the bar and a cell of padding on the
/// left, a cell of padding on the right.
pub(crate) fn card_text_width(card_width: u16) -> usize {
    card_width.saturating_sub(3) as usize
}

pub(crate) fn card_text(task: &Task, card_width: u16) -> CardText {
    let width = card_text_width(card_width);
    let mut title = wrap(&task.title, width, 2);
    if title.is_empty() {
        title.push(String::new());
    }
    let description = rich::preview(&task.description).map(|line| truncate_text(&line, width));
    CardText { title, description }
}

/// The heights of a column's visible cards at this width.
pub(crate) fn lane_heights(model: &Model, column: usize, width: u16) -> Vec<u16> {
    let tasks = &model.board.columns[column].tasks;
    model
        .visible_task_indices(column)
        .into_iter()
        .map(|index| card_text(&tasks[index], width).height())
        .collect()
}

fn span(heights: &[u16]) -> u32 {
    heights.iter().map(|height| u32::from(*height)).sum::<u32>()
        + u32::from(CARD_GAP) * heights.len().saturating_sub(1) as u32
}

/// The first item to show so that the items `target.0..=target.1` are
/// visible, moving as little as possible from `offset`, and without
/// leaving empty space at the end when earlier items would fit there.
/// Items are `heights` tall, separated by [`CARD_GAP`] rows.
pub(crate) fn fit_offset(
    heights: &[u16],
    available: u16,
    offset: usize,
    target: Option<(usize, usize)>,
) -> usize {
    if heights.is_empty() {
        return 0;
    }
    let available = u32::from(available);
    let last = heights.len() - 1;
    let mut offset = offset.min(last);
    if let Some((low, high)) = target {
        let high = high.min(last);
        let low = low.min(high);
        if low < offset {
            offset = low;
        }
        while offset < high && span(&heights[offset..=high]) > available {
            offset += 1;
        }
    }
    while offset > 0 && span(&heights[offset - 1..]) <= available {
        offset -= 1;
    }
    offset
}

/// The end (exclusive) of the items shown from `offset`: whole items only,
/// except that the first one is always shown, cut if it has to be.
pub(crate) fn visible_end(heights: &[u16], available: u16, offset: usize) -> usize {
    if offset >= heights.len() {
        return heights.len();
    }
    let mut end = offset + 1;
    while end < heights.len() && span(&heights[offset..=end]) <= u32::from(available) {
        end += 1;
    }
    end
}

/// The first visible card of a lane: its remembered scroll position,
/// moved if needed to show the selected card when the lane is active.
pub(crate) fn lane_offset(model: &Model, column: usize, area: Rect) -> usize {
    let heights = lane_heights(model, column, area.width);
    let target = if column == model.ui.active_column {
        model
            .selected_visual_index(column)
            .map(|index| (index, index))
    } else {
        None
    };
    fit_offset(
        &heights,
        lane_parts(area).cards.height,
        model.ui.scroll.lane(column),
        target,
    )
}

/// One entry in the All tasks list.
#[derive(Clone, Debug)]
pub(crate) enum Item {
    /// A column's name and underline.
    Header { column: usize },
    /// A row of cards: task indices within the column.
    Row { column: usize, tasks: Vec<usize> },
}

pub(crate) const HEADER_HEIGHT: u16 = 2;

#[derive(Clone, Debug)]
pub(crate) struct AllTasks {
    /// The "↑ N more" row.
    pub above: Rect,
    pub list: Rect,
    /// The "↓ N more" row.
    pub below: Rect,
    pub per_row: u16,
    pub items: Vec<Item>,
    pub heights: Vec<u16>,
}

impl AllTasks {
    /// Each card in a row's (x offset, width).
    pub fn card_columns(&self) -> Vec<(u16, u16)> {
        split_evenly(self.list.width, self.per_row, LANE_GUTTER)
    }

    /// How many cards the items in `range` hold.
    pub fn card_count(&self, range: std::ops::Range<usize>) -> usize {
        self.items[range]
            .iter()
            .map(|item| match item {
                Item::Header { .. } => 0,
                Item::Row { tasks, .. } => tasks.len(),
            })
            .sum()
    }
}

/// The All tasks list: a section for each column with visible tasks.
pub(crate) fn all_tasks(model: &Model, main: Rect) -> AllTasks {
    let area = Rect::new(
        main.x.saturating_add(PAGE_MARGIN),
        main.y,
        main.width.saturating_sub(2 * PAGE_MARGIN),
        main.height,
    );
    let above = Rect::new(area.x, area.y, area.width, 1.min(area.height));
    let list_height = area.height.saturating_sub(2);
    let list = Rect::new(area.x, area.y.saturating_add(1), area.width, list_height);
    let below = Rect::new(
        area.x,
        area.y.saturating_add(1 + list_height),
        area.width,
        u16::from(area.height >= 2),
    );
    let per_row = layout::cards_per_row(list.width);
    let widths = split_evenly(list.width, per_row, LANE_GUTTER);
    let mut items = Vec::new();
    let mut heights = Vec::new();
    for (column, data) in model.board.columns.iter().enumerate() {
        let visible = model.visible_task_indices(column);
        if visible.is_empty() {
            continue;
        }
        items.push(Item::Header { column });
        heights.push(HEADER_HEIGHT);
        for row in visible.chunks(per_row as usize) {
            let height = row
                .iter()
                .zip(&widths)
                .map(|(task, (_, width))| card_text(&data.tasks[*task], *width).height())
                .max()
                .unwrap_or(1);
            items.push(Item::Row {
                column,
                tasks: row.to_vec(),
            });
            heights.push(height);
        }
    }
    AllTasks {
        above,
        list,
        below,
        per_row,
        items,
        heights,
    }
}

/// The All tasks grid as the user sees it: each row's task ids, top to
/// bottom, so the keys can move through it spatially.
pub(crate) fn all_tasks_rows(model: &Model) -> Vec<Vec<Uuid>> {
    let (width, height) = model.ui.viewport;
    let page = page(model, Rect::new(0, 0, width, height));
    all_tasks(model, page.main)
        .items
        .into_iter()
        .filter_map(|item| match item {
            Item::Row { column, tasks } => Some(
                tasks
                    .into_iter()
                    .map(|task| model.board.columns[column].tasks[task].id)
                    .collect(),
            ),
            Item::Header { .. } => None,
        })
        .collect()
}

/// The first visible All tasks item: the remembered scroll position,
/// moved if needed to show the selected card (and its column's header,
/// when it is in the first row).
pub(crate) fn all_tasks_offset(model: &Model, list: &AllTasks) -> usize {
    let target = model.selected_task_id().and_then(|id| {
        let (column, index) = model.board.task_location(id)?;
        let row = list.items.iter().position(|item| {
            matches!(item, Item::Row { column: c, tasks } if *c == column && tasks.contains(&index))
        })?;
        let low = match row.checked_sub(1).map(|previous| &list.items[previous]) {
            Some(Item::Header { .. }) => row - 1,
            _ => row,
        };
        Some((low, row))
    });
    fit_offset(
        &list.heights,
        list.list.height,
        model.ui.scroll.all_tasks,
        target,
    )
}

/// The parts of the detail drawer, relative to the drawer's own area.
#[derive(Clone, Debug)]
pub(crate) struct Detail {
    pub title: Vec<String>,
    pub title_area: Rect,
    /// Two rows: where the task is, then when it changed.
    pub meta: Rect,
    pub description: Vec<RichLine>,
    pub body: Rect,
    /// The column the scrollbar goes in.
    pub scrollbar: Rect,
}

impl Detail {
    pub fn max_scroll(&self) -> u16 {
        (self.description.len() as u16).saturating_sub(self.body.height)
    }

    /// The lines checklist item `item` takes up.
    pub fn item_lines(&self, item: usize) -> std::ops::Range<usize> {
        let first = self
            .description
            .iter()
            .position(|line| line.item == Some(item));
        match first {
            Some(first) => {
                let count = self.description[first..]
                    .iter()
                    .take_while(|line| line.item == Some(item))
                    .count();
                first..first + count
            }
            None => 0..0,
        }
    }
}

pub(crate) fn detail(task: &Task, drawer: Rect) -> Detail {
    // The accent edge and a cell of padding on the left; padding and the
    // scrollbar on the right.
    let x = drawer.x.saturating_add(3);
    let width = drawer.width.saturating_sub(6);
    let top = drawer.y.saturating_add(1);
    let height = drawer.height.saturating_sub(2);
    let max_title_lines = (height / 3).max(1) as usize;
    let mut title = wrap(&task.title, width as usize, max_title_lines);
    if title.is_empty() {
        title.push(String::new());
    }
    let title_height = (title.len() as u16).min(height);
    let meta_y = top + title_height + 1;
    let body_y = meta_y + 3;
    let body_height = drawer.bottom().saturating_sub(1).saturating_sub(body_y);
    Detail {
        title_area: Rect::new(x, top, width, title_height),
        meta: Rect::new(
            x,
            meta_y,
            width,
            drawer.bottom().saturating_sub(meta_y).min(2),
        ),
        description: rich::lines(&task.description, width as usize),
        body: Rect::new(x, body_y, width, body_height),
        scrollbar: Rect::new(
            drawer.right().saturating_sub(2),
            body_y,
            1.min(drawer.width),
            body_height,
        ),
        title,
    }
}

/// Brings the model's scroll positions in line with the current layout:
/// the active lane or the All tasks list scrolls to the selection, and the
/// detail drawer can't scroll past its text.
pub(crate) fn sync_scroll(model: &mut Model) {
    let (width, height) = model.ui.viewport;
    if !layout::fits(width, height) {
        return;
    }
    let page = page(model, Rect::new(0, 0, width, height));
    match model.ui.view {
        ViewMode::Board => {
            model.ui.scroll.first_lane = lane_range(model, &page).start;
            for (column, area) in lanes(model, &page) {
                let offset = lane_offset(model, column, area);
                model.ui.scroll.set_lane(column, offset);
            }
        }
        ViewMode::AllTasks => {
            let list = all_tasks(model, page.main);
            model.ui.scroll.all_tasks = all_tasks_offset(model, &list);
        }
    }
    if let Some(drawer) = page.drawer {
        let limits: Vec<Option<u16>> = model
            .ui
            .screens
            .iter()
            .map(|screen| match screen {
                Screen::Detail { task, .. } => Some(detail_max_scroll(model, *task, drawer)),
                _ => None,
            })
            .collect();
        for (screen, limit) in model.ui.screens.iter_mut().zip(limits) {
            if let (Screen::Detail { scroll, .. }, Some(limit)) = (screen, limit) {
                *scroll = (*scroll).min(limit);
            }
        }
    }
}

/// Scrolls the detail drawer on top so its focused checklist item is in
/// view.
pub(crate) fn reveal_detail_item(model: &mut Model) {
    let (width, height) = model.ui.viewport;
    if !layout::fits(width, height) {
        return;
    }
    let Some(drawer) = page(model, Rect::new(0, 0, width, height)).drawer else {
        return;
    };
    let Some(&Screen::Detail { task, item, .. }) = model.ui.screens.last() else {
        return;
    };
    let Some(task) = model.board.task(task) else {
        return;
    };
    let layout = detail(task, drawer);
    let lines = layout.item_lines(item);
    let visible = layout.body.height as usize;
    if let Some(Screen::Detail { scroll, .. }) = model.ui.screens.last_mut() {
        let top = *scroll as usize;
        if lines.start < top {
            *scroll = lines.start as u16;
        } else if lines.end > top + visible {
            *scroll = lines.end.saturating_sub(visible).min(lines.start) as u16;
        }
    }
}

fn detail_max_scroll(model: &Model, task: Uuid, drawer: Rect) -> u16 {
    model
        .board
        .task(task)
        .map_or(0, |task| detail(task, drawer).max_scroll())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_evenly_spreads_the_remainder() {
        assert_eq!(split_evenly(10, 3, 2), [(0, 2), (4, 2), (8, 2)]);
        assert_eq!(split_evenly(11, 2, 1), [(0, 5), (6, 5)]);
        assert_eq!(split_evenly(12, 2, 1), [(0, 6), (7, 5)]);
        assert!(split_evenly(10, 0, 2).is_empty());
    }

    #[test]
    fn fit_offset_keeps_the_target_visible() {
        let heights = [3, 3, 3, 3, 3];
        // Three cards of three rows with gaps need 11 rows.
        assert_eq!(fit_offset(&heights, 11, 0, Some((2, 2))), 0);
        assert_eq!(fit_offset(&heights, 11, 0, Some((3, 3))), 1);
        assert_eq!(fit_offset(&heights, 11, 0, Some((4, 4))), 2);
        // Moving back up only scrolls once the target is above the top.
        assert_eq!(fit_offset(&heights, 11, 2, Some((3, 3))), 2);
        assert_eq!(fit_offset(&heights, 11, 2, Some((1, 1))), 1);
        // No empty space is left at the end.
        assert_eq!(fit_offset(&heights, 11, 4, None), 2);
        assert_eq!(fit_offset(&heights, 100, 3, None), 0);
        // A card taller than the space is still the first one shown.
        assert_eq!(fit_offset(&[10, 10], 5, 0, Some((1, 1))), 1);
        assert_eq!(fit_offset(&[], 5, 3, Some((1, 1))), 0);
    }

    #[test]
    fn visible_end_shows_whole_cards() {
        let heights = [3, 3, 3];
        assert_eq!(visible_end(&heights, 7, 0), 2);
        assert_eq!(visible_end(&heights, 6, 0), 1);
        assert_eq!(visible_end(&heights, 1, 1), 2);
        assert_eq!(visible_end(&heights, 100, 0), 3);
        assert_eq!(visible_end(&heights, 100, 3), 3);
    }
}
