use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ratatui::style::{Color, Modifier, Style};

use crate::framework::signals::Signal;
use crate::util::colors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeColor {
    Rgb(u8, u8, u8),
    Indexed(u8),
    Named(Color),
    #[default]
    Reset,
}

impl ThemeColor {
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(ThemeColor::Rgb(r, g, b))
    }

    pub fn to_ratatui(self) -> Color {
        match self {
            ThemeColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
            ThemeColor::Indexed(i) => Color::Indexed(i),
            ThemeColor::Named(c) => c,
            ThemeColor::Reset => Color::Reset,
        }
    }

    pub fn blend(self, other: ThemeColor, factor: f32) -> ThemeColor {
        match (self, other) {
            (ThemeColor::Rgb(r1, g1, b1), ThemeColor::Rgb(r2, g2, b2)) => {
                let blend = |a: u8, b: u8| -> u8 {
                    let fa = a as f32;
                    let fb = b as f32;
                    (fa + (fb - fa) * factor).round() as u8
                };
                ThemeColor::Rgb(blend(r1, r2), blend(g1, g2), blend(b1, b2))
            }
            _ => {
                if factor > 0.5 {
                    other
                } else {
                    self
                }
            }
        }
    }

    pub fn lighten(self, factor: f32) -> ThemeColor {
        match self {
            ThemeColor::Rgb(r, g, b) => {
                let adjust = |c: u8| -> u8 {
                    let fc = c as f32;
                    (fc + (255.0 - fc) * factor).min(255.0).round() as u8
                };
                ThemeColor::Rgb(adjust(r), adjust(g), adjust(b))
            }
            _ => self,
        }
    }

    pub fn darken(self, factor: f32) -> ThemeColor {
        match self {
            ThemeColor::Rgb(r, g, b) => {
                let adjust = |c: u8| -> u8 {
                    let fc = c as f32;
                    (fc * (1.0 - factor)).round() as u8
                };
                ThemeColor::Rgb(adjust(r), adjust(g), adjust(b))
            }
            _ => self,
        }
    }
}

impl From<Color> for ThemeColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Rgb(r, g, b) => ThemeColor::Rgb(r, g, b),
            Color::Indexed(i) => ThemeColor::Indexed(i),
            _ => ThemeColor::Named(color),
        }
    }
}

impl From<ThemeColor> for Color {
    fn from(color: ThemeColor) -> Self {
        color.to_ratatui()
    }
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub name: String,
    pub accent: ThemeColor,
    pub accent_secondary: ThemeColor,
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub muted: ThemeColor,
    pub border: ThemeColor,
    pub border_focused: ThemeColor,
    pub selection_bg: ThemeColor,
    pub selection_fg: ThemeColor,
    pub error: ThemeColor,
    pub warning: ThemeColor,
    pub success: ThemeColor,
    pub info: ThemeColor,
    pub custom: HashMap<String, ThemeColor>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            accent: ThemeColor::from(colors::PRIMARY),
            accent_secondary: ThemeColor::from(colors::SECONDARY),
            background: ThemeColor::from(colors::BACKGROUND),
            foreground: ThemeColor::Named(Color::White),
            muted: ThemeColor::from(colors::NEUTRAL),
            border: ThemeColor::from(colors::NEUTRAL),
            border_focused: ThemeColor::Rgb(160, 160, 160),
            selection_bg: ThemeColor::from(colors::BACKGROUND),
            selection_fg: ThemeColor::from(colors::PRIMARY),
            error: ThemeColor::Named(Color::Red),
            warning: ThemeColor::Named(Color::Yellow),
            success: ThemeColor::Named(Color::Green),
            info: ThemeColor::Named(Color::Cyan),
            custom: HashMap::new(),
        }
    }
}

