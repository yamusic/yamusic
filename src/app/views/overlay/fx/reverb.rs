use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};
use super::util::render_cell_line;

const DEFAULT_ROOM: f32 = 10.0;
const DEFAULT_DECAY: f32 = 2.0;
const DEFAULT_DAMPING: f32 = 0.5;
const DEFAULT_MIX: f32 = 0.15;

const CORE_RADIUS: f64 = 0.08;
const INNER_GLOW_RADIUS: f64 = 0.16;
const RING_COUNT: usize = 5;
const ASPECT_RATIO: f64 = 2.1;

pub struct ReverbRenderer {
    meta: EffectMeta,
}

impl Default for ReverbRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverbRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "reverb",
                name: "Reverb",
                icon: "󰕾",
                description: "Reverb",
                category: EffectCategory::Spatial,
                params: vec![
                    ParamMeta {
                        name: "Room Size",
                        suffix: "m",
                        min: 5.0,
                        max: 50.0,
                        default: DEFAULT_ROOM,
                        step: 1.0,
                    },
                    ParamMeta {
                        name: "Decay",
                        suffix: "s",
                        min: 0.5,
                        max: 10.0,
                        default: DEFAULT_DECAY,
                        step: 0.1,
                    },
                    ParamMeta {
                        name: "Damping",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: DEFAULT_DAMPING,
                        step: 0.05,
                    },
                    ParamMeta {
                        name: "Mix",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: DEFAULT_MIX,
                        step: 0.01,
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
        _muted: Color,
        _text: Color,
    ) {
        if area.width < 20 || area.height < 12 {
            return;
        }

        let room = vals.first().copied().unwrap_or(DEFAULT_ROOM);
        let decay = vals.get(1).copied().unwrap_or(DEFAULT_DECAY);
        let damping = vals.get(2).copied().unwrap_or(DEFAULT_DAMPING);
        let mix = vals.get(3).copied().unwrap_or(DEFAULT_MIX);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        self.render_bloom(frame, inner_area, room, decay, damping, mix);
    }

    fn render_bloom(
        &self,
        frame: &mut Frame,
        area: Rect,
        room: f32,
        decay: f32,
        damping: f32,
        mix: f32,
    ) {
        let w = area.width as usize;
        let h = area.height as usize;

        if w < 6 || h < 3 {
            return;
        }

        let cx = w as f64 * 0.5;
        let cy = h as f64 * 0.5;

        let room_norm = (room as f64 / 50.0).clamp(0.0, 1.0);
        let decay_norm = (decay as f64 / 10.0).clamp(0.0, 1.0);
        let mix_norm = mix as f64;

        let base_r = (cx / ASPECT_RATIO).min(cy) * 0.95;
        let max_r = base_r * (0.5 + room_norm * 0.7 + decay_norm * 0.3);
        let ring_width = 0.12 + mix_norm * 0.08;

        for row in 0..h {
            let mut row_buffer = vec![(' ', Color::Reset); w];
            let dy = row as f64 - cy;

            for col in 0..w {
                let dx = (col as f64 - cx) / ASPECT_RATIO;
                let dist = (dx * dx + dy * dy).sqrt();
                let norm_dist = dist / max_r;

                if norm_dist > 1.15 {
                    continue;
                }

                if norm_dist < CORE_RADIUS {
                    row_buffer[col] = ('█', Color::White);
                    continue;
                }
                if norm_dist < INNER_GLOW_RADIUS {
                    row_buffer[col] = ('▓', Color::Rgb(220, 230, 255));
                    continue;
                }

                if let Some((ch, color)) = self.calculate_ring_pixel(
                    norm_dist,
                    mix_norm,
                    decay_norm,
                    damping as f64,
                    ring_width,
                ) {
                    row_buffer[col] = (ch, color);
                }
            }

            render_cell_line(frame, area, row, &row_buffer);
        }
    }

    fn calculate_ring_pixel(
        &self,
        norm_dist: f64,
        mix_norm: f64,
        decay_norm: f64,
        damping: f64,
        ring_width: f64,
    ) -> Option<(char, Color)> {
        if norm_dist > 1.0 {
            return None;
        }

        let mut total_energy = 0.0;

        for i in 0..RING_COUNT {
            let ring_center = 0.2 + (i as f64 / RING_COUNT as f64) * 0.75;
            let dist_from_ring = (norm_dist - ring_center).abs();

            if dist_from_ring < ring_width {
                let ring_intensity = (1.0 - dist_from_ring / ring_width).powf(1.2);
                let fade = (1.0 - ring_center * 0.8).powf(0.8 + decay_norm * 0.5);
                total_energy += ring_intensity * fade * (0.7 + mix_norm * 0.5);
            }
        }

        let ambient = ((1.0 - norm_dist).powf(1.2 + decay_norm * 0.3) * (0.3 + mix_norm * 0.4))
            .clamp(0.0, 1.0);
        total_energy = (total_energy + ambient).clamp(0.0, 1.0);

        if total_energy < 0.015 {
            return None;
        }

        let ch = self.energy_to_char(total_energy);
        let color = self.energy_to_color(total_energy, norm_dist, damping);

        Some((ch, color))
    }

    fn energy_to_char(&self, energy: f64) -> char {
        match energy {
            e if e > 0.75 => '█',
            e if e > 0.55 => '▓',
            e if e > 0.35 => '▒',
            e if e > 0.20 => '░',
            e if e > 0.08 => '·',
            _ => '⋅',
        }
    }

    fn energy_to_color(&self, energy: f64, norm_dist: f64, damping: f64) -> Color {
        let warmth = (norm_dist * damping * 1.25).clamp(0.0, 1.0);
        Color::Rgb(
            (100.0 + 150.0 * warmth + 20.0 * energy) as u8,
            (145.0 + 85.0 * (1.0 - warmth) * energy) as u8,
            (220.0 - 130.0 * warmth) as u8,
        )
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
