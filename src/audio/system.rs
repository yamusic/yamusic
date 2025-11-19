use crate::{
    audio::{
        engine::AudioPlayer,
        enums::RepeatMode,
        progress::TrackProgress,
        queue::{PlaybackContext, QueueManager},
    },
    event::events::Event,
    http::ApiService,
};
use flume::Sender;
use std::sync::{Arc, atomic::Ordering};
use yandex_music::model::{
    playlist::{Playlist, PlaylistTracks},
    track::Track,
};

pub struct AudioSystem {
    player: AudioPlayer,
    queue: QueueManager,
    event_tx: Sender<Event>,
    api: Arc<ApiService>,
}

impl AudioSystem {
    pub async fn new(event_tx: Sender<Event>, api: Arc<ApiService>) -> color_eyre::Result<Self> {
        let player = AudioPlayer::new(event_tx.clone(), api.clone()).await?;
        let queue = QueueManager::new(api.clone());

        Ok(Self {
            player,
            queue,
            event_tx,
            api,
        })
    }

    pub async fn init(&mut self) -> color_eyre::Result<()> {
        let playlist = self.api.fetch_liked_tracks().await?;
        let tracks = match &playlist.tracks {
            Some(PlaylistTracks::Full(tracks)) => tracks.clone(),
            Some(PlaylistTracks::WithInfo(tracks)) => {
                tracks.iter().map(|t| t.track.clone()).collect()
            }
            Some(PlaylistTracks::Partial(tracks)) => self.api.fetch_tracks_partial(tracks).await?,
            None => vec![],
        };
        let context = PlaybackContext::Playlist(playlist);

        if !tracks.is_empty() {
            self.queue.load(context, tracks, 0).await;
        }

        Ok(())
    }

    pub async fn play_playlist(&mut self, playlist: Playlist) -> color_eyre::Result<()> {
        let tracks = match &playlist.tracks {
            Some(PlaylistTracks::Full(tracks)) => tracks.clone(),
            Some(PlaylistTracks::WithInfo(tracks)) => {
                tracks.iter().map(|t| t.track.clone()).collect()
            }
            Some(PlaylistTracks::Partial(tracks)) => self.api.fetch_tracks_partial(tracks).await?,
            None => vec![],
        };

        if let Some(track) = self
            .queue
            .load(PlaybackContext::Playlist(playlist), tracks, 0)
            .await
        {
            self.player.play_track(track).await;
        }

        Ok(())
    }

    pub async fn play_single_track(&mut self, track: Track) {
        if let Some(track) = self
            .queue
            .load(PlaybackContext::Track, vec![track], 0)
            .await
        {
            self.player.play_track(track).await;
        }
    }

    pub async fn play_track_at_index(&mut self, index: usize) {
        if let Some(track) = self.queue.play_track_at_index(index).await {
            let _ = self.player.play_track(track).await;
        }
    }

    pub async fn on_track_ended(&mut self) {
        if let Some(next_track) = self.queue.get_next_track().await {
            let _ = self.player.play_track(next_track).await;
        } else {
            let _ = self.event_tx.send(Event::TrackEnded);
        }
    }

    pub async fn play_next(&mut self) {
        if let Some(next_track) = self.queue.get_next_track().await {
            let _ = self.player.play_track(next_track).await;
        }
    }

    pub async fn play_previous(&mut self) {
        if let Some(prev_track) = self.queue.get_previous_track() {
            let _ = self.player.play_track(prev_track).await;
        }
    }

    pub fn queue_track(&mut self, track: Track) {
        self.queue.queue_track(track);
    }

    pub fn play_track_next(&mut self, track: Track) {
        self.queue.play_next(track);
    }

    pub fn play_pause(&mut self) {
        self.player.play_pause();
    }

    pub fn stop(&mut self) {
        self.player.stop_track();
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.player.set_volume(volume);
    }

    pub fn volume_up(&mut self, volume: u8) {
        self.player.volume_up(volume);
    }

    pub fn volume_down(&mut self, volume: u8) {
        self.player.volume_down(volume);
    }

    pub fn seek_backwards(&mut self, seconds: u64) {
        self.player.seek_backwards(seconds);
    }

    pub fn seek_forwards(&mut self, seconds: u64) {
        self.player.seek_forwards(seconds);
    }

    pub fn toggle_mute(&mut self) {
        self.player.toggle_mute();
    }

    pub fn toggle_repeat_mode(&mut self) {
        self.queue.toggle_repeat_mode();
    }

    pub fn toggle_shuffle(&mut self) {
        self.queue.toggle_shuffle();
    }

    pub fn current_track(&self) -> &Option<Track> {
        &self.player.current_track
    }

    pub fn is_playing(&self) -> bool {
        self.player.is_playing.load(Ordering::Relaxed)
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.queue.repeat_mode
    }

    pub fn is_shuffled(&self) -> bool {
        self.queue.is_shuffled
    }

    pub fn volume(&self) -> u8 {
        self.player.volume
    }

    pub fn is_muted(&self) -> bool {
        self.player.is_muted
    }

    pub fn track_progress(&self) -> &Arc<TrackProgress> {
        &self.player.track_progress
    }
}
