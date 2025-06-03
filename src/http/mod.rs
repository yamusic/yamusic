use std::sync::Arc;

use yandex_music::{
    model::track_model::track::{PartialTrack, Track},
    YandexMusicClient,
};

pub struct ApiService {
    pub client: Arc<YandexMusicClient>,
    user_id: i32,
}

impl ApiService {
    pub async fn new() -> color_eyre::Result<Self> {
        let client = Arc::new(YandexMusicClient::new(
            &std::env::var("YANDEX_MUSIC_TOKEN")
                .expect("YANDEX_MUSIC_TOKEN environment variable must be set"),
        ));
        let user_id = client
            .get_account_status()
            .await?
            .account
            .uid
            .ok_or(color_eyre::eyre::eyre!("No user id found"))?;

        Ok(Self { client, user_id })
    }

    pub async fn fetch_liked_tracks(
        &self,
    ) -> color_eyre::Result<Vec<PartialTrack>> {
        Ok(self.client.get_liked_tracks(self.user_id).await?.tracks)
    }

    pub async fn fetch_tracks(
        &self,
        track_ids: Vec<i32>,
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self.client.get_tracks(&track_ids, true).await?)
    }

    pub async fn fetch_tracks_partial(
        &self,
        tracks: &[PartialTrack],
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self
            .client
            .get_tracks(&tracks.iter().map(|t| t.id).collect::<Vec<_>>(), true)
            .await?)
    }

    pub async fn fetch_similar_tracks(
        &self,
        track_id: i32,
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self
            .client
            .get_similar_tracks(track_id)
            .await?
            .similar_tracks)
    }

    pub async fn fetch_track_url(
        &self,
        track_id: i32,
    ) -> color_eyre::Result<(String, String, i32)> {
        let download_info =
            self.client.get_track_download_info(track_id).await?;
        let info = download_info
            .iter()
            .max_by_key(|info| info.bitrate_in_kbps)
            .ok_or(color_eyre::eyre::eyre!("No download info found"))?;
        let url = info.get_direct_link(&self.client.client).await?;

        Ok((url, info.codec.clone(), info.bitrate_in_kbps))
    }
}
