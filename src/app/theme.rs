use std::sync::{Arc, OnceLock};

use opaline::{
    Gradient, ThemeInfo, app_theme_dirs, load_from_str, load_theme_by_name_in_dirs,
    names::{gradients as ogradients, styles as ostyles, tokens as otokens},
    set_theme, theme_dirs,
};
use ratatui::style::{Color, Style};

use crate::framework::signals::Signal;

pub const DEFAULT_THEME_DARK: &str = "golden_night";
pub const DEFAULT_THEME_LIGHT: &str = "golden_day";
const THEME_DEFS: [(&str, &str); 2] = [
    (
        DEFAULT_THEME_DARK,
        include_str!("../../theme/golden_night.toml"),
    ),
    (
        DEFAULT_THEME_LIGHT,
        include_str!("../../theme/golden_day.toml"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum ContractToken {
    TextPrimary = 0,
    TextSecondary,
    TextMuted,
    TextDim,
    BgBase,
    BgPanel,
    BgCode,
    BgHighlight,
    BgSelection,
    AccentPrimary,
    AccentSecondary,
    AccentTertiary,
    AccentDeep,
    Success,
    Error,
    Warning,
    Info,
    BorderFocused,
    BorderUnfocused,
    CodeKeyword,
    CodeFunction,
    CodeString,
    CodeNumber,
    CodeComment,
    CodeType,
    CodeLineNumber,
}

impl ContractToken {
    pub const ALL: [Self; 26] = [
        Self::TextPrimary,
        Self::TextSecondary,
        Self::TextMuted,
        Self::TextDim,
        Self::BgBase,
        Self::BgPanel,
        Self::BgCode,
        Self::BgHighlight,
        Self::BgSelection,
        Self::AccentPrimary,
        Self::AccentSecondary,
        Self::AccentTertiary,
        Self::AccentDeep,
        Self::Success,
        Self::Error,
        Self::Warning,
        Self::Info,
        Self::BorderFocused,
        Self::BorderUnfocused,
        Self::CodeKeyword,
        Self::CodeFunction,
        Self::CodeString,
        Self::CodeNumber,
        Self::CodeComment,
        Self::CodeType,
        Self::CodeLineNumber,
    ];

    pub const fn as_index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::TextPrimary => otokens::TEXT_PRIMARY,
            Self::TextSecondary => otokens::TEXT_SECONDARY,
            Self::TextMuted => otokens::TEXT_MUTED,
            Self::TextDim => otokens::TEXT_DIM,
            Self::BgBase => otokens::BG_BASE,
            Self::BgPanel => otokens::BG_PANEL,
            Self::BgCode => otokens::BG_CODE,
            Self::BgHighlight => otokens::BG_HIGHLIGHT,
            Self::BgSelection => otokens::BG_SELECTION,
            Self::AccentPrimary => otokens::ACCENT_PRIMARY,
            Self::AccentSecondary => otokens::ACCENT_SECONDARY,
            Self::AccentTertiary => otokens::ACCENT_TERTIARY,
            Self::AccentDeep => otokens::ACCENT_DEEP,
            Self::Success => otokens::SUCCESS,
            Self::Error => otokens::ERROR,
            Self::Warning => otokens::WARNING,
            Self::Info => otokens::INFO,
            Self::BorderFocused => otokens::BORDER_FOCUSED,
            Self::BorderUnfocused => otokens::BORDER_UNFOCUSED,
            Self::CodeKeyword => otokens::CODE_KEYWORD,
            Self::CodeFunction => otokens::CODE_FUNCTION,
            Self::CodeString => otokens::CODE_STRING,
            Self::CodeNumber => otokens::CODE_NUMBER,
            Self::CodeComment => otokens::CODE_COMMENT,
            Self::CodeType => otokens::CODE_TYPE,
            Self::CodeLineNumber => otokens::CODE_LINE_NUMBER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum ContractStyle {
    Keyword = 0,
    LineNumber,
    Selected,
    ActiveSelected,
    FocusedBorder,
    UnfocusedBorder,
    SuccessStyle,
    ErrorStyle,
    WarningStyle,
    InfoStyle,
    Dimmed,
    Muted,
    InlineCode,
}

impl ContractStyle {
    pub const ALL: [Self; 13] = [
        Self::Keyword,
        Self::LineNumber,
        Self::Selected,
        Self::ActiveSelected,
        Self::FocusedBorder,
        Self::UnfocusedBorder,
        Self::SuccessStyle,
        Self::ErrorStyle,
        Self::WarningStyle,
        Self::InfoStyle,
        Self::Dimmed,
        Self::Muted,
        Self::InlineCode,
    ];

    pub const fn as_index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Keyword => ostyles::KEYWORD,
            Self::LineNumber => ostyles::LINE_NUMBER,
            Self::Selected => ostyles::SELECTED,
            Self::ActiveSelected => ostyles::ACTIVE_SELECTED,
            Self::FocusedBorder => ostyles::FOCUSED_BORDER,
            Self::UnfocusedBorder => ostyles::UNFOCUSED_BORDER,
            Self::SuccessStyle => ostyles::SUCCESS_STYLE,
            Self::ErrorStyle => ostyles::ERROR_STYLE,
            Self::WarningStyle => ostyles::WARNING_STYLE,
            Self::InfoStyle => ostyles::INFO_STYLE,
            Self::Dimmed => ostyles::DIMMED,
            Self::Muted => ostyles::MUTED,
            Self::InlineCode => ostyles::INLINE_CODE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
enum ContractGradient {
    Primary = 0,
    Warm,
    Success,
    Error,
    Aurora,
}

impl ContractGradient {
    pub const ALL: [Self; 5] = [
        Self::Primary,
        Self::Warm,
        Self::Success,
        Self::Error,
        Self::Aurora,
    ];

    pub const fn as_index(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => ogradients::PRIMARY,
            Self::Warm => ogradients::WARM,
            Self::Success => ogradients::SUCCESS_GRADIENT,
            Self::Error => ogradients::ERROR_GRADIENT,
            Self::Aurora => ogradients::AURORA,
        }
    }
}

#[derive(Debug, Clone)]
struct ThemeContract {
    tokens: [Color; 26],
    styles: [Style; 13],
    gradients: [Option<Gradient>; 5],
}

impl ThemeContract {
    fn from_theme(theme: &opaline::Theme) -> Self {
        let tokens = std::array::from_fn(|idx| {
            let token = ContractToken::ALL[idx];
            Color::from(theme.color(token.name()))
        });

        let styles = std::array::from_fn(|idx| {
            let style = ContractStyle::ALL[idx];
            Style::from(theme.style(style.name()))
        });

        let gradients = std::array::from_fn(|idx| {
            let gradient = ContractGradient::ALL[idx];
            theme.get_gradient(gradient.name()).cloned()
        });

        Self {
            tokens,
            styles,
            gradients,
        }
    }

    fn token(&self, token: ContractToken) -> Color {
        self.tokens[token.as_index()]
    }

    fn style(&self, style: ContractStyle) -> Style {
        self.styles[style.as_index()]
    }

    fn gradient(&self, gradient: ContractGradient) -> Option<Gradient> {
        self.gradients[gradient.as_index()].clone()
    }
}

pub struct Theme;

#[derive(Debug, Clone, Copy)]
pub struct ThemeTextTokens {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub dim: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeBgTokens {
    pub base: Color,
    pub panel: Color,
    pub code: Color,
    pub highlight: Color,
    pub selection: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeAccentTokens {
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub deep: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeBorderTokens {
    pub focused: Color,
    pub unfocused: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeCodeTokens {
    pub keyword: Color,
    pub function: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub ty: Color,
    pub line_number: Color,
}

#[derive(Debug, Clone)]
pub struct ThemeStyleTokens {
    pub keyword: Style,
    pub line_number: Style,
    pub selected: Style,
    pub active_selected: Style,
    pub focused_border: Style,
    pub unfocused_border: Style,
    pub success_style: Style,
    pub error_style: Style,
    pub warning_style: Style,
    pub info_style: Style,
    pub dimmed: Style,
    pub muted: Style,
    pub inline_code: Style,
}

#[derive(Debug, Clone)]
pub struct ThemeGradientTokens {
    pub primary: Option<Gradient>,
    pub warm: Option<Gradient>,
    pub success_gradient: Option<Gradient>,
    pub error_gradient: Option<Gradient>,
    pub aurora: Option<Gradient>,
}

#[derive(Debug, Clone)]
pub struct ThemeSnapshot {
    pub text: ThemeTextTokens,
    pub bg: ThemeBgTokens,
    pub accent: ThemeAccentTokens,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub border: ThemeBorderTokens,
    pub code: ThemeCodeTokens,

    pub keyword: Style,
    pub line_number: Style,
    pub selected: Style,
    pub active_selected: Style,
    pub focused_border: Style,
    pub unfocused_border: Style,
    pub success_style: Style,
    pub error_style: Style,
    pub warning_style: Style,
    pub info_style: Style,
    pub dimmed: Style,
    pub muted: Style,
    pub inline_code: Style,

    pub styles: ThemeStyleTokens,
    pub gradients: ThemeGradientTokens,
}

impl ThemeSnapshot {
    fn from(contract: &ThemeContract) -> Self {
        let token = |t: ContractToken| contract.token(t);
        let style = |s: ContractStyle| contract.style(s);
        let gradient = |g: ContractGradient| contract.gradient(g);

        let keyword = style(ContractStyle::Keyword);
        let line_number = style(ContractStyle::LineNumber);
        let selected = style(ContractStyle::Selected);
        let active_selected = style(ContractStyle::ActiveSelected);
        let focused_border = style(ContractStyle::FocusedBorder);
        let unfocused_border = style(ContractStyle::UnfocusedBorder);
        let success_style = style(ContractStyle::SuccessStyle);
        let error_style = style(ContractStyle::ErrorStyle);
        let warning_style = style(ContractStyle::WarningStyle);
        let info_style = style(ContractStyle::InfoStyle);
        let dimmed = style(ContractStyle::Dimmed);
        let muted = style(ContractStyle::Muted);
        let inline_code = style(ContractStyle::InlineCode);

        let primary_gradient = gradient(ContractGradient::Primary);
        let warm_gradient = gradient(ContractGradient::Warm);
        let success_gradient = gradient(ContractGradient::Success);
        let error_gradient = gradient(ContractGradient::Error);
        let aurora_gradient = gradient(ContractGradient::Aurora);

        Self {
            text: ThemeTextTokens {
                primary: token(ContractToken::TextPrimary),
                secondary: token(ContractToken::TextSecondary),
                muted: token(ContractToken::TextMuted),
                dim: token(ContractToken::TextDim),
            },
            bg: ThemeBgTokens {
                base: token(ContractToken::BgBase),
                panel: token(ContractToken::BgPanel),
                code: token(ContractToken::BgCode),
                highlight: token(ContractToken::BgHighlight),
                selection: token(ContractToken::BgSelection),
            },
            accent: ThemeAccentTokens {
                primary: token(ContractToken::AccentPrimary),
                secondary: token(ContractToken::AccentSecondary),
                tertiary: token(ContractToken::AccentTertiary),
                deep: token(ContractToken::AccentDeep),
            },
            success: token(ContractToken::Success),
            error: token(ContractToken::Error),
            warning: token(ContractToken::Warning),
            info: token(ContractToken::Info),
            border: ThemeBorderTokens {
                focused: token(ContractToken::BorderFocused),
                unfocused: token(ContractToken::BorderUnfocused),
            },
            code: ThemeCodeTokens {
                keyword: token(ContractToken::CodeKeyword),
                function: token(ContractToken::CodeFunction),
                string: token(ContractToken::CodeString),
                number: token(ContractToken::CodeNumber),
                comment: token(ContractToken::CodeComment),
                ty: token(ContractToken::CodeType),
                line_number: token(ContractToken::CodeLineNumber),
            },
            keyword,
            line_number,
            selected,
            active_selected,
            focused_border,
            unfocused_border,
            success_style,
            error_style,
            warning_style,
            info_style,
            dimmed,
            muted,
            inline_code,
            styles: ThemeStyleTokens {
                keyword,
                line_number,
                selected,
                active_selected,
                focused_border,
                unfocused_border,
                success_style,
                error_style,
                warning_style,
                info_style,
                dimmed,
                muted,
                inline_code,
            },
            gradients: ThemeGradientTokens {
                primary: primary_gradient,
                warm: warm_gradient,
                success_gradient,
                error_gradient,
                aurora: aurora_gradient,
            },
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self
    }
}

static THEME_SNAPSHOT: OnceLock<Signal<ThemeSnapshot>> = OnceLock::new();
static THEME_INIT: OnceLock<()> = OnceLock::new();

fn all_theme_dirs() -> Vec<std::path::PathBuf> {
    let mut builtin = theme_dirs();
    let user = app_theme_dirs("yamusic");

    builtin.extend(user);
    builtin
}

fn embedded_theme(theme_id: &str) -> Option<opaline::Theme> {
    THEME_DEFS
        .iter()
        .find(|(id, _)| *id == theme_id)
        .and_then(|(_, toml)| load_from_str(toml, None).ok())
}

fn load_inner(theme_id: &str) -> bool {
    if let Some(theme) = embedded_theme(theme_id) {
        set_theme(theme);
        return true;
    }

    load_theme_by_name_in_dirs(theme_id, all_theme_dirs()).is_ok()
}

pub fn bootstrap() {
    THEME_INIT.get_or_init(|| {
        let _ = load_inner(DEFAULT_THEME_DARK);
    });
}

fn current() -> Arc<opaline::Theme> {
    opaline::current()
}

fn apply(theme: &opaline::Theme) {
    let cache = ThemeContract::from_theme(theme);
    let snapshot = ThemeSnapshot::from(&cache);

    if let Some(sig) = THEME_SNAPSHOT.get() {
        sig.set(snapshot);
    } else {
        let _ = THEME_SNAPSHOT.set(Signal::new(snapshot));
    }
}

pub fn snapshot() -> &'static Signal<ThemeSnapshot> {
    THEME_SNAPSHOT.get_or_init(|| {
        let theme = current();
        let cache = ThemeContract::from_theme(&theme);
        Signal::new(ThemeSnapshot::from(&cache))
    })
}

pub fn theme() -> ThemeSnapshot {
    snapshot().get()
}

pub fn color(name: &str) -> Color {
    let t = theme();
    match name {
        otokens::ACCENT_PRIMARY => t.accent.primary,
        otokens::ACCENT_SECONDARY => t.accent.secondary,
        otokens::ACCENT_TERTIARY => t.accent.tertiary,
        otokens::ACCENT_DEEP => t.accent.deep,
        otokens::BG_BASE => t.bg.base,
        otokens::BG_PANEL => t.bg.panel,
        otokens::BG_CODE => t.bg.code,
        otokens::BG_HIGHLIGHT => t.bg.highlight,
        otokens::BG_SELECTION => t.bg.selection,
        otokens::TEXT_PRIMARY => t.text.primary,
        otokens::TEXT_SECONDARY => t.text.secondary,
        otokens::TEXT_MUTED => t.text.muted,
        otokens::TEXT_DIM => t.text.dim,
        otokens::BORDER_FOCUSED => t.border.focused,
        otokens::BORDER_UNFOCUSED => t.border.unfocused,
        otokens::SUCCESS => t.success,
        otokens::ERROR => t.error,
        otokens::WARNING => t.warning,
        otokens::INFO => t.info,
        otokens::CODE_KEYWORD => t.code.keyword,
        otokens::CODE_FUNCTION => t.code.function,
        otokens::CODE_STRING => t.code.string,
        otokens::CODE_NUMBER => t.code.number,
        otokens::CODE_COMMENT => t.code.comment,
        otokens::CODE_TYPE => t.code.ty,
        otokens::CODE_LINE_NUMBER => t.code.line_number,
        _ => unimplemented!(),
    }
}

pub fn style(name: &str) -> Style {
    let t = theme();

    match name {
        ostyles::KEYWORD => t.keyword,
        ostyles::LINE_NUMBER => t.line_number,
        ostyles::SELECTED => t.selected,
        ostyles::ACTIVE_SELECTED => t.active_selected,
        ostyles::FOCUSED_BORDER => t.focused_border,
        ostyles::UNFOCUSED_BORDER => t.unfocused_border,
        ostyles::SUCCESS_STYLE => t.success_style,
        ostyles::ERROR_STYLE => t.error_style,
        ostyles::WARNING_STYLE => t.warning_style,
        ostyles::INFO_STYLE => t.info_style,
        ostyles::DIMMED => t.dimmed,
        ostyles::MUTED => t.muted,
        ostyles::INLINE_CODE => t.inline_code,
        _ => unimplemented!(),
    }
}

pub fn load(theme_id: &str) -> bool {
    if !load_inner(theme_id) {
        return false;
    }

    let theme = current();
    apply(&theme);

    true
}

pub fn refresh() {
    let theme = current();
    apply(&theme);
}

pub fn embedded_theme_by_id(theme_id: &str) -> Option<opaline::Theme> {
    embedded_theme(theme_id)
}

pub fn embedded_theme_info(theme_id: &str) -> Option<ThemeInfo> {
    embedded_theme(theme_id).map(|theme| ThemeInfo {
        name: theme_id.to_string(),
        display_name: theme.meta.name.clone(),
        variant: theme.meta.variant,
        author: theme.meta.author.clone().unwrap_or_default(),
        description: theme.meta.description.clone().unwrap_or_default(),
        builtin: true,
        path: None,
    })
}
