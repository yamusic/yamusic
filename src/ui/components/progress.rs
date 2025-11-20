use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style, Stylize},
    symbols::{self, border},
    text::ToSpan,
    widgets::{Block, Borders, Widget},
};

use crate::{audio::progress::TrackProgress, ui::components::gauge::CustomGauge, util::colors};

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
        let percent = if total > 0 {
            current as f64 / total as f64
        } else {
            0.0
        };

        let mut track_info = format!(
            "{}  {}",
            if self.is_playing { "" } else { "" },
            self.track_title
        );
        if let Some(artist) = self.track_artist {
            track_info = format!("{track_info} by {artist}");
        }

        let duration_info = format!("{} / {}", format_duration(current), format_duration(total));

        let buffered_ratio = self.progress.get_buffered_ratio();

        let gauge = CustomGauge::default()
            .block(
                Block::default()
                    .title_top(track_info)
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_set(border::Set {
                        top_right: symbols::line::ROUNDED.horizontal_down,
                        bottom_right: symbols::line::ROUNDED.horizontal_up,
                        ..symbols::border::ROUNDED
                    }),
            )
            .ratios(percent.min(1.0), buffered_ratio.min(1.0))
            .label(duration_info.to_span().fg(Color::White))
            .played_style(Style::default().fg(colors::PRIMARY).bg(colors::SECONDARY))
            .buffered_style(
                Style::default()
                    .fg(colors::SECONDARY)
                    .bg(colors::BACKGROUND),
            )
            .remaining_style(
                Style::default()
                    .fg(colors::BACKGROUND)
                    .bg(colors::BACKGROUND),
            )
            .use_unicode(true);

        gauge.render(area, buf);
    }
}

fn format_duration(duration: u64) -> String {
    let total_seconds = duration / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}
