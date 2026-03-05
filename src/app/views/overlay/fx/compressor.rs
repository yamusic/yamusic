use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};
use super::util::{ARC_ANGLE_END, ARC_ANGLE_START, ASPECT_RATIO, render_cell_line};

const MAX_METER_DB: f32 = 40.0;
const TICK_VALUES: &[f32] = &[0.0, 8.0, 16.0, 24.0, 32.0, 40.0];

const GREEN_THRESHOLD: f64 = 12.0;
const YELLOW_THRESHOLD: f64 = 24.0;

const COLOR_GREEN: Color = Color::Rgb(50, 190, 70);
const COLOR_YELLOW: Color = Color::Rgb(200, 200, 40);
const COLOR_RED: Color = Color::Rgb(220, 60, 40);
const COLOR_EMPTY: Color = Color::Rgb(40, 42, 48);
const COLOR_LABEL: Color = Color::Rgb(180, 180, 190);
const COLOR_SHADOW: Color = Color::Rgb(80, 80, 90);

pub struct CompressorRenderer {
    meta: EffectMeta,
}

impl Default for CompressorRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl CompressorRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "compressor",
                name: "Compressor",
                description: "Tames peaks and amplifies quiet sounds",
                category: EffectCategory::Dynamics,
                params: vec![
                    ParamMeta {
                        name: "Threshold",
                        suffix: "dB",
                        min: -40.0,
                        max: 0.0,
                        default: -12.0,
                        step: 1.0,
                    },
                    ParamMeta {
                        name: "Ratio",
                        suffix: ":1",
                        min: 1.0,
                        max: 20.0,
                        default: 4.0,
                        step: 0.5,
                    },
                    ParamMeta {
                        name: "Attack",
                        suffix: "ms",
                        min: 0.1,
                        max: 100.0,
                        default: 5.0,
                        step: 1.0,
                    },
                    ParamMeta {
                        name: "Release",
                        suffix: "ms",
                        min: 10.0,
                        max: 1000.0,
                        default: 100.0,
                        step: 10.0,
                    },
                ],
            },
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, vals: &[f32], accent: Color, muted: Color) {
        if area.width < 20 || area.height < 10 {
            return;
        }
        let threshold = vals.first().copied().unwrap_or(-12.0);
        let ratio = vals.get(1).copied().unwrap_or(4.0).max(1.0);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.render_gauge(frame, inner, threshold, ratio, accent, muted);
    }

    fn render_gauge(
        &self,
        frame: &mut Frame,
        area: Rect,
        threshold: f32,
        ratio: f32,
        accent: Color,
        muted: Color,
    ) {
        let (w, h) = (area.width as usize, area.height as usize);

        let over_threshold = (-threshold).max(0.0);
        let gain_reduction = (over_threshold * (1.0 - 1.0 / ratio) * 1.06).clamp(0.0, MAX_METER_DB);
        let needle_frac = (gain_reduction / MAX_METER_DB) as f64;

        let cx = w as f64 / 2.0;
        let cy = h as f64 * 1.35;
        let radius = ((cy - 1.2).min((cx - 2.0) / ASPECT_RATIO)).max(2.0);

        let r_outer = radius;
        let r_inner = radius - (h as f64 * 0.32).min(radius * 0.4).max(2.0);
        let r_shadow = (r_inner - (h as f64 * 0.18).min(r_inner * 0.4).max(1.0)).max(0.0);
        let needle_angle = ARC_ANGLE_START + (ARC_ANGLE_END - ARC_ANGLE_START) * needle_frac;

        let mut grid = vec![vec![(' ', muted); w]; h];

        for row in 0..h {
            let dy = row as f64 - cy;
            for col in 0..w {
                let dx = (col as f64 - cx) / ASPECT_RATIO;
                let dist = (dx * dx + dy * dy).sqrt();
                let angle = (-dy).atan2(dx);

                if angle < ARC_ANGLE_END - 0.01 || angle > ARC_ANGLE_START + 0.01 {
                    continue;
                }

                let frac_along_arc = (angle - ARC_ANGLE_END) / (ARC_ANGLE_START - ARC_ANGLE_END);
                let is_filled = frac_along_arc >= (1.0 - needle_frac);

                if dist >= r_inner && dist <= r_outer + 0.5 {
                    let db_at_pos = (1.0 - frac_along_arc) * MAX_METER_DB as f64;
                    let color = if db_at_pos < GREEN_THRESHOLD {
                        COLOR_GREEN
                    } else if db_at_pos < YELLOW_THRESHOLD {
                        COLOR_YELLOW
                    } else {
                        COLOR_RED
                    };
                    grid[row][col] = if is_filled {
                        ('▓', color)
                    } else {
                        ('░', COLOR_EMPTY)
                    };
                } else if dist >= r_shadow && dist < r_inner && is_filled {
                    grid[row][col] = ('·', COLOR_SHADOW);
                }

                if needle_frac < 0.98 {
                    if (angle - needle_angle).abs() < 0.03 && dist >= 1.0 && dist <= r_outer + 0.2 {
                        grid[row][col] = ('┃', Color::White);
                    }
                }

                if dist < 1.2 && row >= h.saturating_sub(1) {
                    grid[row][col] = ('●', accent);
                }
            }
        }

        let label_r = r_outer + 1.2;
        for &db in TICK_VALUES {
            let frac = db as f64 / MAX_METER_DB as f64;
            let angle = ARC_ANGLE_START + frac * (ARC_ANGLE_END - ARC_ANGLE_START);
            let lx = cx + label_r * angle.cos() * ASPECT_RATIO;
            let ly = cy - label_r * angle.sin();
            let label = format!("{}", db as u32);
            let (sc, r) = (
                lx.round() as isize - (label.len() as isize / 2),
                ly.round() as isize,
            );
            for (i, ch) in label.chars().enumerate() {
                let c = sc + i as isize;
                if c >= 0 && (c as usize) < w && r >= 0 && (r as usize) < h {
                    grid[r as usize][c as usize] = (ch, COLOR_LABEL);
                }
            }
        }

        for (idx, row_data) in grid.into_iter().enumerate() {
            render_cell_line(frame, area, idx, &row_data);
        }
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
