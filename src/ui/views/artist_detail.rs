use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use yandex_music::model::{artist::Artist, track::Track};

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

pub struct ArtistDetail {
    pub artist: Artist,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
    pub is_loading: bool,
}

impl ArtistDetail {
    pub fn new(artist: Artist) -> Self {
        Self {
            artist,
            tracks: Vec::new(),
            list_state: ListState::default(),
            is_loading: true,
        }
    }
}

#[async_trait]
impl View for ArtistDetail {
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

        let name = self.artist.name.clone().unwrap_or_default();
        let genres = self.artist.genres.clone().unwrap_or_default().join(", ");
        let likes = self.artist.likes_count.unwrap_or(0);
        let description = self
            .artist
            .description
            .as_ref()
            .map(|d| d.text.clone())
            .unwrap_or_default();

        let description = if description.len() > 100 {
            format!("{}...", &description[..100])
        } else {
            description
        };

        let header = Paragraph::new(vec![
            Line::from(Span::styled(
                name,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(colors::PRIMARY),
            )),
            Line::from(genres),
            Line::from(format!("{} likes", likes)),
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
            _ => None,
        }
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        if let Event::ArtistTracksFetched(tracks) = event {
            self.tracks = tracks.clone();
            self.is_loading = false;
            if !self.tracks.is_empty() {
                self.list_state.select(Some(0));
            }
        }
    }
}
