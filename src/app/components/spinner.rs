use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::Span,
    widgets::Paragraph,
};

use crate::framework::theme::global_theme;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

static GLOBAL_FRAME: AtomicUsize = AtomicUsize::new(0);

pub fn tick_global() {
    GLOBAL_FRAME.fetch_add(1, Ordering::Relaxed);
}

pub struct Spinner {
    label: Option<String>,
    style: Style,
}

impl Default for Spinner {
    fn default() -> Self {
        let styles = global_theme().styles().get();
        Self {
            label: None,
            style: styles.accent,
        }
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn tick(&mut self) {}

    pub fn current_char(&self) -> char {
        SPINNER_FRAMES[GLOBAL_FRAME.load(Ordering::Relaxed) % SPINNER_FRAMES.len()]
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let spinner_char = self.current_char();
        let text = if let Some(label) = &self.label {
            format!("{} {}", spinner_char, label)
        } else {
            spinner_char.to_string()
        };
        frame.render_widget(
            Paragraph::new(Span::styled(text, self.style)).alignment(Alignment::Center),
            area,
        );
    }
}

impl ratatui::widgets::Widget for Spinner {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let spinner_char = self.current_char();
        let text = if let Some(label) = &self.label {
            format!("{} {}", spinner_char, label)
        } else {
            spinner_char.to_string()
        };
        Paragraph::new(Span::styled(text, self.style))
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}
