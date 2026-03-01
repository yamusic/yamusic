use std::sync::Arc;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};
use yandex_music::model::{playlist::Playlist, track::Track};

use crate::{
    app::{
        actions::Action,
        components::{DynamicList, Header, HeaderBuilder, Spinner},
        data::{DataSource, PlaylistInfo},
        keymap::Key,
        signals::AppSignals,
        views::TrackRenderer,
    },
    audio::queue::PlaybackContext,
    framework::{signals::Signal, theme::ThemeStyles},
};

#[derive(Debug, Clone)]
pub enum TrackListContext {
    Playlist {
        kind: u32,
        title: String,
        owner: String,
        owner_uid: u64,
        track_count: usize,
    },
    Album {
        id: String,
        title: String,
        artists: String,
        year: Option<i32>,
        track_count: usize,
    },
    Artist {
        id: String,
        name: String,
        genres: String,
        likes: u64,
        track_count: usize,
    },
    Search {
        query: String,
        result_count: usize,
    },
    Queue,
    Standalone,
}

impl TrackListContext {
    fn build_header(&self, theme: Signal<ThemeStyles>) -> Option<Header> {
        match self {
            TrackListContext::Playlist {
                title,
                owner,
                track_count,
                ..
            } => Some(HeaderBuilder::playlist(
                title,
                owner,
                *track_count,
                None,
                theme,
            )),
            TrackListContext::Album {
                title,
                artists,
                year,
                track_count,
                ..
            } => Some(HeaderBuilder::album(
                title,
                artists,
                *year,
                *track_count,
                theme,
            )),
            TrackListContext::Artist {
                name,
                genres,
                likes,
                track_count,
                ..
            } => Some(HeaderBuilder::artist(
                name,
                genres,
                *likes,
                *track_count,
                theme,
            )),
            TrackListContext::Search {
                query,
                result_count,
            } => Some(HeaderBuilder::search(query, *result_count, theme)),
            TrackListContext::Queue | TrackListContext::Standalone => None,
        }
    }

    fn playback_context(&self) -> PlaybackContext {
        match self {
            TrackListContext::Playlist {
                kind: _, title: _, ..
            } => PlaybackContext::Standalone,
            TrackListContext::Album { .. } => PlaybackContext::Standalone,
            TrackListContext::Artist { .. } => PlaybackContext::Standalone,
            _ => PlaybackContext::Standalone,
        }
    }
}

pub struct TrackListView {
    context: TrackListContext,
    source: Arc<dyn DataSource<Track>>,
    list: DynamicList<Track>,
    header: Option<Header>,
    theme: Signal<ThemeStyles>,
    playlist: Option<Playlist>,
    playlist_info_signal: Option<Signal<Option<PlaylistInfo>>>,
}

impl TrackListView {
    pub fn new(
        context: TrackListContext,
        source: Arc<dyn DataSource<Track>>,
        signals: &AppSignals,
    ) -> Self {
        let theme = signals.theme.styles().clone();
        let mut renderer = TrackRenderer::new(
            signals.library.clone(),
            signals.audio.current_track_id.clone(),
            signals.audio.is_playing.clone(),
            theme.clone(),
        );

        if matches!(context, TrackListContext::Queue) {
            renderer = renderer.with_queue_index(signals.audio.queue_index.clone());
        }

        let renderer = Arc::new(renderer);
        let list = DynamicList::new(source.clone(), renderer, theme.clone()).with_fuzzy(|track| {
            use crate::app::components::FuzzyFields;
            let title = track.title.clone().unwrap_or_default();
            let artists = track
                .artists
                .iter()
                .filter_map(|a| a.name.as_ref())
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let album = track
                .albums
                .first()
                .and_then(|a| a.title.clone())
                .unwrap_or_default();
            let full = format!("{} {} {}", title, artists, album);
            FuzzyFields {
                full,
                title: Some(title),
                artist: Some(artists),
                album: Some(album),
            }
        });
        let header = context.build_header(theme.clone());

        Self {
            context,
            source,
            list,
            header,
            theme,
            playlist: None,
            playlist_info_signal: None,
        }
    }

    pub fn with_playlist_info(mut self, info: Signal<Option<PlaylistInfo>>) -> Self {
        self.playlist_info_signal = Some(info);
        self
    }

    pub fn context(&self) -> &TrackListContext {
        &self.context
    }

    fn maybe_update_header(&mut self) {
        let info = match &self.playlist_info_signal {
            Some(sig) => match sig.get() {
                Some(info) => info,
                None => return,
            },
            None => return,
        };

        let needs_update = matches!(
            &self.context,
            TrackListContext::Playlist { track_count, .. } if *track_count == 0
        );
        if needs_update && info.track_count > 0 {
            if let TrackListContext::Playlist {
                track_count,
                owner,
                owner_uid,
                ..
            } = &mut self.context
            {
                *track_count = info.track_count;
                *owner = info.owner.clone();
                *owner_uid = info.owner_uid;
            }
            self.header = self.context.build_header(self.theme.clone());
        }
    }

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

        if key == &Key::Enter {
            if let Some(_track) = self.list.selected_item() {
                let index = self.list.selected();
                let tracks = self.source.range(0..self.source.total().unwrap_or(0));
                let context = if let Some(playlist) = &self.playlist {
                    PlaybackContext::Playlist(playlist.clone())
                } else {
                    self.context.playback_context()
                };
                return Action::PlayContext {
                    context,
                    tracks,
                    start_index: index,
                };
            }
        }

        Action::None
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.maybe_update_header();

        use crate::app::data::FetchState;
        let is_loading = matches!(self.source.fetch_state(), FetchState::Loading);
        let no_tracks = self.source.total().is_none_or(|t| t == 0);

        if let Some(header) = &self.header {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(header.height()), Constraint::Min(0)])
                .split(area);

            header.view(frame, chunks[0]);

            if is_loading && no_tracks {
                let spinner = Spinner::new()
                    .with_label("Loading tracks...")
                    .with_style(self.theme.get().accent);
                spinner.view(frame, chunks[1]);
            } else {
                self.list.view(frame, chunks[1]);
            }
        } else if is_loading && no_tracks {
            let spinner = Spinner::new()
                .with_label("Loading tracks...")
                .with_style(self.theme.get().accent);
            spinner.view(frame, area);
        } else {
            self.list.view(frame, area);
        }
    }

    pub fn set_playlist(&mut self, playlist: Playlist) {
        self.playlist = Some(playlist.clone());
        if let TrackListContext::Playlist {
            title,
            owner,
            owner_uid,
            track_count,
            ..
        } = &mut self.context
        {
            *title = playlist.title.clone();
            *owner = playlist.owner.name.clone().unwrap_or_default();
            *owner_uid = playlist.owner.uid;
            *track_count = playlist.track_count as usize;
        }

        self.header = self.context.build_header(self.theme.clone());
    }

    pub fn selection_signal(&self) -> Signal<usize> {
        self.list.selection_signal()
    }

    pub fn selected_index(&self) -> usize {
        self.list.selected()
    }

    pub fn selected_item(&self) -> Option<Track> {
        self.list.selected_item()
    }

    pub fn items(&self) -> im::Vector<Track> {
        let total = self.source.total().unwrap_or(0);
        self.source.range(0..total)
    }
}
