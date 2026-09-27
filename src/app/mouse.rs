//! What the mouse points at and is doing. The view works out what is
//! under the pointer ([`crate::ui::hit`]) from the same geometry it draws
//! with, so clicks always land on what is on screen; `update` acts on it.

use ratatui::layout::Rect;
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::model::ViewMode;
use crate::command::CommandId;

/// Two clicks on the same thing this close together are a double click.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// Something on screen the mouse can point at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hit {
    Card {
        id: Uuid,
    },
    /// A lane's header or the space around its cards, or an All tasks
    /// section's header.
    Lane {
        column: usize,
    },
    /// A collapsed lane.
    Strip {
        column: usize,
    },
    /// A column in the rail.
    Rail {
        column: usize,
    },
    /// A view's name in the top bar.
    View(ViewMode),
    /// "‹ N more" and "N more ›" above the lanes.
    MoreLeft,
    MoreRight,
    /// A hint in the status line, and the command it runs.
    Hint(CommandId),
    /// The detail drawer.
    Drawer,
}

/// Where a dragged card would go if dropped now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DropTarget {
    pub column: usize,
    /// The index to insert at, as for `Board::move_task`, or `None` for
    /// the end of the column.
    pub index: Option<usize>,
    pub marker: Marker,
}

/// How the drop position is shown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Marker {
    /// A line across a lane, between cards.
    Line(Rect),
    /// A bar down the side of a card in All tasks, or beside a rail entry.
    Bar(Rect),
}

#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub task: Uuid,
    /// Where the button went down.
    pub from: (u16, u16),
    /// Whether the pointer has moved since, which makes it a drag rather
    /// than a click.
    pub moved: bool,
    pub target: Option<DropTarget>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Mouse {
    /// The last click, to tell a double click.
    pub last_click: Option<(Instant, Hit)>,
    /// A card being dragged.
    pub drag: Option<Drag>,
}
