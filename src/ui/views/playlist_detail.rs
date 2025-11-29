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
use yandex_music::model::{playlist::Playlist, track::Track};

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

pub struct PlaylistDetail {
    pub playlist: Option<Playlist>,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
}

impl PlaylistDetail {
    pub fn loading() -> Self {
        Self {
            playlist: None,
            tracks: Vec::new(),
            list_state: ListState::default(),
        }
    }

    pub fn new(playlist: Playlist) -> Self {
        Self {
            playlist: Some(playlist),
            tracks: Vec::new(),
            list_state: ListState::default(),
        }
    }

    pub fn with_tracks(playlist: Playlist, tracks: Vec<Track>) -> Self {
        let mut list_state = ListState::default();
        if !tracks.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            playlist: Some(playlist),
            tracks,
            list_state,
        }
    }
}

#[async_trait]
impl View for PlaylistDetail {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, ctx: &AppContext) {
        if self.playlist.is_none() {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label("Loading...".to_string());
            f.render_widget(spinner, area);
            return;
        }

        let playlist = self.playlist.as_ref().unwrap();

        if self.tracks.is_empty() {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label(format!("Loading {}...", playlist.title));
            f.render_widget(spinner, area);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(area);

        let title = playlist.title.clone();
        let owner = playlist.owner.name.clone().unwrap_or_default();
        let description = playlist.description.clone().unwrap_or_default();
        let likes = playlist.likes_count;
        let track_count = playlist.track_count;
        let duration_secs = playlist.duration.as_secs();
        let duration_str = format!(
            "{:02}:{:02}:{:02}",
            duration_secs / 3600,
            (duration_secs % 3600) / 60,
            duration_secs % 60
        );

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(colors::PRIMARY),
            )),
            Line::from(format!("By {}", owner)),
            Line::from(format!(
                "{} tracks • {} • {} likes",
                track_count, duration_str, likes
            )),
            Line::from(Span::styled(
                description,
                Style::default().fg(ratatui::style::Color::Gray),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .padding(ratatui::widgets::Padding::new(1, 1, 0, 1)),
        );

        f.render_widget(header, chunks[0]);

        let current_track_id = ctx.audio_system.current_track().map(|t| t.id);
        let is_playing = ctx.audio_system.is_playing();

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
                let mut spans = Vec::with_capacity(5);
                spans.push(Span::raw(prefix));
                spans.push(Span::raw(title));
                spans.push(Span::raw(" - "));

                if let Some(first_artist) = track.artists.first() {
                    spans.push(Span::raw(
                        first_artist.name.as_deref().unwrap_or("Unknown Artist"),
                    ));
                    if track.artists.len() > 1 {
                        spans.push(Span::raw(", ..."));
                    }
                } else {
                    spans.push(Span::raw("Unknown Artist"));
                }

                let mut item = ListItem::new(Line::from(spans));

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
                if let Some(playlist) = &self.playlist {
                    let playlist_author = playlist.owner.login.clone();
                    let playlist_kind = playlist.kind;
                    let session = ctx
                        .api
                        .create_session(vec![format!("playlist:{playlist_author}_{playlist_kind}")])
                        .await
                        .unwrap();
                    let tracks = session.sequence.iter().map(|s| s.track.clone()).collect();

                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::WaveReady(session, tracks));
                }
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
        if let Event::PlaylistFetched(playlist, tracks) = event {
            let should_accept = self.playlist.is_none()
                || self.playlist.as_ref().map(|p| p.kind) == Some(playlist.kind);

            if should_accept {
                self.playlist = Some(playlist.clone());
                self.tracks = tracks.clone();
                if !self.tracks.is_empty() && self.list_state.selected().is_none() {
                    self.list_state.select(Some(0));
                }
            }
        }
    }
}