impl ThemeConfig {
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            accent: ThemeColor::from(colors::PRIMARY),
            accent_secondary: ThemeColor::from(colors::SECONDARY),
            background: ThemeColor::from(colors::BACKGROUND),
            foreground: ThemeColor::Named(Color::White),
            muted: ThemeColor::from(colors::NEUTRAL),
            border: ThemeColor::from(colors::NEUTRAL),
            border_focused: ThemeColor::Rgb(160, 160, 160),
            selection_bg: ThemeColor::from(colors::BACKGROUND),
            selection_fg: ThemeColor::from(colors::PRIMARY),
            error: ThemeColor::Named(Color::Red),
            warning: ThemeColor::Named(Color::Yellow),
            success: ThemeColor::Named(Color::Green),
            info: ThemeColor::Named(Color::Cyan),
            custom: HashMap::new(),
        }
    }

    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            accent: ThemeColor::from_hex("7C3AED").unwrap(),
            accent_secondary: ThemeColor::from_hex("EC4899").unwrap(),
            background: ThemeColor::Rgb(255, 255, 255),
            foreground: ThemeColor::Rgb(24, 24, 27),
            muted: ThemeColor::Rgb(161, 161, 170),
            border: ThemeColor::Rgb(228, 228, 231),
            border_focused: ThemeColor::from_hex("7C3AED").unwrap(),
            selection_bg: ThemeColor::from_hex("7C3AED").unwrap(),
            selection_fg: ThemeColor::Rgb(255, 255, 255),
            error: ThemeColor::Rgb(220, 38, 38),
            warning: ThemeColor::Rgb(202, 138, 4),
            success: ThemeColor::Rgb(22, 163, 74),
            info: ThemeColor::Rgb(2, 132, 199),
            custom: HashMap::new(),
        }
    }

    pub fn custom(&self, name: &str) -> Option<ThemeColor> {
        self.custom.get(name).copied()
    }

    pub fn set_custom(&mut self, name: impl Into<String>, color: ThemeColor) {
        self.custom.insert(name.into(), color);
    }
}

#[derive(Debug, Clone)]
pub struct ThemeStyles {
    pub text: Style,
    pub text_muted: Style,
    pub text_bold: Style,
    pub heading: Style,
    pub block: Style,
    pub block_focused: Style,
    pub selected: Style,
    pub highlighted: Style,
    pub error: Style,
    pub warning: Style,
    pub success: Style,
    pub info: Style,
    pub accent: Style,
    pub button: Style,
    pub button_focused: Style,
    pub input: Style,
    pub input_focused: Style,
    pub progress_bg: Style,
    pub progress_fg: Style,
}

impl ThemeStyles {
    pub fn from_config(config: &ThemeConfig) -> Self {
        Self {
            text: Style::default()
                .fg(config.foreground.to_ratatui())
                .bg(config.background.to_ratatui()),
            text_muted: Style::default().fg(config.muted.to_ratatui()),
            text_bold: Style::default()
                .fg(config.foreground.to_ratatui())
                .add_modifier(Modifier::BOLD),
            heading: Style::default()
                .fg(config.foreground.to_ratatui())
                .add_modifier(Modifier::BOLD),
            block: Style::default().fg(config.border.to_ratatui()),
            block_focused: Style::default().fg(config.border_focused.to_ratatui()),
            selected: Style::default()
                .fg(config.selection_fg.to_ratatui())
                .bg(config.selection_bg.to_ratatui()),
            highlighted: Style::default()
                .fg(config.foreground.to_ratatui())
                .bg(config.border.to_ratatui()),
            error: Style::default().fg(config.error.to_ratatui()),
            warning: Style::default().fg(config.warning.to_ratatui()),
            success: Style::default().fg(config.success.to_ratatui()),
            info: Style::default().fg(config.info.to_ratatui()),
            accent: Style::default().fg(config.accent.to_ratatui()),
            button: Style::default()
                .fg(config.foreground.to_ratatui())
                .bg(config.border.to_ratatui()),
            button_focused: Style::default()
                .fg(config.selection_fg.to_ratatui())
                .bg(config.accent.to_ratatui()),
            input: Style::default()
                .fg(config.foreground.to_ratatui())
                .bg(config.background.to_ratatui()),
            input_focused: Style::default()
                .fg(config.foreground.to_ratatui())
                .bg(config.background.to_ratatui()),
            progress_bg: Style::default()
                .fg(config.accent_secondary.to_ratatui())
                .bg(config.background.to_ratatui()),
            progress_fg: Style::default()
                .fg(config.accent.to_ratatui())
                .bg(config.accent_secondary.to_ratatui()),
        }
    }
}

pub struct Theme {
    config: Signal<ThemeConfig>,
    styles: Signal<ThemeStyles>,
    theme_path: Option<PathBuf>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeConfig::default())
    }
}

impl Theme {
    pub fn new(config: ThemeConfig) -> Self {
        let styles = ThemeStyles::from_config(&config);
        Self {
            config: Signal::new(config),
            styles: Signal::new(styles),
            theme_path: None,
        }
    }

