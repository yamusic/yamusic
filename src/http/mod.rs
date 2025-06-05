use std::sync::Arc;

use yandex_music::{
    api::playlist::get_playlists::PlaylistsRequest,
    model::{
        playlist_model::playlist::Playlist,
        track_model::track::{PartialTrack, Track},
    },
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
    ) -> color_eyre::Result<(Playlist, Vec<PartialTrack>)> {
        let library = self.client.get_liked_tracks(self.user_id).await?;
        let playlist = self.client.get_playlist(self.user_id, 3).await?;

        Ok((playlist, library.tracks))
    }

    pub async fn fetch_all_playlists(
        &self,
    ) -> color_eyre::Result<Vec<Playlist>> {
        Ok(self.client.get_all_playlists(self.user_id).await?)
    }

    pub async fn fetch_playlists(
        &self,
        kinds: Vec<i32>,
    ) -> color_eyre::Result<Playlist> {
        self.client
            .get_playlists(
                &PlaylistsRequest::new(self.user_id)
                    .kinds(kinds)
                    .with_tracks(true),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("Playlist not found"))
    }

    pub async fn fetch_tracks(
        &self,
        track_ids: Vec<String>,
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self.client.get_tracks(&track_ids, true).await?)
    }

    pub async fn fetch_tracks_partial(
        &self,
        tracks: &[PartialTrack],
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self
            .client
            .get_tracks(
                &tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
                true,
            )
            .await?)
    }

    pub async fn fetch_similar_tracks(
        &self,
        track_id: String,
    ) -> color_eyre::Result<Vec<Track>> {
        Ok(self
            .client
            .get_similar_tracks(track_id)
            .await?
            .similar_tracks)
    }

    pub async fn fetch_track_url(
        &self,
        track_id: String,
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
