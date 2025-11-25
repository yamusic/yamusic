use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, search::Search, track::Track,
};

#[derive(Debug, Clone)]
pub enum AppMessage {
    // User Input
    Quit,
    TogglePlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBackward,
    ToggleShuffle,
    ToggleRepeat,
    ToggleMute,

    // Navigation
    NavigateTo(ViewRoute),
    GoBack,
    NextSidebarItem,
    PreviousSidebarItem,
    SetSidebarIndex(usize),
    ToggleQueue,

    // Data Loaded
    LikedTracksLoaded(Vec<Track>),
    PlaylistsLoaded(Vec<Playlist>),
    PlaylistTracksLoaded(Vec<Track>),
    AlbumTracksLoaded(Vec<Track>),
    ArtistTracksLoaded(Vec<Track>),
    SearchResultsLoaded(Search),
    LyricsLoaded(Option<String>),

    // Audio Events
    TrackStarted(Track),
    TrackEnded,
    PlaybackProgress(u64), // ms

    // Errors
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ViewRoute {
    MyWave,
    TrackList,
    LikedTracks,
    Playlists,
    Search,
    PlaylistDetail(Playlist),
    AlbumDetail(Album),
    ArtistDetail(Artist),
    TrackDetail(Track),
    Lyrics,
}