    pub fn with_file(path: impl AsRef<Path>) -> Self {
        let config = Self::load_from_file(path.as_ref()).unwrap_or_default();
        let styles = ThemeStyles::from_config(&config);

        Self {
            config: Signal::new(config),
            styles: Signal::new(styles),
            theme_path: Some(path.as_ref().to_path_buf()),
        }
    }

    fn load_from_file(path: &Path) -> Option<ThemeConfig> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::parse_toml(&content)
    }

    fn parse_toml(content: &str) -> Option<ThemeConfig> {
        let mut config = ThemeConfig::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');

                match key {
                    "name" => config.name = value.to_string(),
                    "accent" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.accent = c;
                        }
                    }
                    "accent_secondary" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.accent_secondary = c;
                        }
                    }
                    "background" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.background = c;
                        }
                    }
                    "foreground" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.foreground = c;
                        }
                    }
                    "muted" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.muted = c;
                        }
                    }
                    "border" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.border = c;
                        }
                    }
                    "border_focused" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.border_focused = c;
                        }
                    }
                    "error" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.error = c;
                        }
                    }
                    "warning" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.warning = c;
                        }
                    }
                    "success" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.success = c;
                        }
                    }
                    "info" => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.info = c;
                        }
                    }
                    _ => {
                        if let Some(c) = ThemeColor::from_hex(value) {
                            config.custom.insert(key.to_string(), c);
                        }
                    }
                }
            }
        }

        Some(config)
    }

    pub fn config(&self) -> &Signal<ThemeConfig> {
        &self.config
    }

    pub fn styles(&self) -> &Signal<ThemeStyles> {
        &self.styles
    }

    pub fn get_config(&self) -> ThemeConfig {
        self.config.get()
    }

    pub fn get_styles(&self) -> ThemeStyles {
        self.styles.get()
    }

    pub fn set_config(&self, config: ThemeConfig) {
        let styles = ThemeStyles::from_config(&config);
        self.config.set(config);
        self.styles.set(styles);
    }

    pub fn reload(&self) -> bool {
        if let Some(path) = &self.theme_path
            && let Some(config) = Self::load_from_file(path)
        {
            self.set_config(config);
            return true;
        }
        false
    }

    pub fn color(&self, name: &str) -> Color {
        self.config.with(|c| {
            match name {
                "accent" => c.accent,
                "accent_secondary" => c.accent_secondary,
                "background" => c.background,
                "foreground" => c.foreground,
                "muted" => c.muted,
                "border" => c.border,
                "border_focused" => c.border_focused,
                "selection_bg" => c.selection_bg,
                "selection_fg" => c.selection_fg,
                "error" => c.error,
                "warning" => c.warning,
                "success" => c.success,
                "info" => c.info,
                _ => c.custom.get(name).copied().unwrap_or(ThemeColor::Reset),
            }
            .to_ratatui()
        })
    }

    pub fn style(&self, name: &str) -> Style {
        self.styles.with(|s| match name {
            "text" => s.text,
            "text_muted" => s.text_muted,
            "text_bold" => s.text_bold,
            "heading" => s.heading,
            "block" => s.block,
            "block_focused" => s.block_focused,
            "selected" => s.selected,
            "highlighted" => s.highlighted,
            "error" => s.error,
            "warning" => s.warning,
            "success" => s.success,
            "info" => s.info,
            "accent" => s.accent,
            "button" => s.button,
            "button_focused" => s.button_focused,
            "input" => s.input,
            "input_focused" => s.input_focused,
            "progress_bg" => s.progress_bg,
            "progress_fg" => s.progress_fg,
            _ => Style::default(),
        })
    }
}

impl Clone for Theme {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            styles: self.styles.clone(),
            theme_path: self.theme_path.clone(),
        }
    }
}

static GLOBAL_THEME: std::sync::OnceLock<Arc<Theme>> = std::sync::OnceLock::new();

pub fn global_theme() -> &'static Arc<Theme> {
    GLOBAL_THEME.get_or_init(|| Arc::new(Theme::default()))
}

pub fn set_global_theme(theme: Arc<Theme>) -> Result<(), Arc<Theme>> {
    GLOBAL_THEME.set(theme)
}

pub fn color(name: &str) -> Color {
    global_theme().color(name)
}

pub fn style(name: &str) -> Style {
    global_theme().style(name)
}
