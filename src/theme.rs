//! Colours by role. The UI never names a palette colour: it asks for
//! `text_muted` or `selection`, and the theme decides what that is. See
//! `docs/design.md` for where each role is used.
//!
//! Built-in themes are the four Catppuccin flavours, a 16-colour theme for
//! terminals without more colours, and a monochrome theme for `NO_COLOR`.
//! Users can add their own as TOML files (see [`Theme::from_toml`]).

use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use ratatui::style::{Color, Modifier};

/// The built-in theme names, for `--theme` and error messages.
pub const BUILT_IN: [&str; 5] = ["latte", "frappe", "macchiato", "mocha", "ansi"];

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
    /// Colours that can be named, as in a column's `"color": "mauve"`.
    pub palette: Vec<(&'static str, Color)>,
    /// Added to the selected item. Colour themes show selection with a
    /// background colour instead, so this is empty for them.
    pub selected_modifier: Modifier,
    /// Added to muted and faint text.
    pub muted_modifier: Modifier,
    /// Added to the active input.
    pub active_modifier: Modifier,
}

/// The role names, as used in theme files.
const ROLES: [&str; 14] = [
    "bg",
    "panel",
    "surface",
    "selection",
    "border",
    "text",
    "text_muted",
    "text_faint",
    "accent",
    "focus",
    "danger",
    "success",
    "warning",
    "info",
];

/// Lane accents, by Catppuccin colour name.
const LANE_NAMES: [&str; 12] = [
    "sapphire",
    "peach",
    "green",
    "mauve",
    "teal",
    "pink",
    "yellow",
    "lavender",
    "sky",
    "maroon",
    "flamingo",
    "rosewater",
];

impl Theme {
    pub fn latte() -> Self {
        Self::catppuccin(&catppuccin::PALETTE.latte)
    }

    pub fn frappe() -> Self {
        Self::catppuccin(&catppuccin::PALETTE.frappe)
    }

    pub fn macchiato() -> Self {
        Self::catppuccin(&catppuccin::PALETTE.macchiato)
    }

    pub fn mocha() -> Self {
        Self::catppuccin(&catppuccin::PALETTE.mocha)
    }

    fn catppuccin(flavor: &catppuccin::Flavor) -> Self {
        let palette: Vec<(&'static str, Color)> = flavor
            .iter()
            .map(|color| (color.name.identifier(), Color::from(*color)))
            .collect();
        let named = |name: &str| {
            palette
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map(|(_, color)| *color)
                .expect("every Catppuccin flavour has the same colour names")
        };
        // Light flavours use lighter surfaces and a darker faint text, so
        // text keeps its contrast (see the contrast test below).
        let (surface, selection, border, faint) = if flavor.dark {
            ("surface0", "surface1", "surface2", "overlay2")
        } else {
            ("crust", "surface0", "surface1", "subtext0")
        };
        Self {
            bg: named("base"),
            panel: named("mantle"),
            surface: named(surface),
            selection: named(selection),
            border: named(border),
            text: named("text"),
            text_muted: named("subtext1"),
            text_faint: named(faint),
            accent: named("mauve"),
            focus: named("lavender"),
            danger: named("red"),
            success: named("green"),
            warning: named("peach"),
            info: named("sapphire"),
            lanes: LANE_NAMES.iter().map(|name| named(name)).collect(),
            palette,
            selected_modifier: Modifier::empty(),
            muted_modifier: Modifier::empty(),
            active_modifier: Modifier::empty(),
        }
    }

    /// For terminals with only the 16 standard colours. It uses the
    /// terminal's own background and text colours, so it works on light
    /// and dark backgrounds alike.
    pub fn ansi() -> Self {
        let palette = vec![
            ("red", Color::Red),
            ("maroon", Color::Red),
            ("peach", Color::LightRed),
            ("flamingo", Color::LightRed),
            ("rosewater", Color::LightRed),
            ("yellow", Color::Yellow),
            ("green", Color::Green),
            ("teal", Color::Cyan),
            ("sky", Color::LightCyan),
            ("sapphire", Color::Blue),
            ("blue", Color::Blue),
            ("lavender", Color::LightBlue),
            ("mauve", Color::Magenta),
            ("pink", Color::LightMagenta),
        ];
        Self {
            bg: Color::Reset,
            panel: Color::Reset,
            surface: Color::Reset,
            selection: Color::DarkGray,
            border: Color::DarkGray,
            text: Color::Reset,
            text_muted: Color::Gray,
            text_faint: Color::DarkGray,
            accent: Color::Magenta,
            focus: Color::LightBlue,
            danger: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,
            info: Color::Cyan,
            lanes: vec![
                Color::Blue,
                Color::Yellow,
                Color::Green,
                Color::Magenta,
                Color::Cyan,
                Color::LightMagenta,
                Color::LightYellow,
                Color::LightBlue,
            ],
            palette,
            selected_modifier: Modifier::empty(),
            muted_modifier: Modifier::empty(),
            active_modifier: Modifier::UNDERLINED,
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
            palette: Vec::new(),
            selected_modifier: Modifier::REVERSED,
            muted_modifier: Modifier::DIM,
            active_modifier: Modifier::UNDERLINED,
        }
    }

    /// A built-in theme by name.
    pub fn built_in(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "latte" => Some(Self::latte()),
            "frappe" | "frappé" => Some(Self::frappe()),
            "macchiato" => Some(Self::macchiato()),
            "mocha" => Some(Self::mocha()),
            "ansi" => Some(Self::ansi()),
            "monochrome" => Some(Self::monochrome()),
            _ => None,
        }
    }

