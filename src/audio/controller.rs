use flume::Sender;
use rodio::Source;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
};
use tokio::sync::Mutex;
use yandex_music::model::track::Track;

use crate::audio::{
    commands::AudioCommand,
    fx::{FxSource, analyzer::AudioAnalyzer, fade::Fade},
    playback::PlaybackEngine,
    progress::TrackProgress,
    state::PlaybackState,
    stream_manager::StreamManager,
};
use crate::event::events::Event;

pub struct AudioController {
    engine: Arc<PlaybackEngine>,
    stream_manager: Arc<StreamManager>,
    state: Arc<RwLock<PlaybackState>>,
    event_tx: Sender<Event>,
    pub track_progress: Arc<RwLock<Arc<TrackProgress>>>,
    current_playback_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub current_amplitude: Arc<AtomicU32>,
    volume: Arc<AtomicU8>,
    is_muted: Arc<AtomicBool>,
}

impl AudioController {
    pub fn new(
        engine: PlaybackEngine,
        stream_manager: Arc<StreamManager>,
        event_tx: Sender<Event>,
    ) -> Self {
        let controller = Self {
            engine: Arc::new(engine),
            stream_manager,
            state: Arc::new(RwLock::new(PlaybackState::Stopped)),
            event_tx,
            track_progress: Arc::new(RwLock::new(Arc::new(TrackProgress::default()))),
            current_playback_task: Arc::new(Mutex::new(None)),
            current_amplitude: Arc::new(AtomicU32::new(0)),
            volume: Arc::new(AtomicU8::new(100)),
            is_muted: Arc::new(AtomicBool::new(false)),
        };

        controller.start_monitor();
        controller
    }

    fn start_monitor(&self) {
        let engine = self.engine.clone();
        let progress = self.track_progress.clone();
        let state = self.state.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let is_playing = {
                    let state_guard = state.read().unwrap();
                    matches!(*state_guard, PlaybackState::Playing(_))
                };

                if is_playing {
                    let pos = engine.get_pos();
                    if let Ok(guard) = progress.read() {
                        guard.set_current_position(pos);
                    }

                    if engine.is_empty() {
                        let mut state_guard = state.write().unwrap();
                        *state_guard = PlaybackState::Stopped;
                        drop(state_guard);
                        let _ = event_tx.send(Event::TrackEnded);
                    }
                }
            }
        });
    }

    pub async fn handle_command(&self, cmd: AudioCommand) {
        match cmd {
            AudioCommand::PlayTrack(track) => self.play_track(track).await,
            AudioCommand::Pause => self.pause().await,
            AudioCommand::Resume => self.resume().await,
            AudioCommand::Stop => self.stop().await,
            AudioCommand::SetVolume(vol) => self.set_volume(vol),
            AudioCommand::Seek(pos) => self.seek(pos).await,
            _ => {}
        }
    }

    async fn play_track(&self, track: Track) {
        self.stop().await;

        {
            let mut state = self.state.write().unwrap();
            *state = PlaybackState::Buffering(track.clone());
        }

        let engine = self.engine.clone();
        let stream_manager = self.stream_manager.clone();
        let progress = self.track_progress.clone();
        let event_tx = self.event_tx.clone();
        let state = self.state.clone();
        let track_clone = track.clone();
        let amplitude = self.current_amplitude.clone();

        self.apply_volume();

        let task = tokio::spawn(async move {
            match stream_manager.create_stream_session(&track_clone).await {
                Ok((session, new_progress)) => {
                    if let Ok(mut guard) = progress.write() {
                        *guard = new_progress;
                    }

                    let mut source = FxSource::new(session.source);
                    source.add_effect(AudioAnalyzer::new(amplitude));

                    if let Some(fade) = track_clone.fade.clone() {
                        source.add_effect(Fade::new(
                            fade.in_start,
                            fade.in_stop,
                            fade.out_start,
                            fade.out_stop,
                            source.sample_rate().get(),
                            source.channels().get(),
                        ));
                    }

                    engine.play_source(source);

                    {
                        let mut state_guard = state.write().unwrap();
                        *state_guard = PlaybackState::Playing(track_clone.clone());
                    }

                    let _ = event_tx.send(Event::TrackStarted(track_clone, 0));
                }
                Err(_e) => {
                    let mut state_guard = state.write().unwrap();
                    *state_guard = PlaybackState::Error(_e.to_string());
                    let _ = event_tx.send(Event::TrackEnded);
                }
            }
        });

        let mut task_guard = self.current_playback_task.lock().await;
        *task_guard = Some(task);
    }

    async fn stop(&self) {
        let mut task_guard = self.current_playback_task.lock().await;
        if let Some(task) = task_guard.take() {
            task.abort();
        }
        self.engine.stop();
        if let Ok(progress) = self.track_progress.read() {
            progress.reset();
        }
        let mut state = self.state.write().unwrap();
        *state = PlaybackState::Stopped;
    }

    async fn pause(&self) {
        self.engine.pause();
        let mut state = self.state.write().unwrap();
        if let PlaybackState::Playing(track) = &*state {
            *state = PlaybackState::Paused(track.clone());
        }
    }

    async fn resume(&self) {
        self.engine.play();
        let mut state = self.state.write().unwrap();
        if let PlaybackState::Paused(track) = &*state {
            *state = PlaybackState::Playing(track.clone());
        }
    }

    async fn seek(&self, pos: std::time::Duration) {
        let _ = self.engine.try_seek(pos);
        if let Ok(progress) = self.track_progress.read() {
            progress.set_current_position(pos);
        }
    }

    pub fn current_amplitude(&self) -> f32 {
        f32::from_bits(self.current_amplitude.load(Ordering::Relaxed))
    }

    pub fn is_playing(&self) -> bool {
        let state = self.state.read().unwrap();
        matches!(*state, PlaybackState::Playing(_))
    }

    pub fn current_track(&self) -> Option<Track> {
        let state = self.state.read().unwrap();
        match &*state {
            PlaybackState::Playing(t) | PlaybackState::Paused(t) | PlaybackState::Buffering(t) => {
                Some(t.clone())
            }
            _ => None,
        }
    }

    pub fn volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    fn set_volume(&self, volume: f32) {
        let vol_u8 = (volume * 100.0) as u8;
        self.volume.store(vol_u8, Ordering::Relaxed);
        self.is_muted.store(false, Ordering::Relaxed);
        self.apply_volume();
    }

    pub fn set_volume_u8(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
        self.is_muted.store(false, Ordering::Relaxed);
        self.apply_volume();
    }

    pub fn volume_up(&self, amount: u8) {
        let current = self.volume.load(Ordering::Relaxed);
        self.set_volume_u8(current.saturating_add(amount));
    }

    pub fn volume_down(&self, amount: u8) {
        let current = self.volume.load(Ordering::Relaxed);
        self.set_volume_u8(current.saturating_sub(amount));
    }

    pub fn toggle_mute(&self) {
        let muted = self.is_muted.load(Ordering::Relaxed);
        self.is_muted.store(!muted, Ordering::Relaxed);
        self.apply_volume();
    }

    fn apply_volume(&self) {
        let muted = self.is_muted.load(Ordering::Relaxed);
        let volume = if muted {
            0.0
        } else {
            self.volume.load(Ordering::Relaxed) as f32 / 100.0
        };
        self.engine.set_volume(volume);
    }
}
