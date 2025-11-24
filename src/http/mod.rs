use std::sync::Arc;

use yandex_music::{
    YandexMusicClient,
    api::{
        album::get_album::GetAlbumOptions,
        artist::get_artist_tracks::ArtistTracksOptions,
        playlist::{get_all_playlists::GetAllPlaylistsOptions, get_playlists::GetPlaylistsOptions},
        rotor::{
            create_session::CreateSessionOptions, get_session_tracks::GetSessionTracksOptions,
        },
        search::get_search::SearchOptions,
        track::{
            get_file_info::GetFileInfoOptions, get_lyrics::GetLyricsOptions,
            get_similar_tracks::GetSimilarTracksOptions, get_tracks::GetTracksOptions,
        },
    },
    model::{
        album::Album,
        info::{file_info::Codec, lyrics::LyricsFormat},
        landing::wave::LandingWave,
        playlist::Playlist,
        rotor::session::Session,
        search::Search,
        track::{PartialTrack, Track},
    },
};

pub struct ApiService {
    pub client: Arc<YandexMusicClient>,
    user_id: u64,
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

    pub async fn search(&self, query: &str) -> color_eyre::Result<Search> {
        let opts = SearchOptions::new(query);
        Ok(self.client.search(&opts).await?)
    }

    pub async fn fetch_liked_tracks(&self) -> color_eyre::Result<Playlist> {
        let opts = GetPlaylistsOptions::new(self.user_id)
            .kinds([3u32])
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

    pub async fn fetch_playlist(&self, kind: u32) -> color_eyre::Result<Playlist> {
        self.client
            .get_playlists(
                &GetPlaylistsOptions::new(self.user_id)
                    .kinds([kind])
                    .with_tracks(true),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("Playlist not found"))
    }

    pub async fn fetch_playlists(&self, kinds: Vec<u32>) -> color_eyre::Result<Playlist> {
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
    ) -> color_eyre::Result<(String, String, u32)> {
        let opts = GetFileInfoOptions::new(track_id).codecs(vec![Codec::FlacMp4]);
        let info = self.client.get_file_info(&opts).await?;

        Ok((info.url, info.codec, info.bitrate))
    }

    pub async fn fetch_lyrics(
        &self,
        track_id: String,
        format: LyricsFormat,
    ) -> color_eyre::Result<Option<String>> {
        let opts = GetLyricsOptions::new(track_id, format);
        match self.client.get_lyrics(&opts).await {
            Ok(lyrics) => {
                let url = lyrics.download_url;
                let text = self.client.inner.get(url).send().await?.text().await?;
                Ok(Some(text))
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn fetch_album_with_tracks(&self, album_id: u32) -> color_eyre::Result<Album> {
        let opts = GetAlbumOptions::new(album_id).with_tracks();
        Ok(self.client.get_album(&opts).await?)
    }

    pub async fn fetch_artist_tracks(&self, artist_id: String) -> color_eyre::Result<Vec<Track>> {
        let opts = ArtistTracksOptions::new(artist_id);
        Ok(self.client.get_artist_tracks(&opts).await?.tracks)
    }

    pub async fn fetch_waves(&self) -> color_eyre::Result<Vec<LandingWave>> {
        Ok(self.client.get_waves().await?)
    }

    pub async fn create_session(&self, seeds: Vec<String>) -> color_eyre::Result<Session> {
        let opts = CreateSessionOptions::new(seeds);
        Ok(self.client.create_session(opts).await?)
    }

    pub async fn get_session_tracks(
        &self,
        session_id: String,
        queue: Vec<String>,
    ) -> color_eyre::Result<Session> {
        let opts = GetSessionTracksOptions::new(session_id, queue);
        Ok(self.client.get_session_tracks(opts).await?)
    }
}