    /// A theme from a TOML file's contents: a built-in theme to start from,
    /// and the roles to change. Colours are `"#rrggbb"` or a colour name
    /// from the base theme's palette.
    ///
    /// ```toml
    /// base = "mocha"
    ///
    /// [colors]
    /// bg = "#11111b"
    /// accent = "pink"
    /// lanes = ["blue", "#fab387", "green"]
    /// ```
    pub fn from_toml(text: &str) -> Result<Self> {
        let table: toml::Table = toml::from_str(text)?;
        let base = match table.get("base") {
            None => "mocha",
            Some(toml::Value::String(name)) => name,
            Some(_) => bail!("`base` must be a theme name"),
        };
        let mut theme =
            Self::built_in(base).with_context(|| format!("unknown base theme `{base}`"))?;
        for key in table.keys() {
            if key != "base" && key != "colors" {
                bail!("unknown key `{key}`; expected `base` or `[colors]`");
            }
        }
        let Some(colors) = table.get("colors") else {
            return Ok(theme);
        };
        let toml::Value::Table(colors) = colors else {
            bail!("`colors` must be a table");
        };
        for (role, value) in colors {
            if role == "lanes" {
                let toml::Value::Array(values) = value else {
                    bail!("`lanes` must be a list of colours");
                };
                let lanes = values
                    .iter()
                    .map(|value| theme.parse_value(value))
                    .collect::<Result<Vec<_>>>()
                    .context("in `lanes`")?;
                if lanes.is_empty() {
                    bail!("`lanes` needs at least one colour");
                }
                theme.lanes = lanes;
                continue;
            }
            let color = theme
                .parse_value(value)
                .with_context(|| format!("in `{role}`"))?;
            match theme.role_mut(role) {
                Some(slot) => *slot = color,
                None => bail!(
                    "unknown colour role `{role}`; the roles are {} and lanes",
                    ROLES.join(", ")
                ),
            }
        }
        Ok(theme)
    }

    fn parse_value(&self, value: &toml::Value) -> Result<Color> {
        match value {
            toml::Value::String(text) => self
                .color(text)
                .with_context(|| format!("`{text}` is not \"#rrggbb\" or a known colour name")),
            _ => bail!("colours must be strings"),
        }
    }

