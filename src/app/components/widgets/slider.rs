use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct Slider<'a> {
    label: &'a str,
    value: f32,
    min: f32,
    max: f32,
    suffix: &'a str,
    focused: bool,
    accent: Color,
    muted: Color,
    text: Color,
}

impl<'a> Slider<'a> {
    pub fn new(label: &'a str, value: f32) -> Self {
        Self {
            label,
            value,
            min: 0.0,
            max: 1.0,
            suffix: "",
            focused: false,
            accent: Color::Cyan,
            muted: Color::DarkGray,
            text: Color::White,
        }
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = suffix;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn colors(mut self, accent: Color, muted: Color, text: Color) -> Self {
        self.accent = accent;
        self.muted = muted;
        self.text = text;
        self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 || area.height == 0 {
            return;
        }

        let label_width = self.label.len() as u16 + 2;
        let ratio = ((self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0);

        let display_val = if self.suffix == "%" {
            format!("{:>3}%", (self.value * 100.0).round() as i32)
        } else if self.suffix == "dB" {
            format!("{:>+.1} dB", self.value)
        } else if self.suffix == "Hz" {
            if self.value >= 1000.0 {
                format!("{:.1}kHz", self.value / 1000.0)
            } else {
                format!("{:.0} Hz", self.value)
            }
        } else if self.suffix == "s" {
            format!("{:.1}s", self.value)
        } else if self.suffix == "m" {
            format!("{:.0}m", self.value)
        } else if self.suffix == "ms" {
            format!("{:.0}ms", self.value)
        } else if self.suffix == ":1" {
            format!("{:.1}:1", self.value)
        } else {
            format!("{:.2}", self.value)
        };

        let val_width = display_val.len() as u16 + 1;
        let track_width = area
            .width
            .saturating_sub(label_width + val_width + 2)
            .max(4) as usize;

        let handle_pos = ((track_width as f32 - 1.0) * ratio) as usize;

        let label_style = if self.focused {
            Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.text)
        };
        let val_style = if self.focused {
            Style::default().fg(self.accent)
        } else {
            Style::default().fg(self.text)
        };

        let mut track = String::with_capacity(track_width * 3);
        for i in 0..track_width {
            if i == handle_pos {
                if self.focused {
                    track.push('◉');
                } else {
                    track.push('●');
                }
            } else if i < handle_pos {
                track.push('━');
            } else {
                track.push('╌');
            }
        }

        let filled_style = if self.focused {
            Style::default().fg(self.accent)
        } else {
            Style::default().fg(self.text)
        };
        let unfilled_style = Style::default().fg(self.text);
        let handle_style = if self.focused {
            Style::default()
                .fg(self.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.text)
        };

        let mut track_spans = Vec::new();
        for (i, ch) in track.chars().enumerate() {
            if i == handle_pos {
                track_spans.push(Span::styled(ch.to_string(), handle_style));
            } else if i < handle_pos {
                track_spans.push(Span::styled(ch.to_string(), filled_style));
            } else {
                track_spans.push(Span::styled(ch.to_string(), unfilled_style));
            }
        }

        let mut spans = vec![
            Span::styled(
                format!(" {:<width$}", self.label, width = label_width as usize - 2),
                label_style,
            ),
            Span::raw("  "),
        ];
        spans.extend(track_spans);
        spans.push(Span::styled(format!(" {}", display_val), val_style));

        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), area);
    }
}
