use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
};
use yandex_music::model::search::Search as SearchModel;

use crate::event::events::Event;
use crate::{
    ui::util::get_active_track_icon,
    ui::{
        components::spinner::Spinner,
        context::AppContext,
        state::AppState,
        traits::{Action, View},
    },
    util::colors,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchTab {
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl SearchTab {
    fn as_str(&self) -> &str {
        match self {
            SearchTab::Tracks => "Tracks",
            SearchTab::Albums => "Albums",
            SearchTab::Artists => "Artists",
            SearchTab::Playlists => "Playlists",
        }
    }

    fn next(&self) -> Self {
        match self {
            SearchTab::Tracks => SearchTab::Albums,
            SearchTab::Albums => SearchTab::Artists,
            SearchTab::Artists => SearchTab::Playlists,
            SearchTab::Playlists => SearchTab::Tracks,
        }
    }

    fn prev(&self) -> Self {
        match self {
            SearchTab::Tracks => SearchTab::Playlists,
            SearchTab::Albums => SearchTab::Tracks,
            SearchTab::Artists => SearchTab::Albums,
            SearchTab::Playlists => SearchTab::Artists,
        }
    }
}

pub struct Search {
    input: String,
    is_editing: bool,
    list_state: ListState,
    active_tab: SearchTab,
    last_request_id: Option<String>,
    search_results: Option<SearchModel>,
    is_loading: bool,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            input: String::new(),
            is_editing: true,
            list_state: ListState::default(),
            active_tab: SearchTab::Tracks,
            last_request_id: None,
            search_results: None,
            is_loading: false,
        }
    }
}

