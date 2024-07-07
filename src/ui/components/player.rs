use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Widget,
};

use crate::audio::{enums::RepeatMode, progress::TrackProgress};

use super::{controls::PlayerControlsWidget, progress::ProgressWidget};

pub struct PlayerWidget<'a> {
    progress: &'a TrackProgress,
    track_title: &'a str,
    track_artist: Option<String>,
    repeat_mode: RepeatMode,
    shuffle_mode: bool,
    volume: u8,
    is_playing: bool,
}

impl<'a> PlayerWidget<'a> {
    pub fn new(
        progress: &'a TrackProgress,
        track_title: &'a str,
        track_artist: Option<String>,
        repeat_mode: RepeatMode,
        shuffle_mode: bool,
        volume: u8,
        is_playing: bool,
    ) -> Self {
        Self {
            progress,
            track_title,
            track_artist,
            repeat_mode,
            shuffle_mode,
            volume,
            is_playing,
        }
    }
}

impl<'a> Widget for PlayerWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(19)])
            .split(area);

        let progress_widget = ProgressWidget::new(
            self.progress,
            self.track_title,
            self.track_artist,
            self.is_playing,
        );
        let controls_widget = PlayerControlsWidget::new(
            self.repeat_mode,
            self.shuffle_mode,
            self.volume,
        );

        progress_widget.render(layout[0], buf);
        controls_widget.render(layout[1], buf);
    }
}
