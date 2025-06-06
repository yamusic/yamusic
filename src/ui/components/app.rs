use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::{self, border},
    widgets::{Block, Borders, Widget},
};

use crate::ui::{app::App, components::player::PlayerWidget};

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        buf.set_style(area, Style::new().bg(Color::from_u32(0x000d0d0d)));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);

        Block::new()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .border_set(border::Set {
                bottom_left: symbols::line::ROUNDED.vertical_right,
                bottom_right: symbols::line::ROUNDED.vertical_left,
                ..symbols::border::ROUNDED
            })
            .title_top("Yandex Music")
            .title_alignment(Alignment::Center)
            .render(chunks[0], buf);

        let track_title: &str;
        let track_artist: Option<String>;

        if let Some(track) = self.audio_system.current_track() {
            track_title = track.title.as_deref().unwrap_or("Unknown");
            track_artist = Some(
                track
                    .artists
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("Unknown"))
                    .collect::<Vec<&str>>()
                    .join(", "),
            );
        } else {
            track_title = "No track";
            track_artist = None;
        }

        let player_widget = PlayerWidget::new(
            self.audio_system.track_progress(),
            track_title,
            track_artist,
            self.audio_system.repeat_mode(),
            self.audio_system.is_shuffled(),
            if self.audio_system.is_muted() {
                0
            } else {
                self.audio_system.volume()
            },
            self.audio_system.is_playing(),
        );
        player_widget.render(chunks[1], buf);
    }
}
