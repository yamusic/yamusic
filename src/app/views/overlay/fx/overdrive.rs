use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};

const DEFAULT_DRIVE: f32 = 0.5;
const DEFAULT_TONE: f32 = 3000.0;
const DEFAULT_MIX: f32 = 0.5;

pub struct OverdriveRenderer {
    meta: EffectMeta,
}

impl Default for OverdriveRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl OverdriveRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "overdrive",
                name: "Overdrive",
                icon: "󱐋",
                description: "Warm saturation",
                category: EffectCategory::Distortion,
                params: vec![
                    ParamMeta {
                        name: "Drive",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: DEFAULT_DRIVE,
                        step: 0.05,
                    },
                    ParamMeta {
                        name: "Tone",
                        suffix: "Hz",
                        min: 1000.0,
                        max: 10000.0,
                        default: DEFAULT_TONE,
                        step: 100.0,
                    },
                    ParamMeta {
                        name: "Mix",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: DEFAULT_MIX,
                        step: 0.05,
                    },
                ],
            },
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        vals: &[f32],
        accent: Color,
        muted: Color,
        _text: Color,
    ) {
        if area.width < 20 || area.height < 10 {
            return;
        }

        let drive = vals.first().copied().unwrap_or(DEFAULT_DRIVE);
        let mix = vals.get(2).copied().unwrap_or(DEFAULT_MIX);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let w = inner_area.width as usize;
        let h = inner_area.height as usize;

        if w < 6 || h < 4 {
            return;
        }

        let mut lines = Vec::with_capacity(h);

        for row in 0..h.saturating_sub(1) {
            lines.push(self.build_lava_line(w, h.saturating_sub(1), row, drive, mix));
        }

        lines.push(self.build_heat_bar_line(w, drive, muted));

        frame.render_widget(Paragraph::new(lines), inner_area);
    }

    fn build_lava_line(
        &self,
        w: usize,
        h: usize,
        row: usize,
        drive: f32,
        mix: f32,
    ) -> Line<'static> {
        let cy = h as f64 / 2.0;
        let core_thickness = 0.4 + drive as f64 * 2.2;
        let turbulence = drive as f64 * 1.8;
        let intensity = (drive as f64 * 0.6 + mix as f64 * 0.4).clamp(0.0, 1.0);

        let spans: Vec<Span> = (0..w)
            .map(|col| {
                let x = col as f64 / w as f64;
                let y_norm = (row as f64 - cy) / (h as f64 / 2.0);

                let warp = (x * 7.3 + 0.5).sin() * turbulence * 0.4
                    + (x * 13.7 + 2.1).sin() * turbulence * 0.25
                    + (x * 23.1 + 4.7).sin() * turbulence * 0.15;

                let centre_offset = y_norm - warp / (h as f64 / 2.0);
                let dist = centre_offset.abs();

                let core_energy = (-dist / (core_thickness / h as f64 * 2.0))
                    .exp()
                    .clamp(0.0, 1.0);

                let ember = if drive > 0.4 {
                    let seed = ((col as f64 * 17.3 + row as f64 * 31.7).sin() * 43758.5453)
                        .fract()
                        .abs();
                    if seed > 0.92 && dist < core_thickness * 1.5 / (h as f64 / 2.0) {
                        seed * drive as f64
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                let total = (core_energy * intensity + ember).clamp(0.0, 1.0);

                if total < 0.02 {
                    return Span::raw(" ");
                }

                let color = self.intensity_to_color(total);
                let ch = self.intensity_to_char(total);

                Span::styled(ch.to_string(), Style::default().fg(color))
            })
            .collect();

        Line::from(spans)
    }

    fn build_heat_bar_line(&self, w: usize, drive: f32, muted: Color) -> Line<'static> {
        let drive_clamped = drive.clamp(0.0, 1.0);
        let filled_w = (drive_clamped * w as f32) as usize;

        let filled_str: String = (0..filled_w)
            .map(|i| {
                let frac = i as f32 / w as f32;
                if frac > 0.75 {
                    '█'
                } else if frac > 0.5 {
                    '▓'
                } else {
                    '░'
                }
            })
            .collect();

        let empty_str = "·".repeat(w.saturating_sub(filled_w));

        let color = Color::Rgb(
            (60.0 + 195.0 * drive_clamped) as u8,
            (30.0 + 120.0 * drive_clamped.powf(1.5)) as u8,
            15,
        );

        Line::from(vec![
            Span::styled(filled_str, Style::default().fg(color)),
            Span::styled(empty_str, Style::default().fg(muted)),
        ])
    }

    fn intensity_to_char(&self, intensity: f64) -> char {
        match intensity {
            i if i > 0.90 => '█',
            i if i > 0.72 => '▓',
            i if i > 0.50 => '▒',
            i if i > 0.30 => '░',
            i if i > 0.15 => '∙',
            _ => '·',
        }
    }

    fn intensity_to_color(&self, intensity: f64) -> Color {
        Color::Rgb(
            (40.0 + 215.0 * intensity.powf(0.6)) as u8,
            (10.0 + 180.0 * intensity.powf(1.5)) as u8,
            (5.0 + 40.0 * intensity.powf(2.5)) as u8,
        )
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
