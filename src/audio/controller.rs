use flume::Sender;
use rodio::Source;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Mutex;
use yandex_music::model::track::Track;

use crate::audio::{
    commands::AudioCommand,
    fx::{
        EffectHandle, FxSource,
        modules::{FadeEffect, MonitorEffect},
        param::EffectParams,
    },
    playback::PlaybackEngine,
    progress::TrackProgress,
    signals::AudioSignals,
    stream_manager::StreamManager,
};
use crate::event::events::Event;

pub struct AudioController {
    engine: Arc<PlaybackEngine>,
    stream_manager: Arc<StreamManager>,
    event_tx: Sender<Event>,
    pub track_progress: Arc<RwLock<Arc<TrackProgress>>>,
    current_playback_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    signals: AudioSignals,
    effect_handles: Arc<RwLock<HashMap<String, EffectHandle>>>,
}

impl AudioController {
    pub fn new(
        engine: PlaybackEngine,
        stream_manager: Arc<StreamManager>,
        event_tx: Sender<Event>,
        signals: AudioSignals,
    ) -> Self {
        let controller = Self {
            engine: Arc::new(engine),
            stream_manager,
            event_tx,
            track_progress: Arc::new(RwLock::new(Arc::new(TrackProgress::default()))),
            current_playback_task: Arc::new(Mutex::new(None)),
            signals,
            effect_handles: Arc::new(RwLock::new(HashMap::new())),
        };

        controller.start_monitor();
        controller
    }

    pub fn signals(&self) -> AudioSignals {
        self.signals.clone()
    }

    fn start_monitor(&self) {
        let engine = self.engine.clone();
        let progress = self.track_progress.clone();
        let signals = self.signals.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(125)).await;

                let is_playing = signals.is_playing.get();

