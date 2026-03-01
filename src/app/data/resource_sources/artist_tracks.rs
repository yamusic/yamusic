use std::ops::Range;
use std::sync::Arc;

use im::Vector;
use yandex_music::model::track::Track;

use super::super::{DataSource, FetchState};
use crate::framework::reactive::{Update, create_effect, signal};
use crate::framework::resources::{PaginatedResource, ResourceState};
use crate::framework::signals::Signal;
use crate::http::ApiService;

pub struct ArtistTracksSource {
    resource: PaginatedResource<Track>,
    changed: Signal<u64>,
    artist_id: String,
    api: Arc<ApiService>,
}

impl ArtistTracksSource {
    pub fn new(artist_id: String, api: Arc<ApiService>) -> Self {
        let changed = signal(0u64);
        let resource = PaginatedResource::new();

        create_effect({
            let resource = resource.clone();
            let changed = changed.clone();
            move |_| {
                resource.state().track();
                Update::update(&changed, |v| *v += 1);
            }
        });

        let source = Self {
            resource,
            changed,
            artist_id,
            api,
        };

        source.request_more();

        source
    }

    pub fn artist_id(&self) -> &str {
        &self.artist_id
    }

    fn request_more(&self) {
        if self.resource.state().with(|s| s.is_loading()) {
            return;
        }

        let api = self.api.clone();
        let artist_id = self.artist_id.clone();

        self.resource.load_next(move |page| {
            let api = api.clone();
            let artist_id = artist_id.clone();
            async move {
                match api
                    .fetch_artist_tracks_paginated(artist_id, page as u32, 50)
                    .await
                {
                    Ok((tracks, pager)) => {
                        let has_more = (page as u32 + 1) * 50 < pager.total;
                        Ok((tracks, has_more))
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
        });
    }
}

impl DataSource<Track> for ArtistTracksSource {
    fn total(&self) -> Option<usize> {
        Some(self.resource.items().with(|items| items.len()))
    }

    fn range(&self, range: Range<usize>) -> Vector<Track> {
        self.resource.items().with(|items| {
            let start = range.start.min(items.len());
            let end = range.end.min(items.len());
            Vector::from(items[start..end].to_vec())
        })
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        true
    }

    fn request_range(&self, range: Range<usize>) {
        if range.end
            > self
                .resource
                .items()
                .with(|items| items.len())
                .saturating_sub(10)
        {
            self.request_more();
        }
    }

    fn fetch_state(&self) -> FetchState {
        match self.resource.state().get() {
            ResourceState::<(), String>::Idle => FetchState::Idle,
            ResourceState::<(), String>::Loading => FetchState::Loading,
            ResourceState::<(), String>::Ready(()) => FetchState::Loaded,
            ResourceState::<(), String>::Error(e) => FetchState::Error(e),
            _ => FetchState::Loaded,
        }
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {
        self.resource.reset();
        self.request_more();
    }
}