    fn role_mut(&mut self, role: &str) -> Option<&mut Color> {
        Some(match role {
            "bg" => &mut self.bg,
            "panel" => &mut self.panel,
            "surface" => &mut self.surface,
            "selection" => &mut self.selection,
            "border" => &mut self.border,
            "text" => &mut self.text,
            "text_muted" => &mut self.text_muted,
            "text_faint" => &mut self.text_faint,
            "accent" => &mut self.accent,
            "focus" => &mut self.focus,
            "danger" => &mut self.danger,
            "success" => &mut self.success,
            "warning" => &mut self.warning,
            "info" => &mut self.info,
            _ => return None,
        })
    }

    /// A colour by palette name (`"mauve"`) or as `"#rrggbb"`.
    pub fn color(&self, name: &str) -> Option<Color> {
        let name = name.trim();
        if let Some(hex) = name.strip_prefix('#') {
            return parse_hex(hex);
        }
        let name = name.to_lowercase();
        self.palette
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, color)| *color)
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

    /// Every role colour and lane accent.
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

    /// The theme with every colour replaced by its nearest in the xterm
    /// 256-colour palette, for terminals without true colour.
    pub fn to_256_colors(mut self) -> Self {
        for role in ROLES {
            let slot = self.role_mut(role).expect("ROLES lists every role");
            *slot = nearest_256(*slot);
        }
        for lane in &mut self.lanes {
            *lane = nearest_256(*lane);
        }
        for (_, color) in &mut self.palette {
            *color = nearest_256(*color);
        }
        self
    }

    /// Adapts a colour that isn't from the theme, such as a column's own
    /// `#rrggbb`, to what the theme can show.
    pub fn adapt(&self, color: Color) -> Color {
        if self.is_monochrome() {
            return Color::Reset;
        }
        if !matches!(color, Color::Rgb(..)) {
            return color;
        }
        match self.bg {
            Color::Indexed(_) => nearest_256(color),
            Color::Rgb(..) => color,
            // The 16-colour theme.
            _ => nearest_ansi(color),
        }
    }

    /// A column's accent: its own `color` if this theme knows it, or else
    /// a lane colour chosen by its id, so it doesn't change when columns
    /// are reordered or added.
    pub fn column_color(&self, id: &str, color: Option<&str>) -> Color {
        match color.and_then(|name| self.color(name)) {
            Some(color) => self.adapt(color),
            None => self.lane(stable_hash(id) as usize),
        }
    }
}

/// FNV-1a: a hash that is the same in every build and on every platform,
/// unlike the standard library's.
fn stable_hash(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The nearest of the 16 standard colours, using typical xterm values.
fn nearest_ansi(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    const COLOURS: [(Color, (u8, u8, u8)); 14] = [
        (Color::Red, (205, 0, 0)),
        (Color::Green, (0, 205, 0)),
        (Color::Yellow, (205, 205, 0)),
        (Color::Blue, (0, 0, 238)),
        (Color::Magenta, (205, 0, 205)),
        (Color::Cyan, (0, 205, 205)),
        (Color::Gray, (229, 229, 229)),
        (Color::DarkGray, (127, 127, 127)),
        (Color::LightRed, (255, 0, 0)),
        (Color::LightGreen, (0, 255, 0)),
        (Color::LightYellow, (255, 255, 0)),
        (Color::LightBlue, (92, 92, 255)),
        (Color::LightMagenta, (255, 0, 255)),
        (Color::LightCyan, (0, 255, 255)),
    ];
    let distance = |(r2, g2, b2): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(r, r2) + d(g, g2) + d(b, b2)
    };
    COLOURS
        .iter()
        .min_by_key(|(_, rgb)| distance(*rgb))
        .map(|(color, _)| *color)
        .expect("a non-empty list")
}

fn parse_hex(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
    Some(Color::Rgb(channel(0)?, channel(2)?, channel(4)?))
}