                if is_playing {
                    if engine.is_empty() {
                        signals.set_playing(false);
                        signals.is_stopped.set(true);
                        let _ = event_tx.send(Event::TrackEnded);
                        continue;
                    }

                    if signals.monitor.is_focused() {
                        let pos = engine.pos();
                        let dur = signals.duration_ms.get();

                        signals.update_progress(pos.as_millis() as u64, dur);

                        if let Ok(guard) = progress.read() {
                            guard.set_current_position(pos);
                            let buffered = guard.get_buffered_ratio() as f32;
                            signals.update_buffered_ratio(buffered);
                        }

                        signals.amplitude.set(signals.monitor.combined_amplitude());
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

        self.signals.is_buffering.set(true);
        self.signals.is_stopped.set(false);
        self.signals.set_current_track(Some(track.clone()));

        let engine = self.engine.clone();
        let stream_manager = self.stream_manager.clone();
        let progress = self.track_progress.clone();
        let event_tx = self.event_tx.clone();
        let signals = self.signals.clone();
        let track_clone = track.clone();
        let monitor = self.signals.monitor.clone();
        let effect_handles_store = self.effect_handles.clone();

        self.apply_volume();

        let task = tokio::spawn(async move {
            match stream_manager.create_stream_session(&track_clone).await {
                Ok((session, new_progress)) => {
                    if let Ok(mut guard) = progress.write() {
                        *guard = new_progress;
                    }

                    let mut source = FxSource::new(session.source);

                    let monitor_params = Arc::new(EffectParams::new(&[]));
                    monitor_params.set_enabled(true);
                    source.add_effect(
                        "monitor",
                        "Audio Monitor",
                        Box::new(MonitorEffect::new(monitor)),
                        monitor_params,
                    );

                    if let Some(fade) = track_clone.fade.clone() {
                        let fade_params = Arc::new(EffectParams::new(&[]));
                        fade_params.set_enabled(true);
                        source.add_effect(
                            "fade",
                            "Fade",
                            Box::new(FadeEffect::new(
                                fade.in_start,
                                fade.in_stop,
                                fade.out_start,
                                fade.out_stop,
                                source.sample_rate().get(),
                                source.channels().get(),
                            )),
                            fade_params,
                        );
                    }

                    crate::audio::fx::init::init_all(&mut source);

                    if let Ok(old_store) = effect_handles_store.read() {
                        let new_handles = source.get_effect_handles();
                        for (name, new_handle) in new_handles.iter() {
                            if let Some(old_handle) = old_store.get(name) {
                                new_handle.set_enabled(old_handle.is_enabled());
                                for i in 0..old_handle.param_count().min(new_handle.param_count()) {
                                    new_handle.set_param(i, old_handle.get_param(i));
                                }
                            }
                        }
                    }

                    let handles = source.get_effect_handles();
                    if let Ok(mut store) = effect_handles_store.write() {
                        *store = handles;
                    }

                    engine.play_source(source);

                    signals.is_buffering.set(false);
                    signals.set_playing(true);

                    let _ = event_tx.send(Event::TrackStarted(track_clone, 0));
                }
                Err(_e) => {
                    signals.is_buffering.set(false);
                    signals.set_playing(false);
                    signals.is_stopped.set(true);
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

        self.signals.set_playing(false);
        self.signals.is_stopped.set(true);
        self.signals.is_buffering.set(false);
        self.signals.update_progress(0, 0);
        self.signals.update_buffered_ratio(0.0);
    }

    async fn pause(&self) {
        self.engine.pause();
        self.signals.set_playing(false);
    }

    async fn resume(&self) {
        self.engine.play();
        self.signals.set_playing(true);
    }

    async fn seek(&self, pos: std::time::Duration) {
        let _ = self.engine.try_seek(pos);
        if let Ok(progress) = self.track_progress.read() {
            progress.set_current_position(pos);
        }
    }

    pub fn get_effect_handles(&self) -> Arc<RwLock<HashMap<String, EffectHandle>>> {
        self.effect_handles.clone()
    }

    pub fn set_effect_handles(&self, handles: HashMap<String, EffectHandle>) {
        if let Ok(mut guard) = self.effect_handles.write() {
            *guard = handles;
        }
    }

    pub fn toggle_effect(&self, name: &str) -> bool {
        if let Ok(guard) = self.effect_handles.read()
            && let Some(handle) = guard.get(name)
        {
            let enabled = handle.is_enabled();
            handle.set_enabled(!enabled);
            return true;
        }
        false
    }

    pub fn is_effect_enabled(&self, name: &str) -> Option<bool> {
        if let Ok(guard) = self.effect_handles.read() {
            guard.get(name).map(|h| h.is_enabled())
        } else {
            None
        }
    }

    pub fn update_progress(&self, pos: Duration) {
        let dur = self.signals.duration_ms.get();
        self.signals.update_progress(pos.as_millis() as u64, dur);
    }

    pub fn current_amplitude(&self) -> f32 {
        self.signals.monitor.combined_amplitude()
    }

    pub fn is_playing(&self) -> bool {
        self.signals.is_playing.get()
    }

    pub fn current_track(&self) -> Option<Track> {
        self.signals.current_track.get()
    }

    pub fn current_track_id(&self) -> Option<String> {
        self.signals.current_track_id.get()
    }

    pub fn volume(&self) -> u8 {
        self.signals.volume.get()
    }

    pub fn is_muted(&self) -> bool {
        self.signals.is_muted.get()
    }

    fn set_volume(&self, volume: f32) {
        let vol_u8 = (volume * 100.0) as u8;
        self.signals.set_volume(vol_u8, false);
        self.apply_volume();
    }

    pub fn set_volume_u8(&self, volume: u8) {
        self.signals.set_volume(volume.min(100), false);
        self.apply_volume();
    }

    pub fn volume_up(&self, amount: u8) {
        let current = self.signals.volume.get();
        self.set_volume_u8(current.saturating_add(amount));
    }

    pub fn volume_down(&self, amount: u8) {
        let current = self.signals.volume.get();
        self.set_volume_u8(current.saturating_sub(amount));
    }

    pub fn toggle_mute(&self) {
        let muted = self.signals.is_muted.get();
        let vol = self.signals.volume.get();
        self.signals.set_volume(vol, !muted);
        self.apply_volume();
    }

    fn apply_volume(&self) {
        let muted = self.signals.is_muted.get();
        let volume = if muted {
            0.0
        } else {
            self.signals.volume.get() as f32 / 100.0
        };
        self.engine.set_volume(volume);
    }
}
