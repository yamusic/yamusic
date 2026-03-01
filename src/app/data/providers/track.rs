use std::ops::Range;
use std::sync::Arc;

use im::Vector;
use yandex_music::model::track::Track;

use super::super::{DataSource, FetchState};
use super::paginated::PaginatedDataSource;
use crate::{framework::signals::Signal, http::ApiService};

const PAGE_SIZE: usize = 50;

#[derive(Clone)]
pub struct TrackDataSource {
    inner: PaginatedDataSource<String, Track>,
    source_id: u32,
}

impl TrackDataSource {
    pub fn new(source_id: u32, api: Arc<ApiService>) -> Self {
        let inner = PaginatedDataSource::new(PAGE_SIZE, |_batch: Vec<String>, _end: usize| {});

        let inner_clone = inner.clone();
        let fetch_batch = move |batch: Vec<String>, batch_end: usize| {
            let api = api.clone();
            let inner = inner_clone.clone();

            tokio::spawn(async move {
                match api.fetch_tracks_by_ids(batch).await {
                    Ok(tracks) => {
                        let tracks: Vec<_> = tracks
                            .into_iter()
                            .filter(|t| t.available.unwrap_or(false))
                            .collect();
                        inner.append_items(tracks, batch_end);
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch tracks: {}", e);
                    }
                }
            });
        };

        inner.set_fetch_batch(fetch_batch);

        Self { inner, source_id }
    }

    pub fn set_track_ids(&self, track_ids: Vec<String>) {
        self.inner.set_ids(track_ids);
    }

    pub fn set_tracks(&self, tracks: Vec<Track>, loaded_count: usize) {
        self.inner.set_items(tracks, loaded_count);
    }

    pub fn set_item_ids(&self, track_ids: Vec<String>, tracks: Vec<Track>) {
        self.inner.set_item_ids(track_ids, tracks);
    }

    pub fn append_tracks(&self, new_tracks: Vec<Track>, new_loaded_count: usize) {
        self.inner.append_items(new_tracks, new_loaded_count);
    }

    pub fn has_more(&self) -> bool {
        self.inner.has_more()
    }

    pub fn is_loading(&self) -> bool {
        self.inner.is_loading()
    }

    pub fn source_id(&self) -> u32 {
        self.source_id
    }
}

impl DataSource<Track> for TrackDataSource {
    fn total(&self) -> Option<usize> {
        self.inner.total()
    }

    fn range(&self, range: Range<usize>) -> Vector<Track> {
        self.inner.range(range)
    }

    fn is_loaded(&self, range: Range<usize>) -> bool {
        self.inner.is_loaded(range)
    }

    fn request_range(&self, range: Range<usize>) {
        self.inner.request_range(range)
    }

    fn fetch_state(&self) -> FetchState {
        self.inner.fetch_state()
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.inner.changed_signal()
    }

    fn refresh(&self) {
        self.inner.refresh()
    }
}
