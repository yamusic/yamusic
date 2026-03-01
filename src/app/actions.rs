use im::Vector;
use yandex_music::model::track::Track;

use crate::audio::queue::PlaybackContext;

#[derive(Debug, Clone, Default)]
pub enum Action {
    #[default]
    None,
    Redraw,
    Navigate(Route),
    Back,
    Overlay(Route),
    DismissOverlay,
    PlayContext {
        context: PlaybackContext,
        tracks: Vector<Track>,
        start_index: usize,
    },
    PlayTrack(Track),
    TogglePlayback,
    NextTrack,
    PreviousTrack,
    SeekForward(u64),
    SeekBackward(u64),
    SetVolume(u8),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    LikeTrack(Track),
    UnlikeTrack(Track),
    DislikeTrack(Track),
    QueueTrack(Track),
    PlayNext(Track),
    RemoveFromQueue(usize),
    ClearQueue,
    LikeContext,
    DislikeContext,
    QueueAll,
    PlayAllNext,
    FetchData {
        source_id: String,
        range: (usize, usize),
    },
    Refresh,
    Search(String),
    SearchNextPage,
    StartWave {
        seeds: Vec<String>,
        title: Option<String>,
        toast_message: Option<Vec<ratatui::text::Line<'static>>>,
    },
    RefreshWaves,
    Quit,
    Toast(String),
    Focus(String),
    ScrollTop,
    ScrollBottom,
    Batch(Vec<Action>),
}

impl Action {
    pub fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }

    pub fn and(self, other: Action) -> Action {
        match (self, other) {
            (Action::None, other) => other,
            (this, Action::None) => this,
            (Action::Batch(mut a), Action::Batch(b)) => {
                a.extend(b);
                Action::Batch(a)
            }
            (Action::Batch(mut a), other) => {
                a.push(other);
                Action::Batch(a)
            }
            (this, Action::Batch(mut b)) => {
                b.insert(0, this);
                Action::Batch(b)
            }
            (this, other) => Action::Batch(vec![this, other]),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Home,
    Search,
    Playlists,
    Liked,
    Playlist { kind: u32, title: String },
    Album { id: String, title: String },
    Artist { id: String, name: String },
    Track { id: String },
    Lyrics,
    Queue,
    Settings,
}

impl Route {
    pub fn title(&self) -> String {
        match self {
            Route::Home => "My Wave".to_string(),
            Route::Search => "Search".to_string(),
            Route::Liked => "Liked Tracks".to_string(),
            Route::Playlists => "Playlists".to_string(),
            Route::Playlist { title, .. } => title.clone(),
            Route::Album { title, .. } => title.clone(),
            Route::Artist { name, .. } => name.clone(),
            Route::Track { id } => format!("Track {}", id),
            Route::Lyrics => "Lyrics".to_string(),
            Route::Queue => "Queue".to_string(),
            Route::Settings => "Settings".to_string(),
        }
    }
}
