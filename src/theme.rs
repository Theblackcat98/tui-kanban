//! Colours by role. The UI never names a palette colour: it asks for
//! `text_muted` or `selection`, and the theme decides what that is. See
//! `docs/design.md` for where each role is used.

use std::env;
use std::ffi::OsStr;

use ratatui::style::{Color, Modifier};

#[derive(Clone, Debug)]
pub struct Theme {
    /// The page background.
    pub bg: Color,
    /// The rail, the bars, the detail drawer and overlays.
    pub panel: Color,
    /// Card tiles and input fields.
    pub surface: Color,
    /// The selected card tile.
    pub selection: Color,
    /// Overlay borders.
    pub border: Color,
    /// Titles and body text.
    pub text: Color,
    /// Descriptions and unfocused lane names.
    pub text_muted: Color,
    /// Metadata, counts, hints and placeholders.
    pub text_faint: Color,
    /// The focused control, the mode pill and overlay titles.
    pub accent: Color,
    /// The focused lane's underline and the rail marker.
    pub focus: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    pub info: Color,
    /// Lane accents, used in turn.
    pub lanes: Vec<Color>,
    /// Added to the selected item. Colour themes show selection with a
    /// background colour instead, so this is empty for them.
    pub selected_modifier: Modifier,
    /// Added to muted and faint text.
    pub muted_modifier: Modifier,
    /// Added to the active input.
    pub active_modifier: Modifier,
}

impl Theme {
    pub fn from_env() -> Self {
        if no_color(env::var_os("NO_COLOR").as_deref()) {
            Self::monochrome()
        } else {
            Self::mocha()
        }
    }

    pub fn mocha() -> Self {
        let palette = catppuccin::PALETTE.mocha.colors;
        Self {
            bg: palette.base.into(),
            panel: palette.mantle.into(),
            surface: palette.surface0.into(),
            selection: palette.surface1.into(),
            border: palette.surface2.into(),
            text: palette.text.into(),
            text_muted: palette.subtext0.into(),
            text_faint: palette.overlay1.into(),
            accent: palette.mauve.into(),
            focus: palette.lavender.into(),
            danger: palette.red.into(),
            success: palette.green.into(),
            warning: palette.peach.into(),
            info: palette.sapphire.into(),
            lanes: [
                palette.sapphire,
                palette.peach,
                palette.green,
                palette.mauve,
                palette.teal,
                palette.pink,
                palette.yellow,
                palette.lavender,
                palette.sky,
                palette.maroon,
                palette.flamingo,
                palette.rosewater,
            ]
            .into_iter()
            .map(Color::from)
            .collect(),
            selected_modifier: Modifier::empty(),
            muted_modifier: Modifier::empty(),
            active_modifier: Modifier::empty(),
        }
    }

    /// The theme used when `NO_COLOR` is set: no colour at all, following
    /// <https://no-color.org>. State is shown with text attributes instead:
    /// reversed for the selection, bold for focus, dim for muted text and
    /// underlined for the active input.
    pub fn monochrome() -> Self {
        Self {
            bg: Color::Reset,
            panel: Color::Reset,
            surface: Color::Reset,
            selection: Color::Reset,
            border: Color::Reset,
            text: Color::Reset,
            text_muted: Color::Reset,
            text_faint: Color::Reset,
            accent: Color::Reset,
            focus: Color::Reset,
            danger: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            info: Color::Reset,
            lanes: vec![Color::Reset],
            selected_modifier: Modifier::REVERSED,
            muted_modifier: Modifier::DIM,
            active_modifier: Modifier::UNDERLINED,
        }
    }

    /// The accent for the lane at `index`, cycling through [`Theme::lanes`].
    pub fn lane(&self, index: usize) -> Color {
        if self.lanes.is_empty() {
            return self.accent;
        }
        self.lanes[index % self.lanes.len()]
    }

    /// Whether this theme has no colours, so state has to be shown with
    /// text attributes.
    pub fn is_monochrome(&self) -> bool {
        !self.selected_modifier.is_empty()
    }

    /// Every role colour, for tests.
    pub fn roles(&self) -> Vec<Color> {
        let mut roles = vec![
            self.bg,
            self.panel,
            self.surface,
            self.selection,
            self.border,
            self.text,
            self.text_muted,
            self.text_faint,
            self.accent,
            self.focus,
            self.danger,
            self.success,
            self.warning,
            self.info,
        ];
        roles.extend(&self.lanes);
        roles
    }
}

/// Whether a `NO_COLOR` value asks for no colour. Only a non-empty value
/// counts, as the convention specifies.
fn no_color(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

/// Whether a `REDUCE_MOTION` value asks for no animation: any non-empty
/// value other than `0` or `false`.
pub fn reduce_motion(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty() && value != "0" && value != "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_non_empty_no_color_counts() {
        assert!(!no_color(None));
        assert!(!no_color(Some(OsStr::new(""))));
        assert!(no_color(Some(OsStr::new("1"))));
    }

    #[test]
    fn reduce_motion_values() {
        assert!(!reduce_motion(None));
        assert!(!reduce_motion(Some(OsStr::new(""))));
        assert!(!reduce_motion(Some(OsStr::new("0"))));
        assert!(!reduce_motion(Some(OsStr::new("false"))));
        assert!(reduce_motion(Some(OsStr::new("1"))));
        assert!(reduce_motion(Some(OsStr::new("reduce"))));
    }

    #[test]
    fn monochrome_uses_no_colours() {
        let theme = Theme::monochrome();
        assert!(theme.roles().iter().all(|colour| *colour == Color::Reset));
        assert!(theme.is_monochrome());
        assert!(!Theme::mocha().is_monochrome());
    }

    #[test]
    fn lane_colours_cycle() {
        let theme = Theme::mocha();
        assert_eq!(theme.lane(0), theme.lane(theme.lanes.len()));
        assert_ne!(theme.lane(0), theme.lane(1));
        assert_eq!(Theme::monochrome().lane(5), Color::Reset);
    }
}
