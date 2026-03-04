use ratatui::style::Color;

#[derive(Clone)]
pub struct EffectMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub description: &'static str,
    pub category: EffectCategory,
    pub params: Vec<ParamMeta>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EffectCategory {
    Eq,
    Dynamics,
    Filter,
    Spatial,
    Modulation,
    Distortion,
    Utility,
}

impl EffectCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Eq => "EQ",
            Self::Filter => "Filter",
            Self::Dynamics => "Dynamics",
            Self::Spatial => "Spatial",
            Self::Modulation => "Modulation",
            Self::Distortion => "Distortion",
            Self::Utility => "Utility",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Eq => Color::Magenta,
            Self::Filter => Color::Cyan,
            Self::Dynamics => Color::Green,
            Self::Spatial => Color::Blue,
            Self::Modulation => Color::Yellow,
            Self::Distortion => Color::Red,
            Self::Utility => Color::DarkGray,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Eq => "󰺢",
            Self::Filter => "",
            Self::Dynamics => "󰘢",
            Self::Spatial => "󰗅",
            Self::Modulation => "󰥛",
            Self::Distortion => "󱐋",
            Self::Utility => "󰌨",
        }
    }
}

#[derive(Clone)]
pub struct ParamMeta {
    pub name: &'static str,
    pub suffix: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
}
