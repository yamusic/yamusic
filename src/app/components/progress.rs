use ratatui::{layout::Rect, style::Style, widgets::Gauge, Frame};

use crate::{app::theme::theme, framework::signals::Signal};

pub struct ProgressBar {
    progress: Signal<f32>,
    label: Signal<String>,
}

impl ProgressBar {
    pub fn new(progress: Signal<f32>) -> Self {
        Self {
            progress,
            label: Signal::new(String::new()),
        }
    }

    pub fn with_label(mut self, label: Signal<String>) -> Self {
        self.label = label;
        self
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let progress = self.progress.get().clamp(0.0, 1.0);
        let label = self.label.get();
        let fg = theme().accent.primary;
        let bg = theme().text.dim;

        let gauge = Gauge::default()
            .ratio(progress as f64)
            .label(label)
            .gauge_style(Style::default().fg(fg).bg(bg));

        frame.render_widget(gauge, area);
    }
}

pub struct AudioProgressBar {
    current_ms: Signal<u64>,
    total_ms: Signal<u64>,
}

impl AudioProgressBar {
    pub fn new(current_ms: Signal<u64>, total_ms: Signal<u64>) -> Self {
        Self {
            current_ms,
            total_ms,
        }
    }

    fn format_time(ms: u64) -> String {
        let secs = ms / 1000;
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let current = self.current_ms.get();
        let total = self.total_ms.get();

        let ratio = if total > 0 {
            (current as f64 / total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let label = format!(
            "{} / {}",
            Self::format_time(current),
            Self::format_time(total)
        );

        let fg = theme().accent.primary;
        let bg = theme().text.dim;

        let gauge = Gauge::default()
            .ratio(ratio)
            .label(label)
            .gauge_style(Style::default().fg(fg).bg(bg));

        frame.render_widget(gauge, area);
    }
}