/// The nearest xterm 256-colour palette entry to an RGB colour, from the
/// 6×6×6 colour cube and the grey ramp. Other colours are unchanged.
fn nearest_256(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest_level = |value: u8| {
        (0..6)
            .min_by_key(|&index| (i32::from(LEVELS[index]) - i32::from(value)).abs())
            .expect("six levels")
    };
    let distance = |(r2, g2, b2): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(r, r2) + d(g, g2) + d(b, b2)
    };
    let (ri, gi, bi) = (nearest_level(r), nearest_level(g), nearest_level(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;
    let average = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let grey_step = ((average.saturating_sub(8) + 5) / 10).min(23) as u8;
    let grey_level = 8 + 10 * grey_step;
    let grey = (grey_level, grey_level, grey_level);
    if distance(grey) < distance(cube) {
        Color::Indexed(232 + grey_step)
    } else {
        Color::Indexed(cube_index as u8)
    }
}

/// How many colours the terminal can show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorDepth {
    TrueColor,
    Indexed256,
    Ansi16,
}

impl ColorDepth {
    /// From `COLORTERM` and `TERM`, the way most terminal programs decide.
    pub fn detect(colorterm: Option<&OsStr>, term: Option<&OsStr>) -> Self {
        let colorterm = colorterm.and_then(OsStr::to_str).unwrap_or("");
        if matches!(colorterm, "truecolor" | "24bit") {
            return Self::TrueColor;
        }
        match term.and_then(OsStr::to_str) {
            // Windows terminals don't set TERM, and all current ones
            // support true colour.
            None if cfg!(windows) => Self::TrueColor,
            Some(term) if term.contains("truecolor") || term.contains("direct") => Self::TrueColor,
            Some(term) if term.contains("256") => Self::Indexed256,
            _ => Self::Ansi16,
        }
    }
}

/// Whether the terminal has a light background, when it can tell.
pub type LightBackground = Option<bool>;

/// Whether the terminal's background is light: from `COLORFGBG` when the
/// terminal sets it, or else by asking the terminal. Must be called
/// before the terminal enters raw mode.
pub fn detect_light_background() -> LightBackground {
    if let Some(light) = colorfgbg_is_light(env::var_os("COLORFGBG").as_deref()) {
        return Some(light);
    }
    use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};
    match theme_mode(QueryOptions::default()) {
        Ok(ThemeMode::Light) => Some(true),
        Ok(ThemeMode::Dark) => Some(false),
        Err(_) => None,
    }
}

/// Reads `COLORFGBG` (as in `15;0`, foreground then background): the
/// background is light if it is colour 7 (light grey) or a bright colour
/// other than 8 (dark grey).
fn colorfgbg_is_light(value: Option<&OsStr>) -> Option<bool> {
    let background: u8 = value?.to_str()?.rsplit(';').next()?.parse().ok()?;
    Some(background == 7 || (9..=15).contains(&background))
}

/// What the environment says about colours.
#[derive(Clone, Debug)]
pub struct Environment {
    pub no_color: bool,
    pub depth: ColorDepth,
    /// Where user themes live: `<config>/tui-kanban/themes`.
    pub themes_dir: Option<PathBuf>,
}

impl Environment {
    pub fn from_env() -> Self {
        Self {
            no_color: no_color(env::var_os("NO_COLOR").as_deref()),
            depth: ColorDepth::detect(
                env::var_os("COLORTERM").as_deref(),
                env::var_os("TERM").as_deref(),
            ),
            themes_dir: crate::paths::config_dir().map(|dir| dir.join("tui-kanban").join("themes")),
        }
    }
}

