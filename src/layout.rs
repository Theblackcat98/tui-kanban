//! Layout decisions that depend on the terminal size, worked out in one
//! place so the key handlers and the renderer always agree. See the
//! spacing and breakpoint sections of `docs/design.md`.

/// The smallest terminal the app can be used in. Below this, a "terminal
/// too small" screen is shown and only quitting works.
pub const MIN_WIDTH: u16 = 40;
pub const MIN_HEIGHT: u16 = 12;

/// Terminals narrower than this are [`Breakpoint::Compact`].
pub const REGULAR_MIN_WIDTH: u16 = 80;
/// Terminals at least this wide are [`Breakpoint::Wide`].
pub const WIDE_MIN_WIDTH: u16 = 140;

/// The column rail's width, including its right edge.
pub const RAIL_WIDTH: u16 = 22;
/// The narrowest a lane gets; when fewer fit than there are columns, the
/// lanes scroll sideways.
pub const LANE_MIN_WIDTH: u16 = 24;
/// A collapsed lane's width: a strip with its name written downwards.
pub const COLLAPSED_LANE_WIDTH: u16 = 3;
/// Cells between lanes, and between All tasks cards in a row.
pub const LANE_GUTTER: u16 = 2;
/// Cells on each side of the lane area.
pub const PAGE_MARGIN: u16 = 1;
/// Blank rows between cards.
pub const CARD_GAP: u16 = 1;
/// The narrowest All tasks card; rows hold as many as fit, up to
/// [`MAX_CARDS_PER_ROW`].
pub const MIN_CARD_WIDTH: u16 = 30;
pub const MAX_CARDS_PER_ROW: u16 = 3;
/// The pinned detail drawer's width at the Wide breakpoint.
pub const PINNED_DRAWER_WIDTH: u16 = 56;

/// Whether a terminal of this size is big enough to use.
pub fn fits(width: u16, height: u16) -> bool {
    width >= MIN_WIDTH && height >= MIN_HEIGHT
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Breakpoint {
    /// One lane at a time, no rail, and a full-width detail drawer.
    Compact,
    /// The rail plus lanes, with the detail drawer over the right side.
    Regular,
    /// As Regular, with the detail drawer pinned beside the lanes.
    Wide,
}

impl Breakpoint {
    pub fn from_width(width: u16) -> Self {
        if width >= WIDE_MIN_WIDTH {
            Self::Wide
        } else if width >= REGULAR_MIN_WIDTH {
            Self::Regular
        } else {
            Self::Compact
        }
    }

    pub fn shows_rail(self) -> bool {
        self != Self::Compact
    }

    /// Whether the Board view shows several lanes side by side.
    pub fn shows_several_lanes(self) -> bool {
        self != Self::Compact
    }

    /// Whether the detail drawer covers the full width.
    pub fn full_width_drawer(self) -> bool {
        self == Self::Compact
    }

    /// Whether the detail drawer sits beside the lanes instead of over
    /// them.
    pub fn pinned_drawer(self) -> bool {
        self == Self::Wide
    }
}

/// How many lanes of at least [`LANE_MIN_WIDTH`] fit in `width` cells;
/// always at least one.
pub fn lanes_that_fit(width: u16) -> usize {
    usize::from(((width + LANE_GUTTER) / (LANE_MIN_WIDTH + LANE_GUTTER)).max(1))
}

/// How many All tasks cards fit in a row of this width.
pub fn cards_per_row(width: u16) -> u16 {
    ((width + LANE_GUTTER) / (MIN_CARD_WIDTH + LANE_GUTTER)).clamp(1, MAX_CARDS_PER_ROW)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_follow_the_width() {
        assert_eq!(Breakpoint::from_width(0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(79), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(80), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(139), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(140), Breakpoint::Wide);
        assert!(!Breakpoint::Compact.shows_rail());
        assert!(Breakpoint::Regular.shows_rail());
        assert!(!Breakpoint::Regular.pinned_drawer());
        assert!(Breakpoint::Wide.pinned_drawer());
    }

    #[test]
    fn lanes_that_fit_the_width() {
        assert_eq!(lanes_that_fit(0), 1);
        assert_eq!(lanes_that_fit(49), 1);
        assert_eq!(lanes_that_fit(50), 2);
        assert_eq!(lanes_that_fit(76), 3);
        assert_eq!(lanes_that_fit(136), 5);
    }

    #[test]
    fn cards_per_row_fits_the_width() {
        assert_eq!(cards_per_row(20), 1);
        assert_eq!(cards_per_row(61), 1);
        assert_eq!(cards_per_row(62), 2);
        assert_eq!(cards_per_row(200), 3);
    }
}
