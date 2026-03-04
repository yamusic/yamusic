use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};
use super::util::{map_to_width, normalize_q};

pub const HIGHPASS_MIN_HZ: f32 = 20.0;
pub const HIGHPASS_MAX_HZ: f32 = 20_000.0;
pub const Q_MIN: f32 = 0.5;
pub const Q_MAX: f32 = 50.0;
pub const Q_DEFAULT: f32 = 1.0;

pub struct HighpassRenderer {
    meta: EffectMeta,
}

impl Default for HighpassRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl HighpassRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "highpass",
                name: "Highpass",
                icon: "󰘾",
                description: "Isolates high frequencies",
                category: EffectCategory::Filter,
                params: vec![
                    ParamMeta {
                        name: "Cutoff",
                        suffix: "Hz",
                        min: HIGHPASS_MIN_HZ,
                        max: HIGHPASS_MAX_HZ,
                        default: 80.0,
                        step: 20.0,
                    },
                    ParamMeta {
                        name: "Q",
                        suffix: "",
                        min: Q_MIN,
                        max: Q_MAX,
                        default: Q_DEFAULT,
                        step: 0.5,
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
        if area.width < 10 || area.height < 6 {
            return;
        }

        let cutoff = vals.first().copied().unwrap_or(80.0);
        let q = vals.get(1).copied().unwrap_or(Q_DEFAULT).max(0.1);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let width = inner.width as usize;
        let height = inner.height as usize;
        if width < 4 || height < 2 {
            return;
        }

        let max_row = height - 1;

        let cutoff_pos = map_to_width(cutoff, HIGHPASS_MIN_HZ, HIGHPASS_MAX_HZ, width);
        let q_norm = normalize_q(q, Q_MIN, Q_MAX);

        let resonance_gain = if q > 0.707 {
            ((q / 0.707).ln() * 0.8).min(1.5)
        } else {
            0.0
        };

        let max_resonance_rows = (resonance_gain * 2.0).ceil() as usize;
        let passband_row = max_resonance_rows.min(max_row);

        let rolloff_rows = max_row.saturating_sub(passband_row);

        let slope_factor = if q > 20.0 {
            let extreme = (q - 20.0) / 80.0;
            2.0 + extreme * 10.0
        } else {
            0.3 + q_norm * 1.7
        };

        let mut col_rows: Vec<usize> = Vec::with_capacity(width);

        for x in 0..width {
            let row = if x > cutoff_pos {
                let dist_from_cutoff = (x - cutoff_pos) as f32;
                let resonance_width = 2.0 + q_norm * 4.0;

                if resonance_gain > 0.05 && dist_from_cutoff < resonance_width {
                    let t = 1.0 - (dist_from_cutoff / resonance_width);
                    let peak_offset = (resonance_gain * 2.0 * t).round() as usize;
                    passband_row.saturating_sub(peak_offset)
                } else {
                    passband_row
                }
            } else if x == cutoff_pos {
                passband_row
            } else {
                let dist_from_cutoff = (cutoff_pos - x) as f32;

                let stopband_width = if q > 50.0 {
                    (cutoff_pos as f32 * 0.15).max(3.0)
                } else {
                    (cutoff_pos as f32 * 0.6).max(5.0)
                };

                let normalized = (dist_from_cutoff / stopband_width) * slope_factor;
                let drop = (normalized * rolloff_rows as f32).min(rolloff_rows as f32);
                let y = passband_row + drop.round() as usize;
                y.min(max_row)
            };

            col_rows.push(row);
        }

        let mut grid = vec![vec![' '; width]; height];

        for x in 0..width {
            let current_row = col_rows[x];

            let (fill_top, fill_bottom) = if x == 0 {
                (current_row, current_row)
            } else {
                let prev_row = col_rows[x - 1];
                if prev_row <= current_row {
                    (prev_row, current_row)
                } else {
                    (current_row, prev_row)
                }
            };

            let (range_top, range_bottom) = if x + 1 < width {
                let next_row = col_rows[x + 1];
                let top = fill_top.min(next_row);
                let bot = fill_bottom.max(next_row);
                if (next_row as isize - current_row as isize).unsigned_abs() <= 1 {
                    (fill_top.min(current_row), fill_bottom.max(current_row))
                } else {
                    (top.min(current_row), bot.max(current_row))
                }
            } else {
                (fill_top, fill_bottom)
            };

            let is_passband = x > cutoff_pos;
            let is_cutoff = x == cutoff_pos;

            if is_cutoff {
                for r in 0..=passband_row.min(max_row) {
                    grid[r][x] = '│';
                }
            } else if is_passband {
                let dist_from_cutoff = if x > cutoff_pos {
                    (x - cutoff_pos) as f32
                } else {
                    0.0
                };
                let resonance_width = 2.0 + q_norm * 4.0;

                for r in range_top..=range_bottom.min(max_row) {
                    if resonance_gain > 0.1 && dist_from_cutoff < resonance_width {
                        let t = 1.0 - (dist_from_cutoff / resonance_width);
                        let intensity = resonance_gain * t;
                        grid[r][x] = if intensity > 0.8 {
                            '█'
                        } else if intensity > 0.4 {
                            '▓'
                        } else {
                            '▒'
                        };
                    } else {
                        grid[r][x] = '─';
                    }
                }
            } else {
                for r in range_top..=range_bottom.min(max_row) {
                    if r == current_row {
                        grid[r][x] = '·';
                    } else {
                        grid[r][x] = '│';
                    }
                }
            }
        }

        let lines: Vec<Line> = grid
            .into_iter()
            .map(|row| {
                Line::from(Span::styled(
                    row.into_iter().collect::<String>(),
                    Style::default().fg(accent),
                ))
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
