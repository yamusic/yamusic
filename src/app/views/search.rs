use std::sync::Arc;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    widgets::{Block, Borders, Paragraph, Tabs},
};
use yandex_music::model::{album::Album, artist::Artist, playlist::Playlist, track::Track};

use crate::{
    app::{
        actions::{Action, Route},
        components::{DynamicList, Spinner},
        data::{DataSource, StaticDataSource},
        keymap::Key,
        signals::AppSignals,
        state::SearchTab,
        views::{AlbumRenderer, ArtistRenderer, PlaylistRenderer, TrackRenderer},
    },
    framework::{signals::Signal, theme::ThemeStyles},
};

pub struct SearchView {
    query: Signal<String>,
    current_tab: Signal<SearchTab>,
    input_mode: Signal<bool>,

    track_source: Arc<StaticDataSource<Track>>,
    track_list: DynamicList<Track>,

    album_source: Arc<StaticDataSource<Album>>,
    album_list: DynamicList<Album>,

    artist_source: Arc<StaticDataSource<Artist>>,
    artist_list: DynamicList<Artist>,

    playlist_source: Arc<StaticDataSource<Playlist>>,
    playlist_list: DynamicList<Playlist>,

    is_loading: Signal<bool>,
    is_loading_more: Signal<bool>,

    has_searched: bool,

    theme: Signal<ThemeStyles>,
}

