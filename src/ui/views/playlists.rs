use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};
use tokio::task::JoinHandle;
use yandex_music::model::playlist::Playlist;

use crate::{
    event::events::Event,
    ui::{
        components::spinner::Spinner,
        context::AppContext,
        state::AppState,
        traits::{Action, View},
    },
    util::colors,
};

pub struct Playlists {
    pub list_state: ListState,
    pub playlists: Vec<Playlist>,
    pub is_loading: bool,
    pub fetch_handle: Option<JoinHandle<()>>,
}

impl Default for Playlists {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
            playlists: Vec::new(),
            is_loading: true,
            fetch_handle: None,
        }
    }
}

impl Drop for Playlists {
    fn drop(&mut self) {
        if let Some(handle) = self.fetch_handle.take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl View for Playlists {
    async fn on_mount(&mut self, ctx: &AppContext) {
        self.is_loading = true;
        let api = ctx.api.clone();
        let tx = ctx.event_tx.clone();
        let handle = tokio::spawn(async move {
            match api.fetch_all_playlists().await {
                Ok(playlists) => {
                    let _ = tx.send(Event::PlaylistsFetched(playlists));
                }
                Err(e) => {
                    let _ = tx.send(Event::FetchError(e.to_string()));
                }
            }
        });
        self.fetch_handle = Some(handle);
    }

    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, _ctx: &AppContext) {
        if self.is_loading && self.playlists.is_empty() {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label("Loading playlists...".to_string());
            f.render_widget(spinner, area);
            return;
        }

        let items: Vec<ListItem> = self
            .playlists
            .iter()
            .map(|playlist| {
                let title = &playlist.title;
                let count = playlist.track_count;
                let content = format!("{} ({} tracks)", title, count);
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

        if !self.playlists.is_empty() && self.list_state.selected().is_none() {
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
        let len = self.playlists.len();
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
            KeyCode::Char('g') => {
                if len > 0 {
                    self.list_state.select(Some(0));
                }
                None
            }
            KeyCode::Char('G') => {
                if len > 0 {
                    self.list_state.select(Some(len - 1));
                }
                None
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    if let Some(playlist) = self.playlists.get(i) {
                        let _ = ctx
                            .event_tx
                            .send(crate::event::events::Event::PlaylistSelected(
                                playlist.clone(),
                            ));
                    }
                }
                None
            }
            _ => None,
        }
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        if let Event::PlaylistsFetched(playlists) = event {
            self.playlists = playlists.clone();
            self.is_loading = false;
        }
    }
}
