use crate::audio::cache::UrlCache;
use crate::audio::progress::TrackProgress;
use crate::http::ApiService;
use crate::stream;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::info;
use yandex_music::model::track::Track;

pub struct StreamManager {
    api: Arc<ApiService>,
    http_client: Client,
    url_cache: UrlCache,
    prewarm_cache: Arc<Mutex<HashMap<String, (stream::StreamingSession, Arc<TrackProgress>)>>>,
}

impl StreamManager {
    pub fn new(api: Arc<ApiService>, url_cache: UrlCache) -> Self {
        let http_client = Client::builder()
            .build()
            .expect("failed to create http client");

        Self {
            api,
            http_client,
            url_cache,
            prewarm_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn prewarm(&self, track: Track) {
        let id = track.id.clone();
        if self.prewarm_cache.lock().unwrap().contains_key(&id) {
            return;
        }

        let this = self.clone();
        tokio::spawn(async move {
            if let Ok(result) = this.create_stream_session(&track).await {
                this.prewarm_cache.lock().unwrap().insert(track.id, result);
            }
        });
    }

    pub async fn create_stream_session(
        &self,
        track: &Track,
    ) -> color_eyre::Result<(stream::StreamingSession, Arc<TrackProgress>)> {
        {
            let mut cache = self.prewarm_cache.lock().unwrap();
            if let Some((session, progress)) = cache.remove(&track.id) {
                info!(id = track.id.as_str(), "stream_manager_prewarm_hit");
                return Ok((session, progress));
            }
        }

        let start = std::time::Instant::now();
        let (url, codec, bitrate) =
            if let Some((url, codec, bitrate)) = self.url_cache.get(&track.id) {
                info!(id = track.id.as_str(), "stream_manager_cache_hit");
                (url, codec, bitrate)
            } else {
                info!(id = track.id.as_str(), "stream_manager_cache_miss");
                let (url, codec, bitrate) = self.api.fetch_track_url(track.id.clone()).await?;
                self.url_cache
                    .insert(track.id.clone(), url.clone(), codec.clone(), bitrate);
                (url, codec, bitrate)
            };

        info!(
            id = track.id.as_str(),
            url = url.as_str(),
            "stream_manager_url_resolved"
        );

        let progress = Arc::new(TrackProgress::new());
        progress.set_bitrate(bitrate.try_into().unwrap());

        let http_client = self.http_client.clone();
        let progress_clone = progress.clone();

        let session = tokio::task::spawn_blocking(move || {
            stream::create_streaming_session(http_client, url, codec, bitrate, progress_clone)
        })
        .await??;

        info!(
            id = track.id.as_str(),
            elapsed_ms = start.elapsed().as_millis(),
            "stream_manager_session_created"
        );

        Ok((session, progress))
    }
}

impl Clone for StreamManager {
    fn clone(&self) -> Self {
        Self {
            api: self.api.clone(),
            http_client: self.http_client.clone(),
            url_cache: self.url_cache.clone(),
            prewarm_cache: self.prewarm_cache.clone(),
        }
    }
}
