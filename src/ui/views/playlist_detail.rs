use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};
use yandex_music::model::{playlist::Playlist, track::Track};

use crate::{
    ui::{
        context::{AppContext, GlobalUiState},
        traits::{Action, Component},
    },
    util::colors,
};

pub struct PlaylistDetail {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
    pub list_state: ListState,
}

impl PlaylistDetail {
    pub fn new(playlist: Playlist) -> Self {
        Self {
            playlist,
            tracks: Vec::new(),
            list_state: ListState::default(),
        }
    }
}

impl Component for PlaylistDetail {
    fn render(&mut self, f: &mut Frame, area: Rect, _ctx: &AppContext, state: &GlobalUiState) {
        if state.is_loading && self.tracks.is_empty() {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let symbol = spinner[state.spinner_index % spinner.len()];
            let text = format!("{} Loading tracks...", symbol);
            let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
            let y = area.y + area.height / 2;

            f.buffer_mut()
                .set_string(x, y, text, Style::default().fg(colors::PRIMARY));
            return;
        }

        let items: Vec<ListItem> = self
            .tracks
            .iter()
            .map(|track| {
                let title = track.title.as_deref().unwrap_or("Unknown Title");
                let artists = track
                    .artists
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("Unknown Artist"))
                    .collect::<Vec<&str>>()
                    .join(", ");

                let content = format!("{} - {}", title, artists);
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        ctx: &AppContext,
        _state: &GlobalUiState,
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
                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::TracksFetched(
                            self.tracks.clone(),
                        ));
                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::Play(i as i32));
                }
                None
            }
            _ => None,
        }
    }

    fn on_event(&mut self, event: &crate::event::events::Event) {
        if let crate::event::events::Event::PlaylistTracksFetched(tracks) = event {
            self.tracks = tracks.clone();
            if !self.tracks.is_empty() {
                self.list_state.select(Some(0));
            }
        }
    }
}
