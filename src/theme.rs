use std::env;
use std::ffi::OsStr;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub border_focused: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub selection: Color,
    /// Added to the selected item. Colour themes show selection with a
    /// background colour instead, so this is empty for them.
    pub selected_modifier: Modifier,
    /// Added to muted text.
    pub muted_modifier: Modifier,
    /// Added to the active input field's label.
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
            background: palette.base.into(),
            surface: palette.mantle.into(),
            surface_alt: palette.surface0.into(),
            border: palette.surface1.into(),
            border_focused: palette.mauve.into(),
            text: palette.text.into(),
            muted: palette.overlay1.into(),
            accent: palette.mauve.into(),
            accent_alt: palette.sapphire.into(),
            success: palette.green.into(),
            warning: palette.peach.into(),
            error: palette.red.into(),
            selection: palette.surface2.into(),
            selected_modifier: Modifier::empty(),
            muted_modifier: Modifier::empty(),
            active_modifier: Modifier::empty(),
        }
    }

    /// The theme used when `NO_COLOR` is set: no colour at all, following
    /// <https://no-color.org>. State is shown with text attributes instead:
    /// reversed for the selection, bold for focus, dim for muted text and
    /// underlined for the active field.
    pub fn monochrome() -> Self {
        Self {
            background: Color::Reset,
            surface: Color::Reset,
            surface_alt: Color::Reset,
            border: Color::Reset,
            border_focused: Color::Reset,
            text: Color::Reset,
            muted: Color::Reset,
            accent: Color::Reset,
            accent_alt: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,
            selection: Color::Reset,
            selected_modifier: Modifier::REVERSED,
            muted_modifier: Modifier::DIM,
            active_modifier: Modifier::UNDERLINED,
        }
    }
}

/// Whether a `NO_COLOR` value asks for no colour. Only a non-empty value
/// counts, as the convention specifies.
fn no_color(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

static SHARED_THEME: OnceLock<Theme> = OnceLock::new();

pub fn shared_theme() -> Theme {
    *SHARED_THEME.get_or_init(Theme::from_env)
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
    fn monochrome_uses_no_colours() {
        let theme = Theme::monochrome();
        let colours = [
            theme.background,
            theme.surface,
            theme.surface_alt,
            theme.border,
            theme.border_focused,
            theme.text,
            theme.muted,
            theme.accent,
            theme.accent_alt,
            theme.success,
            theme.warning,
            theme.error,
            theme.selection,
        ];
        assert!(colours.iter().all(|colour| *colour == Color::Reset));
    }
}
