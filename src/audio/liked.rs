use std::collections::HashSet;

use yandex_music::model::collection::Collection;

pub type LikedSnapshot = (HashSet<String>, HashSet<String>);

#[derive(Debug, Clone, Default)]
pub struct LikedCache {
    pub revision: Option<u64>,
    liked_ids: HashSet<String>,
    disliked_ids: HashSet<String>,
    liked_albums_ids: HashSet<u32>,
    liked_artists_ids: HashSet<String>,
    disliked_artists_ids: HashSet<String>,
    liked_playlists_ids: HashSet<String>,
}

impl LikedCache {
    pub fn apply_collection(&mut self, collection: Collection) {
        if let Some(tracks) = collection.liked_tracks {
            self.liked_ids = tracks.liked.iter().map(|t| t.track_id.clone()).collect();
            self.disliked_ids = tracks.disliked.iter().map(|t| t.track_id.clone()).collect();
            self.revision = Some(tracks.info.revision);
        }
        if let Some(albums) = collection.liked_albums {
            self.liked_albums_ids = albums.liked.iter().map(|a| a.album_id as u32).collect();
        }
        if let Some(artists) = collection.liked_artists {
            self.liked_artists_ids = artists
                .liked
                .iter()
                .map(|a| a.artist_id.to_string())
                .collect();
            self.disliked_artists_ids = artists
                .disliked
                .iter()
                .map(|a| a.artist_id.to_string())
                .collect();
        }
        if let Some(playlists) = collection.liked_playlists {
            self.liked_playlists_ids = playlists
                .liked
                .iter()
                .map(|p| format!("{}:{}", p.composite_data.uid, p.composite_data.kind))
                .collect();
        }
    }

    pub fn snapshot(&self) -> LikedSnapshot {
        (self.liked_ids.clone(), self.disliked_ids.clone())
    }

    pub fn set_like_status(&mut self, track_id: &str, liked: bool) {
        if liked {
            self.liked_ids.insert(track_id.to_string());
        } else {
            self.liked_ids.remove(track_id);
        }
    }

    pub fn set_dislike_status(&mut self, track_id: &str, disliked: bool) {
        if disliked {
            self.disliked_ids.insert(track_id.to_string());
        } else {
            self.disliked_ids.remove(track_id);
        }
    }

    pub fn is_liked(&self, track_id: &str) -> bool {
        self.liked_ids.contains(track_id)
    }

    pub fn is_disliked(&self, track_id: &str) -> bool {
        self.disliked_ids.contains(track_id)
    }

    pub fn is_album_liked(&self, album_id: u32) -> bool {
        self.liked_albums_ids.contains(&album_id)
    }

    pub fn is_artist_liked(&self, artist_id: &str) -> bool {
        self.liked_artists_ids.contains(artist_id)
    }

    pub fn is_artist_disliked(&self, artist_id: &str) -> bool {
        self.disliked_artists_ids.contains(artist_id)
    }

    pub fn is_playlist_liked(&self, uid: u64, kind: u32) -> bool {
        self.liked_playlists_ids
            .contains(&format!("{}:{}", uid, kind))
    }
}
