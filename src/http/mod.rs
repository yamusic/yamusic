use std::sync::Arc;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use yandex_music::{
    DEFAULT_CLIENT_ID, YandexMusicClient,
    api::{
        album::{
            add_liked_album::AddLikedAlbumOptions, get_album::GetAlbumOptions,
            remove_liked_album::RemoveLikedAlbumOptions,
        },
        artist::{
            add_disliked_artist::AddDislikedArtistOptions, add_liked_artist::AddLikedArtistOptions,
            get_artist_tracks::ArtistTracksOptions,
            remove_disliked_artist::RemoveDislikedArtistOptions,
            remove_liked_artist::RemoveLikedArtistOptions,
        },
        collection::sync::{CollectionSyncOption, CollectionSyncOptions},
        playlist::{
            add_liked_playlist::AddLikedPlaylistOptions, get_all_playlists::GetAllPlaylistsOptions,
            get_playlists::GetPlaylistsOptions, remove_liked_playlist::RemoveLikedPlaylistOptions,
        },
        rotor::{
            create_session::CreateSessionOptions, get_session_tracks::GetSessionTracksOptions,
        },
        search::get_search::SearchOptions,
        track::{
            add_disliked_tracks::AddDislikedTracksOptions, add_liked_tracks::AddLikedTracksOptions,
            get_file_info::GetFileInfoOptions, get_file_info_batch::GetFileInfoBatchOptions,
            get_lyrics::GetLyricsOptions, get_similar_tracks::GetSimilarTracksOptions,
            get_tracks::GetTracksOptions, remove_disliked_tracks::RemoveDislikedTracksOptions,
            remove_liked_tracks::RemoveLikedTracksOptions,
        },
    },
    model::{
        album::Album,
        collection::Collection,
        info::{lyrics::LyricsFormat, pager::Pager},
        playlist::Playlist,
        rotor::{Rotor, session::Session},
        search::Search,
        track::Track,
    },
};

pub struct ApiService {
    pub client: Arc<YandexMusicClient>,
    user_id: u64,
}

impl ApiService {
    pub async fn new(
        token: String,
        client: Option<Arc<YandexMusicClient>>,
        user_id: Option<u64>,
    ) -> color_eyre::Result<Self> {
        let client = if let Some(c) = client {
            c
        } else {
            let mut headers = HeaderMap::new();

            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("OAuth {}", token))?,
            );
            headers.insert(
                "X-Yandex-Music-Client",
                HeaderValue::from_str(DEFAULT_CLIENT_ID)?,
            );
            headers.insert("Accept-Language", HeaderValue::from_str("en")?);
            headers.insert("Accept", HeaderValue::from_str("*/*")?);
            headers.insert(
                "Origin",
                HeaderValue::from_str("music-application://desktop")?,
            );

