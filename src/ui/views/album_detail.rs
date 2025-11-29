use async_trait::async_trait;
use crossterm::event::KeyModifiers;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use yandex_music::model::{album::Album, track::Track};

use crate::{
    event::events::Event,
    ui::util::get_active_track_icon,
    ui::{
        components::spinner::Spinner,
        context::AppContext,
        state::AppState,
        traits::{Action, View},
    },
    util::colors,
};

pub struct AlbumDetail {
    pub album: Album,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub is_loading: bool,
}

impl AlbumDetail {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            tracks: Vec::new(),
            list_state: ListState::default(),
            is_loading: true,
        }
    }
}

#[async_trait]
impl View for AlbumDetail {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, _ctx: &AppContext) {
        if self.is_loading && self.tracks.is_empty() {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label("Loading tracks...".to_string());
            f.render_widget(spinner, area);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(area);

        let title = self.album.title.clone().unwrap_or_default();
        let artists = self
            .album
            .artists
            .iter()
            .map(|a| a.name.clone().unwrap_or_default())
            .collect::<Vec<_>>()
            .join(", ");
        let year = self.album.year.map(|y| y.to_string()).unwrap_or_default();
        let genre = self.album.genre.clone().unwrap_or_default();
        let likes = self.album.likes_count.unwrap_or(0);
        let track_count = self.album.track_count.unwrap_or(0);

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(colors::PRIMARY),
            )),
            Line::from(format!("By {}", artists)),
            Line::from(format!("{} • {}", year, genre)),
            Line::from(format!("{} tracks • {} likes", track_count, likes)),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .padding(ratatui::widgets::Padding::new(1, 1, 0, 1)),
        );

        f.render_widget(header, chunks[0]);

        let current_track_id = _ctx.audio_system.current_track().map(|t| t.id);
        let is_playing = _ctx.audio_system.is_playing();

        let items: Vec<ListItem> = self
            .tracks
            .iter()
            .map(|track| {
                let is_current = current_track_id.as_ref() == Some(&track.id);
                let prefix = if is_current {
                    format!("{} ", get_active_track_icon(is_playing))
                } else {
                    "  ".to_string()
                };

                let title = track.title.as_deref().unwrap_or("Unknown Title");
                let artists = track
                    .artists
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("Unknown Artist"))
                    .collect::<Vec<&str>>()
                    .join(", ");

                let content = format!("{}{}- {}", prefix, title, artists);
                let mut item = ListItem::new(content);

                if is_current {
                    item = item.style(
                        Style::default()
                            .fg(colors::SECONDARY)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                item
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        f.render_stateful_widget(list, chunks[1], &mut self.list_state);
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        let len = self.tracks.len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let i = self
                        .list_state
                        .selected()
                        .map_or(0, |i| if i >= len - 1 { i } else { i + 1 });
                    self.list_state.select(Some(i));
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
                    let i = self
                        .list_state
                        .selected()
                        .map_or(0, |i| if i == 0 { 0 } else { i - 1 });
                    self.list_state.select(Some(i));
                }
                None
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    let tracks_to_play = if i > 0 {
                        self.tracks.iter().skip(i).cloned().collect()
                    } else {
                        self.tracks.clone()
                    };

                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::TracksFetched(tracks_to_play));
                }
                None
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                let album_id = self.album.id.clone();

                if album_id.is_none() {
                    return None;
                }

                let session = ctx
                    .api
                    .create_session(vec![format!("album:{}", album_id.unwrap())])
                    .await
                    .unwrap();
                let tracks = session.sequence.iter().map(|s| s.track.clone()).collect();

                let _ = ctx
                    .event_tx
                    .send(crate::event::events::Event::WaveReady(session, tracks));

                None
            }
            KeyCode::Char('w') => {
                if let Some(i) = self.list_state.selected() {
                    let track_id = self.tracks.get(i).as_ref().map(|track| track.id.clone());
                    if track_id.is_none() {
                        return None;
                    }

                    let session = ctx
                        .api
                        .create_session(vec![format!("track:{}", track_id.unwrap())])
                        .await
                        .unwrap();
                    let tracks = session.sequence.iter().map(|s| s.track.clone()).collect();

                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::WaveReady(session, tracks));
                }
                None
            }
            _ => None,
        }
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        if let Event::AlbumTracksFetched(tracks) = event {
            self.tracks = tracks.clone();
            self.is_loading = false;
            if !self.tracks.is_empty() {
                self.list_state.select(Some(0));
            }
        }
    }
}
