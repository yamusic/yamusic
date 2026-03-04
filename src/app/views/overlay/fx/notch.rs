use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta, ParamMeta};

pub struct NotchRenderer {
    meta: EffectMeta,
}

impl Default for NotchRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl NotchRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "notch",
                name: "Notch",
                icon: "󰸱",
                description: "Removes a narrow frequency band",
                category: EffectCategory::Filter,
                params: vec![
                    ParamMeta {
                        name: "Center",
                        suffix: "Hz",
                        min: 20.0,
                        max: 10000.0,
                        default: 50.0,
                        step: 50.0,
                    },
                    ParamMeta {
                        name: "Q",
                        suffix: "",
                        min: 0.5,
                        max: 30.0,
                        default: 5.0,
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

        let center = vals.first().copied().unwrap_or(60.0);
        let q = vals.get(1).copied().unwrap_or(5.0);

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

        let center_pos =
            (((center.log10() - 1.3) / 2.7 * width as f32) as usize).min(width.saturating_sub(1));
        let bandwidth = (width as f32 / (q * 4.0)) as usize;

        let max_row = height - 1;

        let mut grid = vec![vec![' '; width]; height];

        for x in 0..width {
            let dist = (x as isize - center_pos as isize).abs() as f32;
            let normalized_dist = dist / bandwidth.max(1) as f32;

            if normalized_dist < 0.15 {
                grid[max_row][x] = '·';
            } else if normalized_dist < 0.8 {
                let rise_t = (normalized_dist - 0.15) / 0.65;
                let rise_row = max_row - (rise_t * max_row as f32).round() as usize;
                let rise_row = rise_row.min(max_row);

                if rise_row < height {
                    grid[rise_row][x] = '░';
                }
                for y in 0..rise_row {
                    grid[y][x] = '▀';
                }
            } else {
                grid[0][x] = '▀';
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
