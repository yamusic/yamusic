use yandex_music::model::playlist::Playlist;

#[derive(Debug, Clone, Default)]
pub struct PlaylistInfo {
    pub title: String,
    pub owner: String,
    pub owner_uid: u64,
    pub track_count: usize,
    pub duration_ms: Option<u64>,
    pub description: Option<String>,
}

impl From<&Playlist> for PlaylistInfo {
    fn from(playlist: &Playlist) -> Self {
        Self {
            title: playlist.title.clone(),
            owner: playlist.owner.name.clone().unwrap_or_default(),
            owner_uid: playlist.owner.uid,
            track_count: playlist.track_count as usize,
            duration_ms: Some(playlist.duration.as_millis() as u64),
            description: playlist.description.clone(),
        }
    }
}
