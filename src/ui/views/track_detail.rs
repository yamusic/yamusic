use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use yandex_music::model::track::Track;

use crate::{
    event::events::Event,
    ui::{
        context::AppContext,
        state::AppState,
        traits::{Action, View},
    },
};

pub struct TrackDetail {
    pub track: Track,
}

impl TrackDetail {
    pub fn new(track: Track) -> Self {
        Self { track }
    }
}

#[async_trait]
impl View for TrackDetail {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, _ctx: &AppContext) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(area);

        let title = self.track.title.as_deref().unwrap_or("Unknown Title");
        let artists = self
            .track
            .artists
            .iter()
            .map(|a| a.name.as_deref().unwrap_or("Unknown Artist"))
            .collect::<Vec<&str>>()
            .join(", ");

        let albums = self
            .track
            .albums
            .iter()
            .map(|a| a.title.as_deref().unwrap_or("Unknown Album"))
            .collect::<Vec<&str>>()
            .join(", ");

        let title_block = Block::default().borders(Borders::ALL).title("Title");
        let artist_block = Block::default().borders(Borders::ALL).title("Artist");
        let album_block = Block::default().borders(Borders::ALL).title("Album");
        let info_block = Block::default().borders(Borders::ALL).title("Info");

        f.render_widget(
            Paragraph::new(Span::styled(
                title,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(crate::util::colors::PRIMARY),
            ))
            .block(title_block),
            chunks[0],
        );
        f.render_widget(Paragraph::new(artists).block(artist_block), chunks[1]);
        f.render_widget(Paragraph::new(albums).block(album_block), chunks[2]);

        let mut info_lines = Vec::new();
        if let Some(duration) = self.track.duration {
            let duration_secs = duration.as_secs();
            let duration_str = format!(
                "{:02}:{:02}:{:02}",
                duration_secs / 3600,
                (duration_secs % 3600) / 60,
                duration_secs % 60
            );
            info_lines.push(Line::from(format!("Duration: {}", duration_str)));
        }
        if let Some(file_size) = self.track.file_size {
            info_lines.push(Line::from(format!(
                "File Size: {:.2} MB",
                file_size as f64 / 1024.0 / 1024.0
            )));
        }
        if let Some(explicit) = self.track.explicit {
            if explicit {
                info_lines.push(Line::from(Span::styled(
                    "Explicit",
                    Style::default().fg(ratatui::style::Color::Red),
                )));
            }
        }
        if let Some(lyrics_available) = self.track.lyrics_available {
            if lyrics_available {
                info_lines.push(Line::from("Lyrics available"));
            }
        }
        if let Some(play_count) = self.track.play_count {
            info_lines.push(Line::from(format!("Play count: {}", play_count)));
        }

        f.render_widget(Paragraph::new(info_lines).block(info_block), chunks[3]);
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        match key.code {
            KeyCode::Enter => {
                let _ = ctx.event_tx.send(Event::TrackFetched(self.track.clone()));
                let _ = ctx.event_tx.send(Event::Play(0));
                Some(Action::Back)
            }
            _ => None,
        }
    }
}
