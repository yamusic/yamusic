use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::base::{EffectCategory, EffectMeta};

const COLOR_WAVE_ENABLED: Color = Color::Rgb(100, 190, 130);
const COLOR_WAVE_DISABLED: Color = Color::Rgb(180, 90, 60);
const COLOR_CENTER_LINE: Color = Color::Rgb(50, 55, 65);

pub struct DcBlockRenderer {
    meta: EffectMeta,
}

impl Default for DcBlockRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DcBlockRenderer {
    pub fn new() -> Self {
        Self {
            meta: EffectMeta {
                id: "dc_block",
                name: "DC Block",
                icon: "󰞒",
                description: "Removes DC offset",
                category: EffectCategory::Utility,
                params: vec![],
            },
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        is_enabled: bool,
        accent: Color,
        _muted: Color,
        _text: Color,
    ) {
        if area.width < 20 || area.height < 10 {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        self.render_wave(frame, inner_area, is_enabled);
    }

    fn render_wave(&self, frame: &mut Frame, area: Rect, is_enabled: bool) {
        let w = area.width as usize;
        let h = area.height as usize;

        if w < 6 || h < 3 {
            return;
        }

        let mut grid = vec![vec![(' ', Color::Reset); w]; h];

        let cy = h as f64 / 2.0;

        let center_row = cy as usize;
        for col in 0..w {
            if col % 3 == 0 {
                grid[center_row][col] = ('┄', COLOR_CENTER_LINE);
            }
        }

        if is_enabled {
            for col in 0..w {
                grid[center_row][col] = ('━', COLOR_WAVE_ENABLED);
            }
        } else {
            let amplitude = 0.25;
            let offset = -0.35;
            let frequency = 3.0;
            let color = COLOR_WAVE_DISABLED;

            for col in 0..w {
                let x_norm = col as f64 / w as f64;
                let sine_val = (x_norm * std::f64::consts::TAU * frequency).sin();

                let y_norm = (sine_val * amplitude) + offset;
                let row = (cy - (y_norm * h as f64)).round() as isize;

                if row >= 0 && row < h as isize {
                    grid[row as usize][col] = ('━', color);
                }
            }
        }

        let lines: Vec<Line> = grid
            .into_iter()
            .map(|row| {
                let spans: Vec<Span> = row
                    .into_iter()
                    .map(|(ch, color)| Span::styled(ch.to_string(), Style::default().fg(color)))
                    .collect();
                Line::from(spans)
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), area);
    }

    pub fn meta(&self) -> &EffectMeta {
        &self.meta
    }
}
