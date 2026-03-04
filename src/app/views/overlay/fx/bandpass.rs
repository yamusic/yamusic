use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};

pub struct BandpassRenderer {
    meta: EffectMeta,
}

impl Default for BandpassRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl BandpassRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "bandpass",
                name: "Bandpass",
                icon: "󰺢",
                description: "Isolates a frequency band",
                category: EffectCategory::Filter,
                params: vec![
                    ParamMeta {
                        name: "Center",
                        suffix: "Hz",
                        min: 100.0,
                        max: 10000.0,
                        default: 500.0,
                        step: 50.0,
                    },
                    ParamMeta {
                        name: "Q",
                        suffix: "",
                        min: 0.5,
                        max: 30.0,
                        default: 1.0,
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

        let center = vals.first().copied().unwrap_or(440.0);
        let q = vals.get(1).copied().unwrap_or(10.0);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let width = inner.width as usize;
        let height = inner.height.min(4) as usize;
        if width < 4 || height == 0 {
            return;
        }

        let center_pos =
            (((center.log10() - 2.0) / 2.0 * width as f32) as usize).min(width.saturating_sub(1));
        let bandwidth = (width as f32 / (q * 4.0)) as usize;

        let mut grid = vec![vec![' '; width]; height];

        for x in 0..width {
            let dist = (x as isize - center_pos as isize).abs() as f32;
            let normalized_dist = dist / bandwidth.max(1) as f32;

            if normalized_dist < 0.15 {
                grid[0][x] = '█';
            } else if normalized_dist < 0.8 {
                let fade_row = ((normalized_dist - 0.15) / 0.65 * height as f32) as usize;

                for y in 0..height {
                    grid[y][x] = match y {
                        y if y <= fade_row => '▓',
                        y if y == fade_row + 1 => '░',
                        _ => ' ',
                    };
                }
            } else {
                grid[height - 1][x] = '·';
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
