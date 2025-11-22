use crate::{audio::system::AudioSystem, event::events::Event, http::ApiService};
use flume::Sender;
use std::sync::Arc;
use yandex_music::model::{playlist::Playlist, search::Search, track::Track};

pub struct AppContext {
    pub api: Arc<ApiService>,
    pub audio_system: AudioSystem,
    pub event_tx: Sender<Event>,
}

#[derive(Clone, Debug)]
pub struct GlobalUiState {
    pub sidebar_index: usize,
    pub playlists: Vec<Playlist>,
    pub lyrics: Option<String>,
    pub liked_tracks: Vec<Track>,
    pub search_results: Option<Search>,
}

impl Default for GlobalUiState {
    fn default() -> Self {
        Self {
            sidebar_index: 0,
            playlists: Vec::new(),
            lyrics: None,
            liked_tracks: Vec::new(),
            search_results: None,
        }
    }
}