            let http_client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) YandexMusic/5.82.0 Chrome/140.0.7339.133 Electron/38.2.2 Safari/537.36")
                .default_headers(headers.clone())
                .pool_max_idle_per_host(0)
                .build()?;

            Arc::new(YandexMusicClient::from_client(http_client))
        };

        let user_id = if let Some(uid) = user_id {
            uid
        } else {
            client
                .get_account_status()
                .await?
                .account
                .uid
                .ok_or(color_eyre::eyre::eyre!("No user id found"))?
        };

        Ok(Self { client, user_id })
    }

    pub fn current_user_id(&self) -> u64 {
        self.user_id
    }

    pub async fn search(&self, query: &str) -> color_eyre::Result<Search> {
        let opts = SearchOptions::new(query);
        Ok(self.client.search(&opts).await?)
    }

    pub async fn search_paginated(&self, query: &str, page: u32) -> color_eyre::Result<Search> {
        let opts = SearchOptions::new(query).page(page);
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

    pub async fn fetch_playlist_bare(&self, kind: u32) -> color_eyre::Result<Playlist> {
        self.client
            .get_playlists(
                &GetPlaylistsOptions::new(self.user_id)
                    .kinds([kind])
                    .with_tracks(true)
                    .rich_tracks(false),
            )
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| color_eyre::eyre::eyre!("Playlist not found"))
    }

    pub async fn fetch_tracks_by_ids(
        &self,
        track_album_ids: Vec<String>,
    ) -> color_eyre::Result<Vec<Track>> {
        let opts = GetTracksOptions::new(track_album_ids);
        Ok(self.client.get_tracks(&opts).await?)
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

    pub async fn fetch_similar_tracks(&self, track_id: String) -> color_eyre::Result<Vec<Track>> {
        let opts = GetSimilarTracksOptions::new(track_id);
        Ok(self.client.get_similar_tracks(&opts).await?.similar_tracks)
    }

    pub async fn fetch_track_url(
        &self,
        track_id: String,
    ) -> color_eyre::Result<(String, String, u32)> {
        let opts = GetFileInfoOptions::new(track_id);
        let info = self.client.get_file_info(&opts).await?;

        Ok((info.url, info.codec, info.bitrate))
    }

    pub async fn fetch_track_urls_batch(
        &self,
        track_ids: Vec<String>,
    ) -> color_eyre::Result<Vec<(String, String, String, u32)>> {
        let opts = GetFileInfoBatchOptions::new(track_ids.clone());
        let results = self.client.get_file_info_batch(&opts).await?;

        let mut mapped = Vec::new();
        for (i, info) in results.into_iter().enumerate() {
            if let Some(track_id) = track_ids.get(i) {
                mapped.push((track_id.clone(), info.url, info.codec, info.bitrate));
            }
        }
        Ok(mapped)
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

    pub async fn fetch_artist_tracks_paginated(
        &self,
        artist_id: String,
        page: u32,
        page_size: u32,
    ) -> color_eyre::Result<(Vec<Track>, Pager)> {
        let opts = ArtistTracksOptions::new(artist_id)
            .page(page)
            .page_size(page_size);
        let result = self.client.get_artist_tracks(&opts).await?;
        Ok((result.tracks, result.pager))
    }

    pub async fn fetch_stations(&self) -> color_eyre::Result<Vec<Rotor>> {
        let opts = yandex_music::api::rotor::get_all_stations::GetAllStationsOptions::default();
        Ok(self.client.get_all_stations(&opts).await?)
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

    pub async fn toggle_like_track(
        &self,
        track_id: String,
        is_liked: bool,
    ) -> color_eyre::Result<()> {
        if is_liked {
            let opts = RemoveLikedTracksOptions::new(self.user_id, vec![track_id]);
            self.client.remove_liked_tracks(&opts).await?;
        } else {
            let opts = AddLikedTracksOptions::new(self.user_id, vec![track_id]);
            self.client.add_liked_tracks(&opts).await?;
        }

        Ok(())
    }

    pub async fn add_like_track(&self, track_id: String) -> color_eyre::Result<()> {
        let opts = AddLikedTracksOptions::new(self.user_id, vec![track_id]);
        self.client.add_liked_tracks(&opts).await?;
        Ok(())
    }

    pub async fn remove_like_track(&self, track_id: String) -> color_eyre::Result<()> {
        let opts = RemoveLikedTracksOptions::new(self.user_id, vec![track_id]);
        self.client.remove_liked_tracks(&opts).await?;
        Ok(())
    }

    pub async fn toggle_dislike_track(
        &self,
        track_id: String,
        is_disliked: bool,
    ) -> color_eyre::Result<()> {
        if is_disliked {
            let opts = RemoveDislikedTracksOptions::new(self.user_id, vec![track_id]);
            self.client.remove_disliked_tracks(&opts).await?;
        } else {
            let opts = AddDislikedTracksOptions::new(self.user_id, vec![track_id]);
            self.client.add_disliked_tracks(&opts).await?;
        }

        Ok(())
    }

    pub async fn add_dislike_track(&self, track_id: String) -> color_eyre::Result<()> {
        let opts = AddDislikedTracksOptions::new(self.user_id, vec![track_id]);
        self.client.add_disliked_tracks(&opts).await?;
        Ok(())
    }

    pub async fn remove_dislike_track(&self, track_id: String) -> color_eyre::Result<()> {
        let opts = RemoveDislikedTracksOptions::new(self.user_id, vec![track_id]);
        self.client.remove_disliked_tracks(&opts).await?;
        Ok(())
    }

    pub async fn add_like_album(&self, album_id: u32) -> color_eyre::Result<()> {
        let opts = AddLikedAlbumOptions::new(self.user_id, album_id);
        self.client.add_liked_album(&opts).await?;
        Ok(())
    }

    pub async fn remove_like_album(&self, album_id: u32) -> color_eyre::Result<()> {
        let opts = RemoveLikedAlbumOptions::new(self.user_id, album_id);
        self.client.remove_liked_album(&opts).await?;
        Ok(())
    }

    pub async fn add_like_playlist(&self, owner_uid: u64, kind: u32) -> color_eyre::Result<()> {
        let opts = AddLikedPlaylistOptions::new(self.user_id, owner_uid, kind);
        self.client.add_liked_playlist(&opts).await?;
        Ok(())
    }

    pub async fn remove_like_playlist(&self, owner_uid: u64, kind: u32) -> color_eyre::Result<()> {
        let opts = RemoveLikedPlaylistOptions::new(self.user_id, owner_uid, kind);
        self.client.remove_liked_playlist(&opts).await?;
        Ok(())
    }

    pub async fn add_like_artist(&self, artist_id: String) -> color_eyre::Result<()> {
        let opts = AddLikedArtistOptions::new(self.user_id, artist_id);
        self.client.add_liked_artist(&opts).await?;
        Ok(())
    }

    pub async fn remove_like_artist(&self, artist_id: String) -> color_eyre::Result<()> {
        let opts = RemoveLikedArtistOptions::new(self.user_id, artist_id);
        self.client.remove_liked_artist(&opts).await?;
        Ok(())
    }

    pub async fn add_dislike_artist(&self, artist_id: String) -> color_eyre::Result<()> {
        let opts = AddDislikedArtistOptions::new(self.user_id, artist_id);
        self.client.add_disliked_artist(&opts).await?;
        Ok(())
    }

    pub async fn remove_dislike_artist(&self, artist_id: String) -> color_eyre::Result<()> {
        let opts = RemoveDislikedArtistOptions::new(self.user_id, artist_id);
        self.client.remove_disliked_artist(&opts).await?;
        Ok(())
    }

    pub async fn fetch_liked_collection(
        &self,
        revision: Option<u64>,
    ) -> color_eyre::Result<Collection> {
        let get_opt = || {
            let mut opt = CollectionSyncOption::new();
            if let Some(rev) = revision {
                opt = opt.revision(rev);
            }
            opt
        };
        let opts = CollectionSyncOptions::new()
            .liked_tracks(get_opt())
            .liked_albums(get_opt())
            .liked_artists(get_opt())
            .liked_playlists(get_opt());
        Ok(self.client.collection_sync(&opts).await?)
    }
}
