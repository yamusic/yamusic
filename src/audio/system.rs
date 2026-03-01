use crate::{
    audio::{
        commands::AudioCommand,
        controller::AudioController,
        enums::RepeatMode,
        playback::PlaybackEngine,
        progress::TrackProgress,
        queue::{PlaybackContext, QueueManager},
        signals::AudioSignals,
        state::SystemState,
        stream_manager::StreamManager,
    },
    event::events::Event,
    http::ApiService,
};
use flume::Sender;
use im::Vector;
use std::sync::Arc;
use tokio::sync::RwLock;
use yandex_music::model::track::Track;

pub struct AudioSystem {
    controller: AudioController,
    queue: QueueManager,
    event_tx: Sender<Event>,
    #[allow(dead_code)]
    api: Arc<ApiService>,
    state: Arc<RwLock<SystemState>>,
    signals: AudioSignals,
}

use crate::audio::cache::UrlCache;

impl AudioSystem {
    pub async fn new(event_tx: Sender<Event>, api: Arc<ApiService>) -> color_eyre::Result<Self> {
        let engine = PlaybackEngine::new()?;
        let url_cache = UrlCache::new();
        let stream_manager = Arc::new(
            tokio::task::spawn_blocking({
                let api = api.clone();
                let url_cache = url_cache.clone();
                move || StreamManager::new(api, url_cache)
            })
            .await?,
        );

        let signals = AudioSignals::new();

        let controller = AudioController::new(
            engine,
            stream_manager.clone(),
            event_tx.clone(),
            signals.clone(),
        );

        let mut queue = QueueManager::new(
            api.clone(),
            url_cache,
            stream_manager.clone(),
            signals.clone(),
        );
        queue.set_event_tx(event_tx.clone());
        let state = Arc::new(RwLock::new(SystemState::default()));

        Ok(Self {
            controller,
            queue,
            event_tx,
            api,
            state,
            signals,
        })
    }

    pub fn signals(&self) -> &AudioSignals {
        &self.signals
    }

    pub async fn load_context(
        &mut self,
        context: PlaybackContext,
        tracks: Vector<Track>,
        index: usize,
    ) -> Option<Track> {
        let track = self.queue.load(context, tracks, index).await;
        if let Some(t) = &track {
            self.controller
                .handle_command(AudioCommand::PlayTrack(t.clone()))
                .await;
        }
        track
    }

    pub async fn load_tracks(&mut self, tracks: Vec<Track>) {
        if let Some(track) = self
            .queue
            .load(PlaybackContext::Standalone, Vector::from(tracks), 0)
            .await
        {
            self.controller
                .handle_command(AudioCommand::PlayTrack(track))
                .await;
        }
    }

    pub async fn play_single_track(&mut self, track: Track) {
        if let Some(playing_track) = self
            .queue
            .load(
                PlaybackContext::Track(track.clone()),
                Vector::from(vec![track]),
                0,
            )
            .await
        {
            self.controller
                .handle_command(AudioCommand::PlayTrack(playing_track))
                .await;
        }
    }

    pub async fn play_track_at_index(&mut self, index: usize) {
        if let Some(track) = self.queue.play_track_at_index(index).await {
            self.controller
                .handle_command(AudioCommand::PlayTrack(track))
                .await;
        }
    }

    pub async fn on_track_ended(&mut self) {
        if let Some(next_track) = self.queue.get_next_track().await {
            self.controller
                .handle_command(AudioCommand::PlayTrack(next_track))
                .await;
        } else {
            let _ = self.event_tx.send(Event::QueueEnded);
        }
    }

    pub async fn play_next(&mut self) {
        if let Some(next_track) = self.queue.get_next_track().await {
            self.controller
                .handle_command(AudioCommand::PlayTrack(next_track))
                .await;
        }
    }

    pub async fn play_previous(&mut self) {
        if let Some(prev_track) = self.queue.get_previous_track() {
            self.controller
                .handle_command(AudioCommand::PlayTrack(prev_track))
                .await;
        }
    }

    pub fn queue_track(&mut self, track: Track) {
        self.queue.queue_track(track);
    }