#[async_trait]
impl View for Search {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, _ctx: &AppContext) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(area);

        let input_area = chunks[0];
        let tabs_area = chunks[1];
        let results_area = chunks[2];

        let input_style = if self.is_editing {
            Style::default().fg(colors::PRIMARY)
        } else {
            Style::default().fg(colors::NEUTRAL)
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .title("Search")
            .border_style(input_style);

        let input_text = Paragraph::new(self.input.clone()).block(input_block);
        f.render_widget(input_text, input_area);

        if let Some(results) = &self.search_results {
            if self.last_request_id.as_deref() != Some(&results.search_request_id) {
                self.last_request_id = Some(results.search_request_id.clone());
                if let Some(best) = &results.best {
                    self.active_tab = match best.item_type.as_str() {
                        "track" => SearchTab::Tracks,
                        "album" => SearchTab::Albums,
                        "artist" => SearchTab::Artists,
                        "playlist" => SearchTab::Playlists,
                        _ => SearchTab::Tracks,
                    };
                }
            }
        }

        let tabs = vec![
            SearchTab::Tracks,
            SearchTab::Albums,
            SearchTab::Artists,
            SearchTab::Playlists,
        ];
        let titles = tabs.iter().map(|t| t.as_str()).collect::<Vec<_>>();
        let tabs_widget = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0))
            .highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(tabs_widget, tabs_area);

        if self.is_loading && self.search_results.is_none() {
            let spinner = Spinner::default()
                .with_style(Style::default().fg(colors::PRIMARY))
                .with_label("Searching...".to_string());
            f.render_widget(spinner, results_area);
            return;
        }

        if let Some(results) = &self.search_results {
            let mut items = Vec::new();
            let current_track_id = _ctx.audio_system.current_track().map(|t| t.id);
            let is_playing = _ctx.audio_system.is_playing();

            match self.active_tab {
                SearchTab::Tracks => {
                    if let Some(tracks) = &results.tracks {
                        for item in &tracks.results {
                            let is_current = current_track_id.as_ref() == Some(&item.id);
                            let prefix = if is_current {
                                format!("{} ", get_active_track_icon(is_playing))
                            } else {
                                "  ".to_string()
                            };

                            let title = item.title.as_deref().unwrap_or("Unknown Title");
                            let artist = item
                                .artists
                                .first()
                                .and_then(|a| a.name.as_deref())
                                .unwrap_or("Unknown Artist");
                            let content = format!("{}{}- {}", prefix, title, artist);
                            let mut list_item = ListItem::new(format!("  {}", content));
                            if is_current {
                                list_item = list_item.style(
                                    Style::default()
                                        .fg(colors::SECONDARY)
                                        .add_modifier(Modifier::BOLD),
                                );
                            }
                            items.push(list_item);
                        }
                    }
                }
                SearchTab::Albums => {
                    if let Some(albums) = &results.albums {
                        for item in &albums.results {
                            let title = item.title.as_deref().unwrap_or("Unknown Title");
                            let artist = item
                                .artists
                                .first()
                                .and_then(|a| a.name.as_deref())
                                .unwrap_or("Unknown Artist");
                            let content = format!("{} - {}", title, artist);
                            items.push(ListItem::new(format!("  {}", content)));
                        }
                    }
                }
                SearchTab::Artists => {
                    if let Some(artists) = &results.artists {
                        for item in &artists.results {
                            let name = item.name.as_deref().unwrap_or("Unknown Artist");
                            items.push(ListItem::new(format!("  {}", name)));
                        }
                    }
                }
                SearchTab::Playlists => {
                    if let Some(playlists) = &results.playlists {
                        for item in &playlists.results {
                            let title = item.title.clone();
                            items.push(ListItem::new(format!("  {}", title)));
                        }
                    }
                }
            }

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            if self.list_state.selected().is_none() {
                self.list_state.select(Some(0));
            }

            f.render_stateful_widget(list, results_area, &mut self.list_state);
        }
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        if self.is_editing {
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => None,
                KeyCode::Enter => {
                    if !self.input.is_empty() {
                        let _ = ctx
                            .event_tx
                            .send(crate::event::events::Event::Search(self.input.clone()));
                        self.is_editing = false;
                        self.is_loading = true;
                    }
                    Some(Action::None)
                }
                KeyCode::Char(c) => {
                    self.input.push(c);
                    Some(Action::None)
                }
                KeyCode::Backspace => {
                    self.input.pop();
                    Some(Action::None)
                }
                KeyCode::Esc => {
                    self.is_editing = false;
                    Some(Action::None)
                }
                _ => Some(Action::None),
            }
        } else {
            match key.code {
                KeyCode::Char('/') => {
                    self.is_editing = true;
                    Some(Action::None)
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    self.active_tab = self.active_tab.prev();
                    self.list_state.select(Some(0));
                    Some(Action::None)
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.active_tab = self.active_tab.next();
                    self.list_state.select(Some(0));
                    Some(Action::None)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some(i + 1));
                    Some(Action::None)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = self.list_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.list_state.select(Some(i - 1));
                    }
                    Some(Action::None)
                }
                KeyCode::Enter => {
                    if let Some(results) = &self.search_results {
                        match self.active_tab {
                            SearchTab::Tracks => {
                                if let Some(tracks) = &results.tracks {
                                    if let Some(i) = self.list_state.selected() {
                                        if let Some(track) = tracks.results.get(i) {
                                            let _ = ctx
                                                .event_tx
                                                .send(Event::TrackSelected(track.clone()));
                                        }
                                    }
                                }
                            }
                            SearchTab::Albums => {
                                if let Some(albums) = &results.albums {
                                    if let Some(i) = self.list_state.selected() {
                                        if let Some(album) = albums.results.get(i) {
                                            let _ = ctx
                                                .event_tx
                                                .send(Event::AlbumSelected(album.clone()));
                                        }
                                    }
                                }
                            }
                            SearchTab::Artists => {
                                if let Some(artists) = &results.artists {
                                    if let Some(i) = self.list_state.selected() {
                                        if let Some(artist) = artists.results.get(i) {
                                            let _ = ctx
                                                .event_tx
                                                .send(Event::ArtistSelected(artist.clone()));
                                        }
                                    }
                                }
                            }
                            SearchTab::Playlists => {
                                if let Some(playlists) = &results.playlists {
                                    if let Some(i) = self.list_state.selected() {
                                        if let Some(playlist) = playlists.results.get(i) {
                                            let _ = ctx
                                                .event_tx
                                                .send(Event::PlaylistSelected(playlist.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Action::None)
                }
                _ => None,
            }
        }
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        if let Event::SearchResults(results) = event {
            self.search_results = Some(results.clone());
            self.is_loading = false;
        }
    }
}
