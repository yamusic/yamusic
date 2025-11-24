use std::time::Duration;

use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, rotor::session::Session, search::Search,
    track::Track,
};

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    // Events
    Initialize,
    TrackStarted(Track, usize),
    TrackEnded,
    PlaybackProgress(u64),
    TracksFetched(Vec<Track>),
    TrackFetched(Track),
    LikedTracksFetched(Vec<Track>),
    PlaylistTracksFetched(Vec<Track>),
    AlbumTracksFetched(Vec<Track>),
    ArtistTracksFetched(Vec<Track>),
    PlaylistsFetched(Vec<Playlist>),
    PlaylistSelected(Playlist),
    AlbumSelected(Album),
    ArtistSelected(Artist),
    TrackSelected(Track),
    LyricsFetched(Option<String>),
    SearchResults(Search),
    FetchError(String),
    WaveReady(Session, Vec<Track>),

    // Commands
    Play(i32),
    Search(String),
    Resume,
    Pause,
    Volume(u8),
    VolumeUp(u8),
    VolumeDown(u8),
    Next,
    Previous,
    Seek(u32),
    SeekForward(u32),
    SeekBackward(u32),
    ToggleMute,
}

pub enum ControlSignal {
    Stop,
    Seek(u64),
    SeekForward(u64),
    SeekBackward(u64),
}

pub enum PlayerCommand {
    Play,
    Pause,
    Volume(f32),
    Seek(Duration),
}
