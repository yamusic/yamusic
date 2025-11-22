use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use tokio::task::JoinHandle;
use tracing::info;
use yandex_music::model::{playlist::PlaylistTracks, track::Track};

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

pub struct LikedTracks {
    list_state: ListState,
    tracks: Vec<Track>,
    loading: bool,
    fetch_handle: Option<JoinHandle<()>>,
}

impl Default for LikedTracks {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
            tracks: vec![],
            loading: false,
            fetch_handle: None,
        }
    }
}

impl Drop for LikedTracks {
    fn drop(&mut self) {
        if let Some(handle) = self.fetch_handle.take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl View for LikedTracks {
    async fn on_mount(&mut self, ctx: &AppContext) {
        self.loading = true;
        let api = ctx.api.clone();
        let tx = ctx.event_tx.clone();

        let handle = tokio::spawn(async move {
            match api.fetch_liked_tracks().await {
                Ok(playlist) => {
                    let tracks = match playlist.tracks {
                        Some(PlaylistTracks::Full(tracks)) => tracks,
                        Some(PlaylistTracks::WithInfo(tracks)) => {
                            tracks.into_iter().map(|t| t.track).collect()
                        }
                        Some(PlaylistTracks::Partial(partial_tracks)) => {
                            match api.fetch_tracks_partial(&partial_tracks).await {
                                Ok(tracks) => tracks,
                                Err(e) => {
                                    info!("Failed to fetch partial tracks: {}", e);
                                    vec![]
                                }
                            }
                        }
                        None => vec![],
                    };

                    let tracks: Vec<Track> = tracks
                        .into_iter()
                        .filter(|t| t.available.unwrap_or(false))
                        .collect();

                    let _ = tx.send(Event::LikedTracksFetched(tracks));
                }
                Err(e) => {
                    let _ = tx.send(Event::FetchError(e.to_string()));
                }
            }
        });
        self.fetch_handle = Some(handle);
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        match event {
            Event::LikedTracksFetched(tracks) => {
                self.tracks = tracks.clone();
                self.loading = false;
            }
            Event::FetchError(_) => {
                self.loading = false;
            }
            _ => {}
        }
    }

    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, _ctx: &AppContext) {
        if self.loading {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label("Loading liked tracks...".to_string());
            f.render_widget(spinner, area);
            return;
        }

        let tracks = &self.tracks;
        if tracks.is_empty() {
            let no_tracks = List::new(vec![ListItem::new("No liked tracks")]).highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_widget(no_tracks, area);
            return;
        }

        let current_track_id = _ctx.audio_system.current_track().map(|t| t.id);
        let is_playing = _ctx.audio_system.is_playing();

        let items: Vec<ListItem> = tracks
            .iter()
            .map(|track| {
                let is_current = current_track_id.as_ref() == Some(&track.id);
                let prefix = if is_current {
                    format!("{} ", get_active_track_icon(is_playing))
                } else {
                    "  ".to_string()
                };

                let title = track.title.as_deref().unwrap_or("Unknown Title");
                let mut spans = Vec::with_capacity(3);
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

        if !tracks.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        let tracks = &self.tracks;
        let len = tracks.len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let i = self.list_state.selected().unwrap_or(0);
                    if i < len - 1 {
                        self.list_state.select(Some(i + 1));
                    }
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
                    let i = self.list_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.list_state.select(Some(i - 1));
                    }
                }
                None
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    let tracks_to_play = if i > 0 {
                        tracks.iter().skip(i).cloned().collect()
                    } else {
                        tracks.clone()
                    };

                    if !tracks_to_play.is_empty() {
                        let _ = ctx
                            .event_tx
                            .send(crate::event::events::Event::TracksFetched(tracks_to_play));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