/// Picks the theme for a `--theme` value (`auto` when not given):
/// `NO_COLOR` wins, then a built-in or user theme by name or path, with
/// `auto` choosing Latte on light backgrounds and Mocha otherwise. The
/// result is adapted to the terminal's colour depth.
pub fn resolve(
    choice: Option<&str>,
    environment: &Environment,
    light_background: impl FnOnce() -> LightBackground,
) -> Result<Theme> {
    if environment.no_color {
        return Ok(Theme::monochrome());
    }
    let choice = choice.unwrap_or("auto");
    let theme = if choice == "auto" {
        if environment.depth == ColorDepth::Ansi16 {
            // The 16-colour theme follows the terminal's own colours, so
            // there is no need to ask.
            return Ok(Theme::ansi());
        }
        match light_background() {
            Some(true) => Theme::latte(),
            _ => Theme::mocha(),
        }
    } else if let Some(theme) = Theme::built_in(choice) {
        theme
    } else {
        load_user_theme(choice, environment)?
    };
    Ok(match environment.depth {
        _ if theme.is_monochrome() => theme,
        ColorDepth::TrueColor => theme,
        ColorDepth::Indexed256 => theme.to_256_colors(),
        ColorDepth::Ansi16 => Theme::ansi(),
    })
}

fn load_user_theme(choice: &str, environment: &Environment) -> Result<Theme> {
    let path = if choice.ends_with(".toml") {
        PathBuf::from(choice)
    } else {
        match &environment.themes_dir {
            Some(dir) => dir.join(format!("{choice}.toml")),
            None => bail!(unknown_theme(choice, environment)),
        }
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!(unknown_theme(choice, environment))
        }
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()));
        }
    };
    Theme::from_toml(&text).with_context(|| format!("invalid theme {}", path.display()))
}