impl SearchView {
    pub fn new(signals: &AppSignals, theme: Signal<ThemeStyles>) -> Self {
        let track_source = Arc::new(StaticDataSource::new(Vec::new()));
        let track_renderer = Arc::new(TrackRenderer::new(
            signals.library.clone(),
            signals.audio.current_track_id.clone(),
            signals.audio.is_playing.clone(),
            theme.clone(),
        ));
        let track_list = DynamicList::new(track_source.clone(), track_renderer, theme.clone())
            .with_fuzzy(|track| {
                use crate::app::components::FuzzyFields;
                let title = track.title.clone().unwrap_or_default();
                let artists = track
                    .artists
                    .iter()
                    .filter_map(|a| a.name.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                let album = track.albums.first().and_then(|a| a.title.clone());
                let full = format!("{} {}", title, artists);
                FuzzyFields {
                    full,
                    title: Some(title),
                    artist: Some(artists),
                    album,
                }
            });

        let album_source = Arc::new(StaticDataSource::new(Vec::new()));
        let album_renderer = Arc::new(AlbumRenderer::new());
        let album_list = DynamicList::new(album_source.clone(), album_renderer, theme.clone())
            .with_fuzzy(|album| {
                use crate::app::components::FuzzyFields;
                let title = album.title.clone().unwrap_or_default();
                let artists = album
                    .artists
                    .iter()
                    .filter_map(|a| a.name.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                let full = format!("{} {}", title, artists);
                FuzzyFields {
                    full,
                    title: Some(title),
                    artist: Some(artists),
                    album: None,
                }
            });

        let artist_source = Arc::new(StaticDataSource::new(Vec::new()));
        let artist_renderer = Arc::new(ArtistRenderer::new());
        let artist_list = DynamicList::new(artist_source.clone(), artist_renderer, theme.clone())
            .with_fuzzy(|artist| {
                use crate::app::components::FuzzyFields;
                let name = artist.name.clone().unwrap_or_default();
                let genres = artist
                    .genres
                    .clone()
                    .map(|g| g.join(" "))
                    .unwrap_or_default();
                let full = format!("{} {}", name, genres);
                FuzzyFields {
                    full,
                    title: Some(name),
                    artist: None,
                    album: None,
                }
            });

        let playlist_source = Arc::new(StaticDataSource::new(Vec::new()));
        let playlist_renderer = Arc::new(PlaylistRenderer::new());
        let playlist_list =
            DynamicList::new(playlist_source.clone(), playlist_renderer, theme.clone()).with_fuzzy(
                |playlist| {
                    use crate::app::components::FuzzyFields;
                    let owner = playlist.owner.name.clone().unwrap_or_default();
                    let full = format!("{} {}", playlist.title, owner);
                    FuzzyFields {
                        full,
                        title: Some(playlist.title.clone()),
                        artist: Some(owner),
                        album: None,
                    }
                },
            );

        Self {
            query: Signal::new(String::new()),
            current_tab: Signal::new(SearchTab::Tracks),
            input_mode: Signal::new(true),
            track_source,
            track_list,
            album_source,
            album_list,
            artist_source,
            artist_list,
            playlist_source,
            playlist_list,
            is_loading: Signal::new(false),
            is_loading_more: Signal::new(false),
            has_searched: false,
            theme,
        }
    }

    pub fn current_tab(&self) -> SearchTab {
        self.current_tab.get()
    }

    pub fn current_selection(&self) -> usize {
        match self.current_tab.get() {
            SearchTab::Tracks => self.track_list.selected(),
            SearchTab::Albums => self.album_list.selected(),
            SearchTab::Artists => self.artist_list.selected(),
            SearchTab::Playlists => self.playlist_list.selected(),
        }
    }

    pub fn current_tab_count(&self) -> usize {
        match self.current_tab.get() {
            SearchTab::Tracks => self.track_source.total().unwrap_or(0),
            SearchTab::Albums => self.album_source.total().unwrap_or(0),
            SearchTab::Artists => self.artist_source.total().unwrap_or(0),
            SearchTab::Playlists => self.playlist_source.total().unwrap_or(0),
        }
    }

    pub fn is_input_mode(&self) -> bool {
        self.input_mode.get()
    }

    pub fn query(&self) -> String {
        self.query.get()
    }

    pub fn apply(
        &mut self,
        tracks: Vec<yandex_music::model::track::Track>,
        albums: Vec<yandex_music::model::album::Album>,
        artists: Vec<yandex_music::model::artist::Artist>,
        playlists: Vec<yandex_music::model::playlist::Playlist>,
        optimal_tab: Option<SearchTab>,
    ) {
        if let Some(tab) = optimal_tab {
            self.current_tab.set(tab);
        } else if tracks.is_empty() && !albums.is_empty() {
            self.current_tab.set(SearchTab::Albums);
        } else if tracks.is_empty() && albums.is_empty() && !artists.is_empty() {
            self.current_tab.set(SearchTab::Artists);
        } else if tracks.is_empty()
            && albums.is_empty()
            && artists.is_empty()
            && !playlists.is_empty()
        {
            self.current_tab.set(SearchTab::Playlists);
        }

        self.track_source.set_items(tracks);
        self.album_source.set_items(albums);
        self.artist_source.set_items(artists);
        self.playlist_source.set_items(playlists);

        self.track_list.select_first();
        self.album_list.select_first();
        self.artist_list.select_first();
        self.playlist_list.select_first();

        self.is_loading.set(false);
        self.is_loading_more.set(false);
        self.has_searched = true;
    }

    pub fn apply_merged(
        &mut self,
        tracks: Vec<yandex_music::model::track::Track>,
        albums: Vec<yandex_music::model::album::Album>,
        artists: Vec<yandex_music::model::artist::Artist>,
        playlists: Vec<yandex_music::model::playlist::Playlist>,
    ) {
        self.track_source.set_items(tracks);
        self.album_source.set_items(albums);
        self.artist_source.set_items(artists);
        self.playlist_source.set_items(playlists);
        self.is_loading_more.set(false);
    }

    pub fn set_loading(&self, loading: bool) {
        self.is_loading.set(loading);
    }

    pub fn set_loading_more(&self, loading: bool) {
        self.is_loading_more.set(loading);
    }

    pub fn scroll_top(&mut self) {
        match self.current_tab.get() {
            SearchTab::Tracks => self.track_list.select_first(),
            SearchTab::Albums => self.album_list.select_first(),
            SearchTab::Artists => self.artist_list.select_first(),
            SearchTab::Playlists => self.playlist_list.select_first(),
        }
    }

    pub fn scroll_bottom(&mut self) {
        match self.current_tab.get() {
            SearchTab::Tracks => self.track_list.select_last(),
            SearchTab::Albums => self.album_list.select_last(),
            SearchTab::Artists => self.artist_list.select_last(),
            SearchTab::Playlists => self.playlist_list.select_last(),
        }
    }

    pub fn handle_key(&mut self, key: &Key, prefix: Option<char>) -> Action {
        if self.input_mode.get() {
            return match key {
                Key::Esc => {
                    self.input_mode.set(false);
                    Action::Redraw
                }
                Key::Enter => {
                    self.input_mode.set(false);
                    let query = self.query.get();
                    if !query.is_empty() {
                        self.is_loading.set(true);
                        self.has_searched = false;
                        Action::Search(query)
                    } else {
                        self.has_searched = false;
                        self.track_source.set_items(Vec::new());
                        self.album_source.set_items(Vec::new());
                        self.artist_source.set_items(Vec::new());
                        self.playlist_source.set_items(Vec::new());

                        self.track_list.select_first();
                        self.album_list.select_first();
                        self.artist_list.select_first();
                        self.playlist_list.select_first();
                        Action::Redraw
                    }
                }
                Key::Backspace => {
                    self.query.update(|q| {
                        q.pop();
                    });
                    Action::Redraw
                }
                Key::Char(c) => {
                    self.query.update(|q| q.push(*c));
                    Action::Redraw
                }
                _ => Action::Redraw,
            };
        }

        if prefix.is_none() {
            match key {
                Key::Char('/') | Key::Char('i') => {
                    self.input_mode.set(true);
                    return Action::Redraw;
                }
                Key::Tab | Key::Right | Key::Char('l') => {
                    self.current_tab.update(|t| *t = t.next());
                    return Action::Redraw;
                }
                Key::BackTab | Key::Left | Key::Char('h') => {
                    self.current_tab.update(|t| *t = t.prev());
                    return Action::Redraw;
                }
                _ => {}
            }
        }

        let action = match self.current_tab.get() {
            SearchTab::Tracks => {
                let action = self.track_list.handle_key(key, prefix);
                if prefix.is_none()
                    && *key == Key::Enter
                    && let Some(track) = self.track_list.selected_item()
                {
                    return Action::PlayTrack(track);
                }
                action
            }
            SearchTab::Albums => {
                let action = self.album_list.handle_key(key, prefix);
                if prefix.is_none()
                    && *key == Key::Enter
                    && let Some(album) = self.album_list.selected_item()
                    && let Some(album_id) = album.id
                {
                    let id = album_id.to_string();
                    let title = album.title.clone().unwrap_or_default();
                    return Action::Navigate(Route::Album { id, title });
                }
                action
            }
            SearchTab::Artists => {
                let action = self.artist_list.handle_key(key, prefix);
                if prefix.is_none()
                    && *key == Key::Enter
                    && let Some(artist) = self.artist_list.selected_item()
                    && let Some(id) = artist.id.clone()
                {
                    let name = artist.name.clone().unwrap_or_default();
                    return Action::Navigate(Route::Artist { id, name });
                }
                action
            }
            SearchTab::Playlists => {
                let action = self.playlist_list.handle_key(key, prefix);
                if prefix.is_none()
                    && *key == Key::Enter
                    && let Some(playlist) = self.playlist_list.selected_item()
                {
                    let kind = playlist.kind;
                    let title = playlist.title.clone();
                    return Action::Navigate(Route::Playlist { kind, title });
                }
                action
            }
        };

        let near_end = {
            let len = self.current_tab_count();
            let sel = self.current_selection();
            len > 0 && sel >= len.saturating_sub(2)
        };
        if near_end {
            return action.and(Action::SearchNextPage);
        }

        if !action.is_none() {
            return action;
        }

        Action::None
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        self.render_input(frame, chunks[0]);

        self.render_tabs(frame, chunks[1]);

        self.render_results(frame, chunks[2]);
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let query = self.query.get();
        let in_input = self.input_mode.get();

        let styles = self.theme.get();
        let border_style = if in_input {
            styles.block_focused
        } else {
            styles.block
        };

        let cursor = if in_input { "│" } else { "" };
        let prompt = if query.is_empty() && !in_input {
            "Press '/' to search...".to_string()
        } else {
            format!("{}{}", query, cursor)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Search")
            .border_style(border_style);

        let paragraph = Paragraph::new(prompt).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let tab = self.current_tab.get();
        let titles: Vec<String> = SearchTab::all()
            .iter()
            .map(|t| {
                let icon = match t {
                    SearchTab::Tracks => "",
                    SearchTab::Albums => "󰀥",
                    SearchTab::Artists => "",
                    SearchTab::Playlists => "",
                };
                format!("{} {}", icon, t.title())
            })
            .collect();
        let styles = self.theme.get();

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(tab.index())
            .style(styles.text_muted)
            .highlight_style(styles.selected.add_modifier(Modifier::BOLD));

        frame.render_widget(tabs, area);
    }

    fn render_results(&mut self, frame: &mut Frame, area: Rect) {
        if self.is_loading.get() && !self.has_searched {
            let styles = self.theme.get();
            let spinner = Spinner::new()
                .with_label("Searching...")
                .with_style(styles.accent);
            spinner.view(frame, area);
            return;
        }

        let styles = self.theme.get();

        match self.current_tab.get() {
            SearchTab::Tracks => {
                if self.track_source.total().is_none_or(|t| t == 0) && self.has_searched {
                    let paragraph = Paragraph::new("No tracks found")
                        .style(styles.text_muted)
                        .block(Block::default().borders(Borders::NONE));
                    frame.render_widget(paragraph, area);
                } else {
                    self.track_list.view(frame, area);
                }
            }
            SearchTab::Albums => {
                if self.album_source.total().is_none_or(|t| t == 0) && self.has_searched {
                    let paragraph = Paragraph::new("No albums found")
                        .style(styles.text_muted)
                        .block(Block::default().borders(Borders::NONE));
                    frame.render_widget(paragraph, area);
                } else {
                    self.album_list.view(frame, area);
                }
            }
            SearchTab::Artists => {
                if self.artist_source.total().is_none_or(|t| t == 0) && self.has_searched {
                    let paragraph = Paragraph::new("No artists found")
                        .style(styles.text_muted)
                        .block(Block::default().borders(Borders::NONE));
                    frame.render_widget(paragraph, area);
                } else {
                    self.artist_list.view(frame, area);
                }
            }
            SearchTab::Playlists => {
                if self.playlist_source.total().is_none_or(|t| t == 0) && self.has_searched {
                    let paragraph = Paragraph::new("No playlists found")
                        .style(styles.text_muted)
                        .block(Block::default().borders(Borders::NONE));
                    frame.render_widget(paragraph, area);
                } else {
                    self.playlist_list.view(frame, area);
                }
            }
        }

        if self.is_loading_more.get() {}
    }
}
