use crate::{
    audio::{
        commands::AudioCommand,
        controller::AudioController,
        enums::RepeatMode,
        playback::PlaybackEngine,
        progress::TrackProgress,
        queue::{PlaybackContext, QueueManager},
        stream_manager::StreamManager,
    },
    event::events::Event,
    http::ApiService,
};
use flume::Sender;
use std::sync::Arc;
use yandex_music::model::{
    playlist::{Playlist, PlaylistTracks},
    track::Track,
};

pub struct AudioSystem {
    controller: AudioController,
    queue: QueueManager,
    event_tx: Sender<Event>,
    api: Arc<ApiService>,
}

impl AudioSystem {
    pub async fn new(event_tx: Sender<Event>, api: Arc<ApiService>) -> color_eyre::Result<Self> {
        let engine = PlaybackEngine::new()?;
        let stream_manager = StreamManager::new(api.clone());
        let controller = AudioController::new(engine, stream_manager, event_tx.clone());
        let queue = QueueManager::new(api.clone());

        Ok(Self {
            controller,
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
            Some(PlaylistTracks::Partial(tracks)) => {
                let track_ids: Vec<String> = tracks
                    .iter()
                    .map(|p| {
                        if let Some(album_id) = p.album_id {
                            format!("{}:{}", p.id, album_id)
                        } else {
                            p.id.clone()
                        }
                    })
                    .collect();
                self.api.fetch_tracks_by_ids(track_ids).await?
            }
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
            Some(PlaylistTracks::Partial(tracks)) => {
                let track_ids: Vec<String> = tracks
                    .iter()
                    .map(|p| {
                        if let Some(album_id) = p.album_id {
                            format!("{}:{}", p.id, album_id)
                        } else {
                            p.id.clone()
                        }
                    })
                    .collect();
                self.api.fetch_tracks_by_ids(track_ids).await?
            }
            None => vec![],
        };

        if let Some(track) = self
            .queue
            .load(PlaybackContext::Playlist(playlist), tracks, 0)
            .await
        {
            self.controller
                .handle_command(AudioCommand::PlayTrack(track))
                .await;
        }

        Ok(())
    }

    pub async fn load_context(
        &mut self,
        context: PlaybackContext,
        tracks: Vec<Track>,
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
        if let Some(track) = self.queue.load(PlaybackContext::List, tracks, 0).await {
            self.controller
                .handle_command(AudioCommand::PlayTrack(track))
                .await;
        }
    }

    pub async fn play_single_track(&mut self, track: Track) {
        if let Some(track) = self
            .queue
            .load(PlaybackContext::Track, vec![track], 0)
            .await
        {
            self.controller
                .handle_command(AudioCommand::PlayTrack(track))
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

    pub async fn play_pause(&mut self) {
        if self.controller.is_playing() {
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
        let (current_ms, _) = self.controller.track_progress.get_progress();
        let delta_ms = seconds * 1000;
        let new_pos_ms = current_ms.saturating_sub(delta_ms);
        self.controller
            .handle_command(AudioCommand::Seek(std::time::Duration::from_millis(
                new_pos_ms,
            )))
            .await;
    }

    pub async fn seek_forwards(&mut self, seconds: u64) {
        let (current_ms, total_ms) = self.controller.track_progress.get_progress();
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
        self.controller.current_track()
    }

    pub fn is_playing(&self) -> bool {
        self.controller.is_playing()
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.queue.repeat_mode
    }

    pub fn is_shuffled(&self) -> bool {
        self.queue.is_shuffled
    }

    pub fn volume(&self) -> u8 {
        self.controller.volume()
    }

    pub fn is_muted(&self) -> bool {
        self.controller.is_muted()
    }

    pub fn track_progress(&self) -> &Arc<TrackProgress> {
        &self.controller.track_progress
    }

    pub fn queue(&self) -> &Vec<Track> {
        &self.queue.queue
    }

    pub fn history(&self) -> &Vec<Track> {
        &self.queue.history
    }

    pub fn current_track_index(&self) -> usize {
        self.queue.current_track_index
    }

    pub fn current_amplitude(&self) -> f32 {
        self.controller.current_amplitude()
    }
}
