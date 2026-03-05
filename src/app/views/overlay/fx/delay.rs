use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};

const DEFAULT_TIME_L: f32 = 500.0;
const DEFAULT_TIME_R: f32 = 500.0;
const DEFAULT_FEEDBACK: f32 = 0.5;
const FEEDBACK_WEIGHT: f32 = 0.15;
const DEFAULT_MIX: f32 = 0.3;

const MAX_TIME_MS: f32 = 1500.0;
const TAP_THRESHOLD: f32 = 0.08;
const MIN_AMP: f32 = 0.03;

const COLOR_L: Color = Color::Cyan;
const COLOR_R: Color = Color::Magenta;
const COLOR_FB: Color = Color::Green;

pub struct DelayRenderer {
    meta: EffectMeta,
}

impl Default for DelayRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DelayRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "delay",
                name: "Delay",
                description: "Stereo delay",
                category: EffectCategory::Spatial,
                params: vec![
                    ParamMeta {
                        name: "Time L",
                        suffix: "ms",
                        min: 10.0,
                        max: 2000.0,
                        default: DEFAULT_TIME_L,
                        step: 10.0,
                    },
                    ParamMeta {
                        name: "Time R",
                        suffix: "ms",
                        min: 10.0,
                        max: 2000.0,
                        default: DEFAULT_TIME_R,
                        step: 10.0,
                    },
                    ParamMeta {
                        name: "Feedback",
                        suffix: "%",
                        min: 0.0,
                        max: 0.95,
                        default: DEFAULT_FEEDBACK,
                        step: 0.05,
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

    pub fn render(&self, frame: &mut Frame, area: Rect, vals: &[f32], accent: Color, muted: Color) {
        if area.width < 20 || area.height < 7 {
            return;
        }

        let time_l = vals.first().copied().unwrap_or(DEFAULT_TIME_L);
        let time_r = vals.get(1).copied().unwrap_or(DEFAULT_TIME_R);
        let feedback = vals.get(2).copied().unwrap_or(DEFAULT_FEEDBACK);
        let mix = vals.get(3).copied().unwrap_or(DEFAULT_MIX);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let content_width = inner_area.width as usize;

        let lines = vec![
            self.build_channel_line(content_width, "L ", time_l, feedback, mix, muted, COLOR_L),
            self.build_channel_line(content_width, "R ", time_r, feedback, mix, muted, COLOR_R),
            Line::from(Span::styled(
                "═".repeat(content_width),
                Style::default().fg(accent),
            )),
            self.build_feedback_line(content_width, feedback, muted),
            self.build_timeline_line(content_width, muted),
        ];

        frame.render_widget(Paragraph::new(lines), inner_area);
    }

    fn build_channel_line(
        &self,
        width: usize,
        label: &str,
        time_ms: f32,
        feedback: f32,
        mix: f32,
        muted: Color,
        color: Color,
    ) -> Line<'static> {
        let content_w = width.saturating_sub(label.len());

        let visualizer: String = (0..content_w)
            .map(|i| {
                let time_pos = (i as f32 / content_w as f32) * MAX_TIME_MS;

                let tap_index = (time_pos / time_ms) as i32;
                let tap_phase = (time_pos % time_ms) / time_ms;

                let decay_factor = (1.0 - FEEDBACK_WEIGHT) + (feedback * FEEDBACK_WEIGHT);
                let amp = decay_factor.powi(tap_index) * mix;

                if tap_phase < TAP_THRESHOLD && amp > MIN_AMP {
                    self.intensity_to_char(amp)
                } else {
                    '·'
                }
            })
            .collect();

        Line::from(vec![
            Span::styled(label.to_string(), Style::default().fg(muted)),
            Span::styled(visualizer, Style::default().fg(color)),
        ])
    }

    fn build_feedback_line(&self, width: usize, feedback: f32, muted: Color) -> Line<'static> {
        let label = "FB ";
        let content_w = width.saturating_sub(label.len());

        let fb_decay: String = (0..content_w)
            .map(|i| {
                let t = i as f32 / width as f32;
                let amp = feedback.powf(t * 5.0);
                self.intensity_to_char(amp)
            })
            .collect();

        Line::from(vec![
            Span::styled(label, Style::default().fg(muted)),
            Span::styled(fb_decay, Style::default().fg(COLOR_FB)),
        ])
    }

    fn build_timeline_line(&self, width: usize, muted: Color) -> Line<'static> {
        let label = "   ";
        let content_w = width.saturating_sub(label.len());
        let segment = (content_w / 8).max(1);

        let timeline: String = (0..content_w)
            .map(|i| {
                if i > 0 && i % segment == 0 {
                    '┼'
                } else {
                    '─'
                }
            })
            .collect();

        Line::from(vec![
            Span::styled(label, Style::default()),
            Span::styled(timeline, Style::default().fg(muted)),
        ])
    }

    fn intensity_to_char(&self, amp: f32) -> char {
        match amp {
            a if a > 0.6 => '█',
            a if a > 0.4 => '▓',
            a if a > 0.2 => '▒',
            a if a > 0.05 => '░',
            _ => '·',
        }
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
