use std::time::Duration;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    symbols::{self, border},
    widgets::{block::Title, Block, Borders, Gauge, Widget},
};

use crate::audio::progress::TrackProgress;

pub struct ProgressWidget<'a> {
    progress: &'a TrackProgress,
    track_title: &'a str,
    track_artist: Option<String>,
    is_playing: bool,
}

impl<'a> ProgressWidget<'a> {
    pub fn new(
        progress: &'a TrackProgress,
        track_title: &'a str,
        track_artist: Option<String>,
        is_playing: bool,
    ) -> Self {
        Self {
            progress,
            track_title,
            track_artist,
            is_playing,
        }
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

        let mut track_info = format!(
            "{}  {}",
            if self.is_playing { "" } else { "" },
            self.track_title
        );
        if let Some(artist) = self.track_artist {
            track_info = format!("{} by {}", track_info, artist);
        }

        let duration_info = format!(
            "{} / {}",
            format_duration(current),
            format_duration(total)
        );

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Title::from(track_info).alignment(Alignment::Center))
                    .borders(Borders::ALL)
                    .border_set(border::Set {
                        top_left: symbols::line::ROUNDED.vertical_right,
                        top_right: symbols::line::ROUNDED.horizontal_down,
                        bottom_right: symbols::line::ROUNDED.horizontal_up,
                        ..symbols::border::ROUNDED
                    }),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::from_u32(0x00f7d44b))
                    .bg(Color::from_u32(0x00464646)),
            )
            .ratio(percent.min(1.0))
            .label(duration_info);

        gauge.render(area, buf);
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
