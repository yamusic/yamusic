use std::time::Duration;

use yandex_music::model::{
    album::Album, artist::Artist, info::pager::Pager, playlist::Playlist, rotor::session::Session,
    search::Search, track::Track,
};

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    // Events
    Initialize,
    TrackStarted(Track, usize),
    TrackEnded,
    QueueEnded,
    PlaybackProgress(u64),
    TracksFetched(Vec<Track>),
    TrackFetched(Track),
    PlaylistFetched(Playlist),
    PlaylistTracksFetched(u32, Vec<Track>),
    PlaylistTracksPageFetched(u32, Vec<Track>, usize),
    AlbumTracksFetched(Vec<Track>),
    ArtistTracksFetched(Vec<Track>, Pager),
    ArtistTracksPageFetched(String, Vec<Track>, Pager),
    SearchPageFetched(Search, u32),
    PlaylistsFetched(Vec<Playlist>),
    PlaylistSelected(Playlist),
    PlaylistKindSelected(u32),
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
