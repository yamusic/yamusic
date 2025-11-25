#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub data: AppData,
    pub ui: UiState,
}

#[derive(Debug, Clone, Default)]
pub struct AppData {
    pub lyrics: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub current_route: Route,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub sidebar_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Route {
    #[default]
    MyWave,
    TrackList,
    Playlists,
    Search,
    PlaylistDetail,
    AlbumDetail,
    ArtistDetail,
    TrackDetail,
    Lyrics,
}
