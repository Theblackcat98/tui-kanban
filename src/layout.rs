//! Layout decisions that depend on the terminal size, worked out in one
//! place so the key handlers and the renderer always agree.

/// Terminals narrower than this hide the column rail.
pub const REGULAR_MIN_WIDTH: u16 = 76;
/// Terminals at least this wide show every column side by side.
pub const WIDE_MIN_WIDTH: u16 = 110;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Breakpoint {
    /// One column at a time, no rail.
    Compact,
    /// The rail plus one column at a time.
    Regular,
    /// The rail plus every column.
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

    pub fn shows_all_columns(self) -> bool {
        self == Self::Wide
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_follow_the_width() {
        assert_eq!(Breakpoint::from_width(0), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(75), Breakpoint::Compact);
        assert_eq!(Breakpoint::from_width(76), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(109), Breakpoint::Regular);
        assert_eq!(Breakpoint::from_width(110), Breakpoint::Wide);
        assert!(!Breakpoint::Compact.shows_rail());
        assert!(Breakpoint::Regular.shows_rail());
        assert!(!Breakpoint::Regular.shows_all_columns());
    }
}
