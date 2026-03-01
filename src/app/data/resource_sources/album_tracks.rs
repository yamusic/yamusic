use std::ops::Range;
use std::sync::Arc;

use im::Vector;
use yandex_music::model::track::Track;

use super::super::{DataSource, FetchState};
use crate::framework::reactive::{Resource, ResourceState, Update, create_effect, signal};
use crate::framework::signals::Signal;
use crate::http::ApiService;

pub struct AlbumTracksSource {
    resource: Resource<Vector<Track>>,
    changed: Signal<u64>,
    album_id: u32,
}

impl AlbumTracksSource {
    pub fn new(album_id: u32, api: Arc<ApiService>) -> Self {
        let changed = signal(0u64);

        let resource = Resource::new({
            let api = api.clone();
            move || {
                let api = api.clone();
                async move {
                    match api.fetch_album_with_tracks(album_id).await {
                        Ok(album) => {
                            let tracks: Vec<_> = album.volumes.into_iter().flatten().collect();
                            Ok(Vector::from(tracks))
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
            }
        });

        create_effect({
            let resource = resource.clone();
            let changed = changed.clone();
            move |_| {
                resource.state.track();
                Update::update(&changed, |v| *v += 1);
            }
        });

        Self {
            resource,
            changed,
            album_id,
        }
    }

    pub fn album_id(&self) -> u32 {
        self.album_id
    }
}

impl DataSource<Track> for AlbumTracksSource {
    fn total(&self) -> Option<usize> {
        self.resource.value().map(|v| v.len())
    }

    fn range(&self, range: Range<usize>) -> Vector<Track> {
        self.resource
            .value()
            .map(|tracks| {
                let start = range.start.min(tracks.len());
                let end = range.end.min(tracks.len());
                tracks.clone().slice(start..end)
            })
            .unwrap_or_default()
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        self.resource.is_ready()
    }

    fn request_range(&self, _range: Range<usize>) {}

    fn fetch_state(&self) -> FetchState {
        match self.resource.get() {
            ResourceState::Idle => FetchState::Idle,
            ResourceState::Loading => FetchState::Loading,
            ResourceState::Ready(_) | ResourceState::Stale(_) => FetchState::Loaded,
            ResourceState::Error(e) => FetchState::Error(e),
        }
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {
        self.resource.refetch();
    }
}
