use std::sync::Arc;

use ratatui::{Frame, layout::Rect};
use yandex_music::model::playlist::Playlist;

use crate::{
    app::{
        actions::{Action, Route},
        components::{DynamicList, Spinner},
        data::{DataSource, PlaylistDataSource},
        keymap::Key,
        views::PlaylistRenderer,
    },
    framework::{signals::Signal, theme::ThemeStyles},
};

pub struct PlaylistListView {
    pub source: Arc<PlaylistDataSource>,
    list: DynamicList<Playlist>,

    theme: Signal<ThemeStyles>,
}

impl PlaylistListView {
    pub fn new(source: Arc<PlaylistDataSource>, theme: Signal<ThemeStyles>) -> Self {
        let renderer = Arc::new(PlaylistRenderer::new());
        let list = DynamicList::new(source.clone(), renderer, theme.clone())
            .with_title("My Playlists")
            .with_fuzzy(|playlist| {
                use crate::app::components::FuzzyFields;
                let owner = playlist.owner.name.clone().unwrap_or_default();
                let full = format!("{} {}", playlist.title, owner);
                FuzzyFields {
                    full,
                    title: Some(playlist.title.clone()),
                    artist: Some(owner),
                    album: None,
                }
            });

        Self {
            source,
            list,
            theme,
        }
    }

    pub fn set_loading(&self, _loading: bool) {}

    pub fn scroll_top(&mut self) {
        self.list.select_first();
    }

    pub fn scroll_bottom(&mut self) {
        self.list.select_last();
    }

    pub fn handle_key(&mut self, key: &Key, prefix: Option<char>) -> Action {
        let list_action = self.list.handle_key(key, prefix);
        if !list_action.is_none() {
            return list_action;
        }

        if prefix.is_some() {
            return Action::None;
        }

        if *key == Key::Enter
            && let Some(playlist) = self.list.selected_item()
        {
            let kind = playlist.kind;
            let title = playlist.title.clone();
            return Action::Navigate(Route::Playlist { kind, title });
        }

        Action::None
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        let is_loading = matches!(
            self.source.fetch_state(),
            crate::app::data::FetchState::Loading
        );

        if is_loading && self.source.total().is_none_or(|t| t == 0) {
            let spinner = Spinner::new()
                .with_label("Loading playlists...")
                .with_style(self.theme.get().accent);
            spinner.view(frame, area);
            return;
        }

        self.list.view(frame, area);
    }

    pub fn selection_signal(&self) -> Signal<usize> {
        self.list.selection_signal()
    }
}
