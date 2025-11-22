use crate::audio::progress::TrackProgress;
use crate::http::ApiService;
use crate::stream;
use reqwest::blocking::Client;
use std::sync::Arc;
use yandex_music::model::track::Track;

pub struct StreamManager {
    api: Arc<ApiService>,
    http_client: Client,
}

impl StreamManager {
    pub fn new(api: Arc<ApiService>) -> Self {
        let http_client = Client::builder()
            .build()
            .expect("failed to create http client");

        Self { api, http_client }
    }

    pub async fn create_stream_session(
        &self,
        track: &Track,
        progress: Arc<TrackProgress>,
    ) -> color_eyre::Result<stream::StreamingSession> {
        let (url, codec, bitrate) = self.api.fetch_track_url(track.id.clone()).await?;
        progress.set_bitrate(bitrate.try_into().unwrap());

        let http_client = self.http_client.clone();

        let session = tokio::task::spawn_blocking(move || {
            stream::create_streaming_session(http_client, url, codec, bitrate, progress)
        })
        .await??;

        Ok(session)
    }
}
