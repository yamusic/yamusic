use std::ops::Range;
use std::sync::Arc;

use im::Vector;
use yandex_music::model::{playlist::Playlist, track::Track};

use super::super::{DataSource, FetchState};
use super::playlist_info::PlaylistInfo;
use crate::app::data::providers::track::TrackDataSource;
use crate::framework::reactive::{
    Memo, Resource, ResourceState, With, create_effect, memo, signal,
};
use crate::framework::signals::Signal;
use crate::http::ApiService;
use yandex_music::model::playlist::PlaylistTracks;

pub struct PlaylistTracksSource {
    playlist_resource: Resource<Playlist>,
    playlist_info: Signal<Option<PlaylistInfo>>,
    total_count: Memo<usize>,
    kind: u32,
    track_source: TrackDataSource,
}

impl PlaylistTracksSource {
    pub fn new(kind: u32, api: Arc<ApiService>) -> Self {
        let playlist_info: Signal<Option<PlaylistInfo>> = signal(None);
        let track_source = TrackDataSource::new(kind, api.clone());

        let playlist_resource = Resource::new({
            let api = api.clone();
            move || {
                let api = api.clone();
                async move { api.fetch_playlist(kind).await.map_err(|e| e.to_string()) }
            }
        });

        let total_count = memo({
            let info = playlist_info.clone();
            move |_| With::with(&info, |i| i.as_ref().map_or(0, |p| p.track_count))
        });

        create_effect({
            let resource = playlist_resource.clone();
            let playlist_info = playlist_info.clone();
            let track_source = track_source.clone();

            move |_| {
                if let Some(playlist) = resource.value() {
                    playlist_info.set(Some(PlaylistInfo::from(&playlist)));

                    let ids: Vec<String> = match &playlist.tracks {
                        Some(PlaylistTracks::Full(full_tracks)) => {
                            full_tracks.iter().map(|t| t.id.clone()).collect()
                        }
                        Some(PlaylistTracks::WithInfo(infos)) => {
                            infos.iter().map(|ti| ti.track.id.clone()).collect()
                        }
                        Some(PlaylistTracks::Partial(partial)) => partial
                            .iter()
                            .map(|p| {
                                if let Some(album_id) = p.album_id {
                                    format!("{}:{}", p.id, album_id)
                                } else {
                                    p.id.clone()
                                }
                            })
                            .collect(),
                        None => Vec::new(),
                    };

                    track_source.set_track_ids(ids);
                }
            }
        });

        Self {
            playlist_resource,
            playlist_info,
            total_count,
            kind,
            track_source,
        }
    }

    pub fn playlist_info(&self) -> Signal<Option<PlaylistInfo>> {
        self.playlist_info.clone()
    }

    pub fn total_count(&self) -> Memo<usize> {
        self.total_count.clone()
    }

    pub fn kind(&self) -> u32 {
        self.kind
    }

    pub fn underlying_source(&self) -> &TrackDataSource {
        &self.track_source
    }
}

impl DataSource<Track> for PlaylistTracksSource {
    fn total(&self) -> Option<usize> {
        self.track_source.total()
    }

    fn range(&self, range: Range<usize>) -> Vector<Track> {
        self.track_source.range(range)
    }

    fn is_loaded(&self, range: Range<usize>) -> bool {
        self.track_source.is_loaded(range)
    }

    fn request_range(&self, range: Range<usize>) {
        if let ResourceState::Idle = self.playlist_resource.get() {
            self.playlist_resource.refetch();
        }
        self.track_source.request_range(range)
    }

    fn fetch_state(&self) -> FetchState {
        match self.playlist_resource.get() {
            ResourceState::Idle => FetchState::Idle,
            ResourceState::Loading => FetchState::Loading,
            ResourceState::Ready(_) | ResourceState::Stale(_) => self.track_source.fetch_state(),
            ResourceState::Error(e) => FetchState::Error(e),
        }
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.track_source.changed_signal()
    }

    fn refresh(&self) {
        self.playlist_resource.refetch();
        self.track_source.refresh();
    }
}
