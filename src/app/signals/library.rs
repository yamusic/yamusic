use im::{HashSet, Vector};

use yandex_music::model::playlist::Playlist;

use crate::framework::reactive::{Memo, Resource, Set, Signal, With, batch, memo, signal};
use crate::http::ApiService;
use std::sync::Arc;

#[derive(Clone)]
pub struct LibrarySignals {
    pub liked_track_ids: Signal<HashSet<String>>,

    pub disliked_track_ids: Signal<HashSet<String>>,

    pub playlists: Resource<Vector<Playlist>>,

    pub is_loading: Signal<bool>,

    pub liked_count: Memo<usize>,

    pub playlist_count: Memo<usize>,
}

impl LibrarySignals {
    pub fn new(api: Arc<ApiService>) -> Self {
        let liked_track_ids = signal::<HashSet<String>>(HashSet::new());

        let playlists = Resource::new({
            let api = api.clone();
            move || {
                let api = api.clone();
                async move {
                    api.fetch_all_playlists()
                        .await
                        .map(Vector::from)
                        .map_err(|e| e.to_string())
                }
            }
        });

        let liked_count = memo({
            let liked = liked_track_ids.clone();
            move |_| With::with(&liked, |ids| ids.len())
        });

        let playlist_count = memo({
            let playlists = playlists.clone();
            move |_| match playlists.value() {
                Some(p) => p.len(),
                None => 0,
            }
        });

        Self {
            liked_track_ids,
            disliked_track_ids: signal(HashSet::new()),
            playlists,
            is_loading: signal(false),
            liked_count,
            playlist_count,
        }
    }

    pub fn is_liked(&self, track_id: &str) -> bool {
        With::with(&self.liked_track_ids, |ids| ids.contains(track_id))
    }

    pub fn is_disliked(&self, track_id: &str) -> bool {
        With::with(&self.disliked_track_ids, |ids| ids.contains(track_id))
    }

    pub fn add_like(&self, track_id: String) {
        crate::framework::reactive::Update::update(&self.liked_track_ids, |ids| {
            ids.insert(track_id);
        });
    }

    pub fn remove_like(&self, track_id: &str) {
        crate::framework::reactive::Update::update(&self.liked_track_ids, |ids| {
            ids.remove(track_id);
        });
    }

    pub fn add_dislike(&self, track_id: String) {
        crate::framework::reactive::Update::update(&self.disliked_track_ids, |ids| {
            ids.insert(track_id);
        });
    }

    pub fn remove_dislike(&self, track_id: &str) {
        crate::framework::reactive::Update::update(&self.disliked_track_ids, |ids| {
            ids.remove(track_id);
        });
    }

    pub fn set_liked_snapshot<I1, I2>(&self, liked: I1, disliked: I2)
    where
        I1: Into<HashSet<String>>,
        I2: Into<HashSet<String>>,
    {
        let liked: HashSet<String> = liked.into();
        let disliked: HashSet<String> = disliked.into();
        batch(|| {
            Set::set(&self.liked_track_ids, liked);
            Set::set(&self.disliked_track_ids, disliked);
        });
    }

    pub fn set_playlists(&self, playlist_list: Vec<Playlist>) {
        self.playlists.set(Vector::from(playlist_list));
    }
}
