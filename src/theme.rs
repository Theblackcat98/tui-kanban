use std::env;
use std::sync::OnceLock;

use ratatui::style::Color;

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
    pub use_colors: bool,
}

impl Theme {
    pub fn from_env() -> Self {
        if env::var_os("NO_COLOR").is_some() {
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
            use_colors: true,
        }
    }

    pub fn monochrome() -> Self {
        Self {
            background: Color::Reset,
            surface: Color::Reset,
            surface_alt: Color::Reset,
            border: Color::DarkGray,
            border_focused: Color::White,
            text: Color::White,
            muted: Color::DarkGray,
            accent: Color::Cyan,
            accent_alt: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            selection: Color::Indexed(236),
            use_colors: false,
        }
    }
}

static SHARED_THEME: OnceLock<Theme> = OnceLock::new();

pub fn shared_theme() -> Theme {
    *SHARED_THEME.get_or_init(Theme::from_env)
}
