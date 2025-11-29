use yandex_music::model::playlist::PlaylistTracks;

pub fn extract_track_ids(playlist_tracks: &PlaylistTracks) -> Vec<String> {
    match playlist_tracks {
        PlaylistTracks::Full(tracks) => tracks
            .iter()
            .map(|t| {
                if let Some(album_id) = t.albums.first().and_then(|a| a.id) {
                    format!("{}:{}", t.id, album_id)
                } else {
                    t.id.clone()
                }
            })
            .collect(),
        PlaylistTracks::WithInfo(tracks) => tracks
            .iter()
            .map(|t| {
                if let Some(album_id) = t.track.albums.first().and_then(|a| a.id) {
                    format!("{}:{}", t.track.id, album_id)
                } else {
                    t.track.id.clone()
                }
            })
            .collect(),
        PlaylistTracks::Partial(partial) => partial
            .iter()
            .map(|p| {
                if let Some(album_id) = p.album_id {
                    format!("{}:{}", p.id, album_id)
                } else {
                    p.id.clone()
                }
            })
            .collect(),
    }
}