fn unknown_theme(choice: &str, environment: &Environment) -> String {
    let mut message = format!(
        "unknown theme `{choice}`; the built-in themes are auto, {}",
        BUILT_IN.join(", ")
    );
    if let Some(dir) = &environment.themes_dir {
        message.push_str(&format!(
            ", or add {}",
            dir.join(format!("{choice}.toml")).display()
        ));
    }
    message
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

/// The WCAG contrast ratio between two RGB colours, from 1 to 21.
pub fn contrast_ratio(first: Color, second: Color) -> Option<f64> {
    let luminance = |color: Color| {
        let Color::Rgb(r, g, b) = color else {
            return None;
        };
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b))
    };
    let (first, second) = (luminance(first)?, luminance(second)?);
    let (light, dark) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    Some((light + 0.05) / (dark + 0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(depth: ColorDepth) -> Environment {
        Environment {
            no_color: false,
            depth,
            themes_dir: None,
        }
    }

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

    /// Text must be readable on every background it is drawn on. WCAG AA
    /// asks for 4.5:1 for body text; faint text (metadata and hints) is
    /// held to 3:1.
    #[test]
    fn text_has_enough_contrast_in_every_theme() {
        for name in ["latte", "frappe", "macchiato", "mocha"] {
            let theme = Theme::built_in(name).unwrap();
            let checks = [
                (
                    "text",
                    theme.text,
                    4.5,
                    vec![theme.bg, theme.panel, theme.surface, theme.selection],
                ),
                (
                    "text_muted",
                    theme.text_muted,
                    4.5,
                    vec![theme.bg, theme.panel, theme.surface],
                ),
                (
                    "text_faint",
                    theme.text_faint,
                    3.0,
                    vec![theme.bg, theme.panel, theme.surface],
                ),
            ];
            for (role, color, minimum, backgrounds) in checks {
                for background in backgrounds {
                    let ratio = contrast_ratio(color, background).unwrap();
                    assert!(
                        ratio >= minimum,
                        "{name}: {role} on {background:?} is {ratio:.2}:1"
                    );
                }
            }
        }
    }

    #[test]
    fn contrast_ratio_matches_wcag() {
        let black = Color::Rgb(0, 0, 0);
        let white = Color::Rgb(255, 255, 255);
        assert!((contrast_ratio(black, white).unwrap() - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white).unwrap() - 1.0).abs() < 0.01);
        assert_eq!(contrast_ratio(Color::Reset, white), None);
    }

    #[test]
    fn flavours_differ_and_latte_is_light() {
        let latte = Theme::latte();
        let mocha = Theme::mocha();
        assert_ne!(latte.bg, mocha.bg);
        let Color::Rgb(r, _, _) = latte.bg else {
            panic!("expected RGB");
        };
        assert!(r > 200);
        assert_eq!(Theme::built_in("Frappé").unwrap().bg, Theme::frappe().bg);
        assert!(Theme::built_in("nope").is_none());
    }

    #[test]
    fn named_and_hex_colours() {
        let theme = Theme::mocha();
        assert_eq!(theme.color("mauve"), Some(theme.accent));
        assert_eq!(theme.color("#ff8000"), Some(Color::Rgb(255, 128, 0)));
        assert_eq!(theme.color("#ff80"), None);
        assert_eq!(theme.color("chartreuse"), None);
        assert_eq!(Theme::ansi().color("mauve"), Some(Color::Magenta));
    }

    #[test]
    fn toml_themes_override_roles() {
        let theme = Theme::from_toml(
            r##"
            base = "latte"
            [colors]
            bg = "#101010"
            accent = "pink"
            lanes = ["red", "#00ff00"]
            "##,
        )
        .unwrap();
        let latte = Theme::latte();
        assert_eq!(theme.bg, Color::Rgb(16, 16, 16));
        assert_eq!(theme.accent, latte.color("pink").unwrap());
        assert_eq!(theme.text, latte.text);
        assert_eq!(
            theme.lanes,
            [latte.color("red").unwrap(), Color::Rgb(0, 255, 0)]
        );
        // An empty file is the base theme.
        assert_eq!(Theme::from_toml("").unwrap().bg, Theme::mocha().bg);
    }

    #[test]
    fn toml_errors_are_explained() {
        let error = |text: &str| format!("{:#}", Theme::from_toml(text).unwrap_err());
        assert!(error("base = \"nope\"").contains("unknown base theme"));
        assert!(error("[colors]\nbackground = \"red\"").contains("unknown colour role"));
        assert!(error("[colors]\nbg = \"#12\"").contains("not \"#rrggbb\""));
        assert!(error("[colors]\nlanes = []").contains("at least one"));
        assert!(error("colours = 1").contains("unknown key"));
        assert!(error("[colors").contains("TOML"));
    }

    #[test]
    fn column_colours() {
        let theme = Theme::mocha();
        // A column's own colour, by name or hex.
        assert_eq!(
            theme.column_color("a", Some("teal")),
            theme.color("teal").unwrap()
        );
        assert_eq!(
            theme.column_color("a", Some("#102030")),
            Color::Rgb(16, 32, 48)
        );
        // Otherwise chosen by id: the same whatever the column's position,
        // and an unknown colour name falls back to it.
        let by_id = theme.column_color("review", None);
        assert_eq!(theme.column_color("review", Some("nope")), by_id);
        assert!(theme.lanes.contains(&by_id));
        let ids = ["backlog", "in-progress", "done", "review", "blocked", "qa"];
        let colours: std::collections::HashSet<_> = ids
            .iter()
            .map(|id| format!("{:?}", theme.column_color(id, None)))
            .collect();
        assert!(colours.len() > 1);
        // Adapted to the terminal.
        assert_eq!(
            Theme::ansi().column_color("a", Some("#ff1010")),
            Color::LightRed
        );
        assert_eq!(
            Theme::ansi().column_color("a", Some("mauve")),
            Color::Magenta
        );
        assert!(matches!(
            Theme::mocha()
                .to_256_colors()
                .column_color("a", Some("#102030")),
            Color::Indexed(_)
        ));
        assert_eq!(
            Theme::monochrome().column_color("a", Some("#ff0000")),
            Color::Reset
        );
    }

    #[test]
    fn stable_hash_is_fnv1a() {
        assert_eq!(stable_hash(""), 0x811c_9dc5);
        assert_eq!(stable_hash("a"), 0xe40c_292c);
    }

    #[test]
    fn colorfgbg_background() {
        let os = |value: &'static str| Some(OsStr::new(value));
        assert_eq!(colorfgbg_is_light(os("15;0")), Some(false));
        assert_eq!(colorfgbg_is_light(os("0;15")), Some(true));
        assert_eq!(colorfgbg_is_light(os("0;default;7")), Some(true));
        assert_eq!(colorfgbg_is_light(os("15;8")), Some(false));
        assert_eq!(colorfgbg_is_light(os("default")), None);
        assert_eq!(colorfgbg_is_light(None), None);
    }

    #[test]
    fn colour_depth_detection() {
        let os = |value: &'static str| Some(OsStr::new(value));
        assert_eq!(
            ColorDepth::detect(os("truecolor"), os("xterm")),
            ColorDepth::TrueColor
        );
        assert_eq!(
            ColorDepth::detect(None, os("xterm-256color")),
            ColorDepth::Indexed256
        );
        assert_eq!(
            ColorDepth::detect(None, os("xterm-direct")),
            ColorDepth::TrueColor
        );
        assert_eq!(ColorDepth::detect(None, os("linux")), ColorDepth::Ansi16);
    }

    #[test]
    fn nearest_256_picks_cube_or_grey() {
        assert_eq!(nearest_256(Color::Rgb(0, 0, 0)), Color::Indexed(16));
        assert_eq!(nearest_256(Color::Rgb(255, 255, 255)), Color::Indexed(231));
        assert_eq!(nearest_256(Color::Rgb(255, 0, 0)), Color::Indexed(196));
        assert_eq!(nearest_256(Color::Rgb(128, 128, 128)), Color::Indexed(244));
        assert_eq!(nearest_256(Color::Reset), Color::Reset);
        let theme = Theme::mocha().to_256_colors();
        assert!(
            theme
                .roles()
                .iter()
                .all(|colour| matches!(colour, Color::Indexed(_)))
        );
    }

    #[test]
    fn resolving_a_theme() {
        let true_color = environment(ColorDepth::TrueColor);
        let resolve_bg =
            |choice: Option<&str>, environment: &Environment, light: LightBackground| {
                resolve(choice, environment, || light).unwrap().bg
            };
        assert_eq!(resolve_bg(None, &true_color, Some(true)), Theme::latte().bg);
        assert_eq!(
            resolve_bg(None, &true_color, Some(false)),
            Theme::mocha().bg
        );
        assert_eq!(resolve_bg(None, &true_color, None), Theme::mocha().bg);
        assert_eq!(
            resolve_bg(Some("frappe"), &true_color, Some(true)),
            Theme::frappe().bg
        );
        assert!(matches!(
            resolve_bg(Some("mocha"), &environment(ColorDepth::Indexed256), None),
            Color::Indexed(_)
        ));
        assert_eq!(
            resolve_bg(Some("mocha"), &environment(ColorDepth::Ansi16), None),
            Color::Reset
        );
        let no_color = Environment {
            no_color: true,
            ..true_color.clone()
        };
        assert!(
            resolve(Some("mocha"), &no_color, || None)
                .unwrap()
                .is_monochrome()
        );
        let error = resolve(Some("nope"), &true_color, || None).unwrap_err();
        assert!(error.to_string().contains("unknown theme `nope`"));
    }

    #[test]
    fn user_themes_load_from_the_themes_directory() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("mine.toml"),
            "base = \"latte\"\n[colors]\naccent = \"#123456\"\n",
        )
        .unwrap();
        let environment = Environment {
            themes_dir: Some(directory.path().to_owned()),
            ..environment(ColorDepth::TrueColor)
        };
        let theme = resolve(Some("mine"), &environment, || None).unwrap();
        assert_eq!(theme.accent, Color::Rgb(0x12, 0x34, 0x56));
        let path = directory.path().join("mine.toml");
        let theme = resolve(path.to_str(), &environment, || None).unwrap();
        assert_eq!(theme.bg, Theme::latte().bg);
        std::fs::write(directory.path().join("bad.toml"), "[colors]\nbg = 1\n").unwrap();
        let error = resolve(Some("bad"), &environment, || None).unwrap_err();
        assert!(format!("{error:#}").contains("colours must be strings"));
    }
}
