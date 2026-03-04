use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};
use super::util::edge_fade_curve;
use crate::framework::theme::ThemeColor;

const DEFAULT_RATE_HZ: f32 = 1.5;
const DEFAULT_DEPTH: f32 = 0.7;
const DEFAULT_MIX: f32 = 0.7;

const MAX_VOICE_PAIRS: usize = 3;
const BEAM_WIDTH_BASE: f32 = 0.6;
const BEAM_WIDTH_MIX_FACTOR: f32 = 0.3;
const FADE_EDGE: f32 = 0.15;

pub struct ChorusRenderer {
    meta: EffectMeta,
}

impl Default for ChorusRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChorusRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "chorus",
                name: "Chorus",
                icon: "󰝚",
                description: "Modulated delay",
                category: EffectCategory::Modulation,
                params: vec![
                    ParamMeta {
                        name: "Rate",
                        suffix: "Hz",
                        min: 0.1,
                        max: 5.0,
                        default: 1.5,
                        step: 0.1,
                    },
                    ParamMeta {
                        name: "Depth",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: 0.7,
                        step: 0.05,
                    },
                    ParamMeta {
                        name: "Mix",
                        suffix: "%",
                        min: 0.0,
                        max: 1.0,
                        default: 0.7,
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
        if area.width < 20 || area.height < 8 {
            return;
        }

        let rate = vals.first().copied().unwrap_or(DEFAULT_RATE_HZ);
        let depth = vals.get(1).copied().unwrap_or(DEFAULT_DEPTH);
        let mix = vals.get(2).copied().unwrap_or(DEFAULT_MIX);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        self.render_beams(frame, inner_area, rate, depth, mix, accent, muted);
    }

    fn render_beams(
        &self,
        frame: &mut Frame,
        area: Rect,
        rate: f32,
        depth: f32,
        mix: f32,
        accent: Color,
        muted: Color,
    ) {
        let w = area.width as usize;
        let h = area.height as usize;

        if w < 8 || h < 4 {
            return;
        }

        let center_y = h / 2;
        let depth_scaled = depth.clamp(0.0, 1.0) * MAX_VOICE_PAIRS as f32;
        let voice_pairs = depth_scaled.ceil() as usize;
        let beam_gap = 1.max(h / 10);

        let beam_length = (w as f32 * (BEAM_WIDTH_BASE + mix * BEAM_WIDTH_MIX_FACTOR)) as usize;
        let time = (rate * 2.0) % std::f32::consts::TAU;
        let bg_color = crate::framework::theme::global_theme().color("background");

        let mut lines = vec![Line::default(); h];

        lines[center_y] = self.build_beam_line(0, beam_length, time, 0, mix, 1.0, bg_color, accent);

        for voice_idx in 1..=voice_pairs {
            let activation = self.voice_activation(voice_idx, depth_scaled);
            if activation < 0.01 {
                continue;
            }

            let offset = voice_idx * beam_gap;
            let voice_line = self.build_beam_line(
                0,
                beam_length,
                time,
                voice_idx,
                mix,
                activation,
                bg_color,
                muted,
            );

            if center_y >= offset {
                lines[center_y - offset] = voice_line.clone();
            }
            if center_y + offset < h {
                lines[center_y + offset] = voice_line;
            }
        }

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn build_beam_line(
        &self,
        start_idx: usize,
        base_length: usize,
        time: f32,
        voice_idx: usize,
        mix: f32,
        activation: f32,
        bg_color: Color,
        indicator_color: Color,
    ) -> Line<'static> {
        let mut spans = Vec::new();
        let is_center = voice_idx == 0;

        let indicator = if is_center { "○ " } else { "· " };
        if start_idx >= indicator.len() {
            let padding = start_idx - indicator.len();
            spans.push(Span::raw(" ".repeat(padding)));
            spans.push(Span::styled(
                indicator,
                Style::default().fg(indicator_color),
            ));
        } else {
            spans.push(Span::raw(" ".repeat(start_idx)));
        }

        let voice_fade_factor = 1.0 - (voice_idx as f32 / (MAX_VOICE_PAIRS + 1) as f32) * 0.3;
        let effective_length = (base_length as f32 * voice_fade_factor) as usize;

        for beam_col in 0..effective_length {
            let progress = beam_col as f32 / effective_length as f32;
            let spatial_fade = edge_fade_curve(progress, FADE_EDGE) * voice_fade_factor;
            let total_fade = spatial_fade * activation;

            let (char_to_draw, beam_color) = if is_center {
                let shimmer = (time + progress * 3.0).sin() * 0.15 + 0.85;
                let brightness = (220.0 * shimmer) as u8;
                let c = Color::Rgb(brightness, brightness, brightness);
                (self.beam_char(spatial_fade, progress, 1.0), c)
            } else {
                let phase = voice_idx as f32 * 0.8;
                let shimmer = (time + progress * 3.0 + phase).sin() * 0.2 + 0.6;
                let brightness = (180.0 * shimmer * mix * voice_fade_factor) as u8;
                let hue_shift = voice_idx as u8 * 10;
                let c = Color::Rgb(
                    brightness.saturating_sub(hue_shift),
                    brightness.saturating_sub(hue_shift / 2),
                    brightness,
                );
                (self.beam_char(total_fade, progress, voice_fade_factor), c)
            };

            spans.push(Span::styled(
                char_to_draw.to_string(),
                Style::default().fg(self.blend_color(beam_color, bg_color, total_fade)),
            ));
        }

        Line::from(spans)
    }

    fn beam_char(&self, fade: f32, progress: f32, length_factor: f32) -> char {
        if fade < 0.3 {
            '·'
        } else if progress < 0.8 * length_factor {
            '━'
        } else {
            '╸'
        }
    }

    fn voice_activation(&self, idx: usize, depth_scaled: f32) -> f32 {
        if idx == 0 {
            return 1.0;
        }
        let threshold = idx as f32 - 1.0;
        (depth_scaled - threshold).clamp(0.0, 1.0).powf(0.8)
    }

    fn blend_color(&self, fg: Color, bg: Color, fade: f32) -> Color {
        ThemeColor::from(fg)
            .blend(ThemeColor::from(bg), 1.0 - fade)
            .to_ratatui()
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
