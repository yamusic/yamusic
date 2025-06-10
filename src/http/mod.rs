use std::sync::Arc;

use yandex_music::{
    YandexMusicClient,
    api::{
        playlist::{get_all_playlists::GetAllPlaylistsOptions, get_playlists::GetPlaylistsOptions},
        track::{
            get_download_info::GetDownloadInfoOptions, get_similar_tracks::GetSimilarTracksOptions,
            get_tracks::GetTracksOptions,
        },
    },
    model::{
        playlist::Playlist,
        track::{PartialTrack, Track},
    },
};

pub struct ApiService {
    pub client: Arc<YandexMusicClient>,
    user_id: i32,
}

impl ApiService {
    pub async fn new() -> color_eyre::Result<Self> {
        let client = Arc::new(
            YandexMusicClient::builder(
                &std::env::var("YANDEX_MUSIC_TOKEN")
                    .expect("YANDEX_MUSIC_TOKEN environment variable must be set"),
            )
            .build()?,
        );
        let user_id = client
            .get_account_status()
            .await?
            .account
            .uid
            .ok_or(color_eyre::eyre::eyre!("No user id found"))?;

        Ok(Self { client, user_id })
    }

    pub async fn fetch_liked_tracks(&self) -> color_eyre::Result<Playlist> {
        let opts = GetPlaylistsOptions::new(self.user_id)
            .kinds([3])
            .with_tracks(true);
        let playlist = self.client.get_playlists(&opts).await?;

        playlist
            .into_iter()
            .next()
            .ok_or(color_eyre::eyre::eyre!("Playlist not found"))
    }

    pub async fn fetch_all_playlists(&self) -> color_eyre::Result<Vec<Playlist>> {
        let opts = GetAllPlaylistsOptions::new(self.user_id);
        Ok(self.client.get_all_playlists(&opts).await?)
    }

    pub async fn fetch_playlists(&self, kinds: Vec<i32>) -> color_eyre::Result<Playlist> {
        self.client
            .get_playlists(
                &GetPlaylistsOptions::new(self.user_id)
                    .kinds(kinds)
                    .with_tracks(true),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("Playlist not found"))
    }

    pub async fn fetch_tracks(&self, track_ids: Vec<String>) -> color_eyre::Result<Vec<Track>> {
        let opts = GetTracksOptions::new(track_ids);
        Ok(self.client.get_tracks(&opts).await?)
    }

    pub async fn fetch_tracks_partial(
        &self,
        tracks: &[PartialTrack],
    ) -> color_eyre::Result<Vec<Track>> {
        let opts = GetTracksOptions::new(tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>());
        Ok(self.client.get_tracks(&opts).await?)
    }

    pub async fn fetch_similar_tracks(&self, track_id: String) -> color_eyre::Result<Vec<Track>> {
        let opts = GetSimilarTracksOptions::new(track_id);
        Ok(self.client.get_similar_tracks(&opts).await?.similar_tracks)
    }

    pub async fn fetch_track_url(
        &self,
        track_id: String,
    ) -> color_eyre::Result<(String, String, i32)> {
        let opts = GetDownloadInfoOptions::new(track_id);
        let download_info = self.client.get_download_info(&opts).await?;
        let info = download_info
            .iter()
            .max_by_key(|info| info.bitrate_in_kbps)
            .ok_or(color_eyre::eyre::eyre!("No download info found"))?;
        let url = info.get_direct_link(&self.client.inner).await?;

        Ok((url, info.codec.clone(), info.bitrate_in_kbps))
    }
}
