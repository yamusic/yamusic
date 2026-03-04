use ratatui::{Frame, layout::Rect, style::Color};

use crate::app::components::widgets::EqGraph;
use crate::app::components::widgets::eq_graph::EqBand;

use super::base::{EffectCategory, EffectMeta, ParamMeta};

const EQ_BANDS: [f32; 10] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
const EQ_NAMES: [&str; 10] = [
    "32", "64", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
];

pub struct EqRenderer {
    meta: EffectMeta,
}

impl EqRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "eq",
                name: "EQ",
                icon: "󰓃",
                description: "10-band parametric equalizer",
                category: EffectCategory::Eq,
                params: EQ_NAMES
                    .iter()
                    .map(|name| ParamMeta {
                        name,
                        suffix: "dB",
                        min: -12.0,
                        max: 12.0,
                        default: 0.0,
                        step: 1.0,
                    })
                    .collect(),
            },
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        vals: &[f32],
        focused_band: usize,
        accent: Color,
        muted: Color,
        text: Color,
    ) {
        let bands: Vec<EqBand> = EQ_BANDS
            .iter()
            .enumerate()
            .map(|(i, freq)| EqBand {
                freq: *freq,
                gain_db: vals.get(i).copied().unwrap_or(0.0),
                label: EQ_NAMES[i].to_string(),
            })
            .collect();

        EqGraph::new(bands)
            .focused_band(Some(focused_band))
            .colors(accent, muted, text)
            .render(frame, area);
    }
}

impl EqRenderer {
    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
