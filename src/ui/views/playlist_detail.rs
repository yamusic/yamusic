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
    audio::queue::PlaybackContext,
    event::events::Event,
    ui::util::get_active_track_icon,
    ui::{
        components::spinner::Spinner,
        context::AppContext,
        state::AppState,
        traits::{Action, View},
    },
    util::{colors, track::extract_track_ids},
};

const PAGE_SIZE: usize = 10;

pub struct PlaylistDetail {
    pub playlist: Option<Playlist>,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub all_track_ids: Vec<String>,
    pub loaded_count: usize,
    pub is_loading_more: bool,
}

impl PlaylistDetail {
    pub fn loading() -> Self {
        Self {
            playlist: None,
            tracks: Vec::new(),
            list_state: ListState::default(),
            all_track_ids: Vec::new(),
            loaded_count: 0,
            is_loading_more: false,
        }
    }

    pub fn new(playlist: Playlist) -> Self {
        Self {
            playlist: Some(playlist),
            tracks: Vec::new(),
            list_state: ListState::default(),
            all_track_ids: Vec::new(),
            loaded_count: 0,
            is_loading_more: false,
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
            all_track_ids: Vec::new(),
            loaded_count: 0,
            is_loading_more: false,
        }
    }

    fn has_more_tracks(&self) -> bool {
        self.loaded_count < self.all_track_ids.len()
    }

    fn should_load_more(&self) -> bool {
        if self.is_loading_more || !self.has_more_tracks() {
            return false;
        }

        if let Some(selected) = self.list_state.selected() {
            let len = self.tracks.len();
            len > 0 && selected >= len.saturating_sub(2)
        } else {
            false
        }
    }

    fn trigger_load_more(&mut self, ctx: &AppContext) {
        if self.is_loading_more || !self.has_more_tracks() {
            return;
        }

        let playlist_kind = match &self.playlist {
            Some(p) => p.kind,
            None => return,
        };

        let start = self.loaded_count;
        let end = (start + PAGE_SIZE).min(self.all_track_ids.len());
        let batch: Vec<String> = self.all_track_ids[start..end].to_vec();

        if batch.is_empty() {
            return;
        }

        self.is_loading_more = true;
        let api = ctx.api.clone();
        let tx = ctx.event_tx.clone();
        let batch_end = end;

        tokio::spawn(async move {
            match api.fetch_tracks_by_ids(batch).await {
                Ok(tracks) => {
                    let tracks: Vec<_> = tracks
                        .into_iter()
                        .filter(|t| t.available.unwrap_or(false))
                        .collect();
                    let _ = tx.send(Event::PlaylistTracksPageFetched(
                        playlist_kind,
                        tracks,
                        batch_end,
                    ));
                }
                Err(e) => {
                    tracing::info!("Failed to fetch tracks: {}", e);
                    let _ = tx.send(Event::FetchError(e.to_string()));
                }
            }
        });
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

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(area);

        let header_area = chunks[0];
        let tracks_area = chunks[1];

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

        let track_info = if self.has_more_tracks() {
            format!(
                "{}/{} tracks{}",
                self.tracks.len(),
                track_count,
                if self.is_loading_more {
                    " (loading...)"
                } else {
                    ""
                }
            )
        } else {
            format!("{} tracks", track_count)
        };

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                title,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(colors::PRIMARY),
            )),
            Line::from(format!("By {}", owner)),
            Line::from(format!(
                "{} • {} • {} likes",
                track_info, duration_str, likes
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

        f.render_widget(header, header_area);

        if self.tracks.is_empty() {
            let label = if self.all_track_ids.is_empty() {
                "Loading tracks...".to_string()
            } else {
                format!("Loading tracks (0/{})...", self.all_track_ids.len())
            };
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label(label);
            f.render_widget(spinner, tracks_area);
            return;
        }

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

        f.render_stateful_widget(list, tracks_area, &mut self.list_state);
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

                    if self.should_load_more() {
                        self.trigger_load_more(ctx);
                    }
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
            KeyCode::Char('g') => {
                if len > 0 {
                    self.list_state.select(Some(0));
                }
                None
            }
            KeyCode::Char('G') => {
                if len > 0 {
                    self.list_state.select(Some(len - 1));

                    if self.should_load_more() {
                        self.trigger_load_more(ctx);
                    }
                }
                None
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(playlist) = &self.playlist {
                        return Some(Action::PlayContext(
                            PlaybackContext::Playlist(playlist.clone()),
                            self.tracks.clone(),
                            i,
                        ));
                    }
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
                    let tracks: Vec<_> = session.sequence.iter().map(|s| s.track.clone()).collect();

                    return Some(Action::PlayContext(
                        PlaybackContext::Wave(session),
                        tracks,
                        0,
                    ));
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
                    let tracks: Vec<_> = session.sequence.iter().map(|s| s.track.clone()).collect();

                    return Some(Action::PlayContext(
                        PlaybackContext::Wave(session),
                        tracks,
                        0,
                    ));
                }
                None
            }
            _ => None,
        }
    }

    async fn on_event(&mut self, event: &Event, ctx: &AppContext) {
        match event {
            Event::PlaylistFetched(playlist) => {
                let should_accept = self.playlist.is_none()
                    || self.playlist.as_ref().map(|p| p.kind) == Some(playlist.kind);

                if should_accept {
                    self.all_track_ids = playlist
                        .tracks
                        .as_ref()
                        .map(extract_track_ids)
                        .unwrap_or_default();
                    self.playlist = Some(playlist.clone());
                    self.loaded_count = 0;
                    self.is_loading_more = false;
                }
            }
            Event::PlaylistTracksFetched(playlist_kind, tracks) => {
                if self.playlist.as_ref().map(|p| p.kind) == Some(*playlist_kind) {
                    self.tracks = tracks.clone();
                    self.loaded_count = PAGE_SIZE.min(self.all_track_ids.len());
                    self.is_loading_more = false;

                    if !self.tracks.is_empty() && self.list_state.selected().is_none() {
                        self.list_state.select(Some(0));
                    }
                }
            }
            Event::PlaylistTracksPageFetched(playlist_kind, tracks, loaded_count) => {
                if self.playlist.as_ref().map(|p| p.kind) == Some(*playlist_kind) {
                    self.tracks.extend(tracks.clone());
                    self.loaded_count = *loaded_count;
                    self.is_loading_more = false;

                    if self.list_state.selected().is_none() && !self.tracks.is_empty() {
                        self.list_state.select(Some(0));
                    }

                    if self.should_load_more() {
                        self.trigger_load_more(ctx);
                    }
                }
            }
            _ => {}
        }
    }
}