    pub fn play_track_next(&mut self, track: Track) {
        self.queue.play_next(track);
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        self.queue.remove_track(index);
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    pub async fn play_pause(&mut self) {
        if self.signals.is_playing.get() {
            self.controller.handle_command(AudioCommand::Pause).await;
        } else {
            self.controller.handle_command(AudioCommand::Resume).await;
        }
    }

    pub async fn stop(&mut self) {
        self.controller.handle_command(AudioCommand::Stop).await;
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.controller.set_volume_u8(volume);
    }

    pub fn volume_up(&mut self, volume: u8) {
        self.controller.volume_up(volume);
    }

    pub fn volume_down(&mut self, volume: u8) {
        self.controller.volume_down(volume);
    }

    pub async fn seek_backwards(&mut self, seconds: u64) {
        let current_ms = self.signals.position_ms.get();
        let delta_ms = seconds * 1000;
        let new_pos_ms = current_ms.saturating_sub(delta_ms);
        self.controller
            .handle_command(AudioCommand::Seek(std::time::Duration::from_millis(
                new_pos_ms,
            )))
            .await;
    }

    pub async fn seek_forwards(&mut self, seconds: u64) {
        let current_ms = self.signals.position_ms.get();
        let total_ms = self.signals.duration_ms.get();
        let delta_ms = seconds * 1000;
        let mut new_pos_ms = current_ms.saturating_add(delta_ms);
        if total_ms > 0 {
            new_pos_ms = new_pos_ms.min(total_ms);
        }
        self.controller
            .handle_command(AudioCommand::Seek(std::time::Duration::from_millis(
                new_pos_ms,
            )))
            .await;
    }

    pub fn toggle_mute(&mut self) {
        self.controller.toggle_mute();
    }

    pub fn toggle_repeat_mode(&mut self) {
        self.queue.toggle_repeat_mode();
    }

    pub fn toggle_shuffle(&mut self) {
        self.queue.toggle_shuffle();
    }

    pub fn current_track(&self) -> Option<Track> {
        self.signals.current_track.get()
    }

    pub fn current_track_id(&self) -> Option<String> {
        self.signals.current_track_id.get()
    }

    pub fn is_playing(&self) -> bool {
        self.signals.is_playing.get()
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.signals.repeat_mode.get()
    }

    pub fn is_shuffled(&self) -> bool {
        self.signals.is_shuffled.get()
    }

    pub fn volume(&self) -> u8 {
        self.signals.volume.get()
    }

    pub fn is_muted(&self) -> bool {
        self.signals.is_muted.get()
    }

    pub fn track_progress(&self) -> Arc<TrackProgress> {
        self.controller.track_progress.read().unwrap().clone()
    }

    pub fn queue(&self) -> Vector<Track> {
        self.signals.queue.with(|q| q.clone())
    }

    pub fn history(&self) -> Vector<Track> {
        self.signals.history.with(|h| h.clone())
    }

    pub fn current_track_index(&self) -> usize {
        self.signals.queue_index.get()
    }

    pub fn current_amplitude(&self) -> f32 {
        self.signals.amplitude.get()
    }

    pub async fn sync_queue(&mut self) {
        self.queue.poll_fetch().await;
    }

    pub fn maybe_trigger_fetch(&mut self, cursor_index: usize) {
        let queue_len = self.signals.queue.with(|q: &Vector<Track>| q.len());
        if queue_len > 0 && cursor_index + 2 >= queue_len {
            self.queue.trigger_fetch_if_needed();
        }
    }

    pub fn state_handle(&self) -> Arc<RwLock<SystemState>> {
        self.state.clone()
    }

    pub async fn sync_liked_collection_with(api: Arc<ApiService>, state: Arc<RwLock<SystemState>>) {
        let revision = {
            let state = state.read().await;
            state.liked.revision
        };

        if let Ok(liked_collection) = api.fetch_liked_collection(revision).await {
            let mut state = state.write().await;
            state.liked.apply_collection(liked_collection);
        }
    }

    pub async fn sync_liked_collection(&mut self) {
        Self::sync_liked_collection_with(self.api.clone(), self.state.clone()).await;
    }

    pub async fn is_liked(&self, track_id: &str) -> bool {
        let state = self.state.read().await;
        state.liked.is_liked(track_id)
    }

    pub async fn is_disliked(&self, track_id: &str) -> bool {
        let state = self.state.read().await;
        state.liked.is_disliked(track_id)
    }

    pub async fn is_album_liked(&self, album_id: u32) -> bool {
        let state = self.state.read().await;
        state.liked.is_album_liked(album_id)
    }

    pub async fn is_artist_liked(&self, artist_id: &str) -> bool {
        let state = self.state.read().await;
        state.liked.is_artist_liked(artist_id)
    }

    pub async fn is_artist_disliked(&self, artist_id: &str) -> bool {
        let state = self.state.read().await;
        state.liked.is_artist_disliked(artist_id)
    }

    pub async fn is_playlist_liked(&self, uid: u64, kind: u32) -> bool {
        let state = self.state.read().await;
        state.liked.is_playlist_liked(uid, kind)
    }
}
