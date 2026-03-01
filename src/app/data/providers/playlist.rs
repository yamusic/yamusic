use std::ops::Range;

use im::Vector;
use yandex_music::model::playlist::Playlist;

use super::super::{DataSource, FetchState};
use crate::framework::reactive::{Resource, ResourceState, create_effect};
use crate::framework::signals::Signal;

pub struct PlaylistDataSource {
    resource: Resource<Vector<Playlist>>,
    changed: Signal<u64>,
}

impl PlaylistDataSource {
    pub fn new(resource: Resource<Vector<Playlist>>) -> Self {
        let changed = Signal::new(0);

        create_effect({
            let resource = resource.clone();
            let changed = changed.clone();
            move |_| {
                resource.state.track();
                changed.update(|v| *v += 1);
            }
        });

        Self { resource, changed }
    }
}

impl DataSource<Playlist> for PlaylistDataSource {
    fn total(&self) -> Option<usize> {
        self.resource.value().map(|p| p.len())
    }

    fn range(&self, range: Range<usize>) -> Vector<Playlist> {
        self.resource
            .value()
            .map(|playlists| {
                let start = range.start.min(playlists.len());
                let end = range.end.min(playlists.len());
                playlists.clone().slice(start..end)
            })
            .unwrap_or_default()
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        self.resource.is_ready()
    }

    fn request_range(&self, _range: Range<usize>) {
        if self
            .resource
            .state
            .with(|s| matches!(s, ResourceState::Idle))
        {
            self.resource.refetch();
        }
    }

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
