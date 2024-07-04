use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Widget},
};

use crate::audio::progress::TrackProgress;

pub struct ProgressWidget<'a> {
    progress: &'a TrackProgress,
}

impl<'a> ProgressWidget<'a> {
    pub fn new(progress: &'a TrackProgress) -> Self {
        Self { progress }
    }
}

impl<'a> Widget for ProgressWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (current, total) = self.progress.get_progress();
        let percent = if total.as_secs() > 0 {
            current.as_secs_f64() / total.as_secs_f64()
        } else {
            0.0
        };

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL))
            .gauge_style(
                Style::default()
                    .fg(Color::from_u32(0x00f7d44b))
                    .bg(Color::from_u32(0x00464646)),
            )
            .ratio(percent.min(1.0))
            .label(format!(
                "{} / {}",
                format_duration(current),
                format_duration(total)
            ));

        gauge.render(area, buf);
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
