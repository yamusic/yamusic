use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Spinner {
    style: Style,
    label: Option<String>,
}

impl Spinner {
    pub fn default() -> Self {
        Self {
            style: Style::default(),
            label: None,
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }
}

impl Widget for Spinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let index = (now / 100) as usize % spinner_chars.len();
        let symbol = spinner_chars[index];

        let text = if let Some(label) = self.label {
            format!("{} {}", symbol, label)
        } else {
            symbol.to_string()
        };

        let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
        let y = area.y + area.height / 2;

        if area.width > 0 && area.height > 0 {
            buf.set_string(x, y, text, self.style);
        }
    }
}
