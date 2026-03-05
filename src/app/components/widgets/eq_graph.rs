use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

#[derive(Clone)]
pub struct EqBand {
    pub freq: f32,
    pub gain_db: f32,
    pub label: String,
}

pub struct EqGraph {
    bands: Vec<EqBand>,
    focused_band: Option<usize>,
    accent: Color,
    muted: Color,
    text: Color,
}

impl EqGraph {
    pub fn new(bands: Vec<EqBand>) -> Self {
        Self {
            bands,
            focused_band: None,
            accent: Color::Cyan,
            muted: Color::DarkGray,
            text: Color::White,
        }
    }

    pub fn focused_band(mut self, band: Option<usize>) -> Self {
        self.focused_band = band;
        self
    }

    pub fn colors(mut self, accent: Color, muted: Color, text: Color) -> Self {
        self.accent = accent;
        self.muted = muted;
        self.text = text;
        self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 20 || area.height < 5 {
            return;
        }

        let _inner_x = area.x + 5;
        let inner_width = area.width.saturating_sub(6) as usize;
        let mut graph_height = area.height.saturating_sub(2) as usize;
        if graph_height < 3 {
            return;
        }
        if graph_height.is_multiple_of(2) {
            graph_height = graph_height.saturating_sub(1);
        }

        if inner_width < 10 || graph_height < 3 {
            return;
        }

        const MAX_DB: f32 = 12.0;
        const MIN_DB: f32 = -12.0;
        const DB_RANGE: f32 = MAX_DB - MIN_DB;
        const DB_TICKS: [f32; 7] = [12.0, 8.0, 4.0, 0.0, -4.0, -8.0, -12.0];
        let zero_row = ((MAX_DB / DB_RANGE) * (graph_height - 1) as f32).round() as usize;

        let num_bands = self.bands.len().max(1);
        let col_spacing = inner_width / num_bands;

        let mut grid: Vec<Vec<(char, Style)>> =
            vec![vec![(' ', Style::default()); inner_width]; graph_height];

        let zero_style = Style::default().fg(self.muted);
        if zero_row < graph_height {
            for col in 0..inner_width {
                grid[zero_row][col] = ('╌', zero_style);
            }
        }

        let mut band_positions: Vec<(usize, usize)> = Vec::new();

        for (i, band) in self.bands.iter().enumerate() {
            let col = (col_spacing / 2) + i * col_spacing;
            let col = col.min(inner_width - 1);

            let gain_ratio = ((MAX_DB - band.gain_db) / DB_RANGE).clamp(0.0, 1.0);
            let row = (gain_ratio * (graph_height - 1) as f32).round() as usize;
            let row = row.min(graph_height - 1);

            band_positions.push((col, row));

            let is_focused = self.focused_band == Some(i);
            let handle_style = if is_focused {
                Style::default().fg(self.accent).bg(Color::Reset)
            } else {
                Style::default().fg(self.text)
            };

            let handle_char = if is_focused { '◉' } else { '●' };
            grid[row][col] = (handle_char, handle_style);

            let stem_style = Style::default().fg(if is_focused { self.accent } else { self.muted });
            if row < zero_row {
                for r in (row + 1)..zero_row {
                    if r < graph_height {
                        grid[r][col] = ('│', stem_style);
                    }
                }
            } else if row > zero_row {
                for r in (zero_row + 1)..row {
                    if r < graph_height {
                        grid[r][col] = ('│', stem_style);
                    }
                }
            }
        }

        let curve_style = Style::default().fg(self.text);
        for w in band_positions.windows(2) {
            let (c1, r1) = w[0];
            let (c2, r2) = w[1];
            if c2 <= c1 + 1 {
                continue;
            }
            let span = (c2 - c1) as f32;
            for col in (c1 + 1)..c2 {
                let t = (col - c1) as f32 / span;
                let smooth_t = t * t * (3.0 - 2.0 * t);
                let row = (r1 as f32 + (r2 as f32 - r1 as f32) * smooth_t) as usize;
                let row = row.min(graph_height - 1);
                if grid[row][col].0 == ' ' || grid[row][col].0 == '╌' {
                    grid[row][col] = ('─', curve_style);
                }
            }
        }

        let tick_rows: Vec<(usize, f32)> = DB_TICKS
            .iter()
            .map(|&tick| {
                let r = ((MAX_DB - tick) / DB_RANGE * (graph_height - 1) as f32).round() as usize;
                (r.min(graph_height - 1), tick)
            })
            .collect();

        for row in 0..graph_height {
            let mut spans: Vec<Span> = Vec::new();

            let db_label = tick_rows
                .iter()
                .find(|(r, _)| *r == row)
                .map(|(_, tick)| {
                    if *tick == 0.0 {
                        "  0".to_string()
                    } else {
                        format!("{:+3.0}", tick)
                    }
                })
                .unwrap_or_else(|| "   ".to_string());

            spans.push(Span::styled(
                format!("{:>4} ", db_label),
                Style::default().fg(self.muted),
            ));

            for col in 0..inner_width {
                let (ch, style) = grid[row][col];
                spans.push(Span::styled(ch.to_string(), style));
            }

            let y = area.y + row as u16;
            if y < area.y + area.height {
                frame.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect::new(area.x, y, area.width, 1),
                );
            }
        }

        let freq_y = area.y + graph_height as u16;
        if freq_y < area.y + area.height {
            let mut freq_spans: Vec<Span> = vec![Span::raw("     ")];
            let mut last_end = 0usize;

            for (i, band) in self.bands.iter().enumerate() {
                let col = (col_spacing / 2) + i * col_spacing;
                let label = &band.label;
                let label_start = col.saturating_sub(label.len() / 2);

                if label_start > last_end {
                    let gap = label_start - last_end;
                    freq_spans.push(Span::raw(" ".repeat(gap)));
                }

                let is_focused = self.focused_band == Some(i);
                let style = if is_focused {
                    Style::default().fg(self.accent)
                } else {
                    Style::default().fg(self.text)
                };
                freq_spans.push(Span::styled(label.clone(), style));
                last_end = label_start + label.len();
            }

            frame.render_widget(
                Paragraph::new(Line::from(freq_spans)),
                Rect::new(area.x, freq_y, area.width, 1),
            );
        }
    }
}
