use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    audio::{
        fx::{AudioAnalyzer, FxSource},
        util::{construct_sink, setup_device_config},
    },
    event::events::Event,
    http::ApiService,
    stream,
};
use flume::Sender;
use rodio::{OutputStream, Sink, cpal::StreamConfig};
use yandex_music::model::track::Track;

use super::progress::TrackProgress;
use reqwest::blocking::Client;

#[allow(dead_code)]
pub struct AudioPlayer {
    api: Arc<ApiService>,
    http_client: Client,
    stream: OutputStream,
    sink: Arc<Sink>,
    stream_config: StreamConfig,
    event_tx: Sender<Event>,

    pub current_track: Option<Track>,
    pub volume: u8,
    pub is_muted: bool,

    pub track_progress: Arc<TrackProgress>,
    pub is_ready: Arc<AtomicBool>,
    pub is_playing: Arc<AtomicBool>,
    pub current_playback_task: Option<tokio::task::JoinHandle<()>>,
    pub playback_generation: Arc<AtomicU64>,
    stream_controller: Arc<Mutex<Option<stream::StreamController>>>,
    playback_offset_millis: Arc<AtomicI64>,
    pub current_amplitude: Arc<AtomicU32>,
}

impl AudioPlayer {
    pub async fn new(
        event_tx: flume::Sender<Event>,
        api: Arc<ApiService>,
    ) -> color_eyre::Result<Self> {
        let (device, stream_config, sample_format) = setup_device_config();
        let (stream, sink) = construct_sink(device, &stream_config, sample_format)?;

        let http_client = Client::builder()
            .build()
            .expect("failed to create http client");

        let player = Self {
            api,
            http_client,
            stream,
            sink: Arc::new(sink),
            stream_config,

            event_tx,

            current_track: None,
            volume: 100,
            is_muted: false,

            track_progress: Arc::new(TrackProgress::default()),
            is_ready: Arc::new(AtomicBool::new(false)),
            is_playing: Arc::new(AtomicBool::new(false)),
            current_playback_task: None,
            playback_generation: Arc::new(AtomicU64::new(0)),
            stream_controller: Arc::new(Mutex::new(None)),
            playback_offset_millis: Arc::new(AtomicI64::new(0)),
            current_amplitude: Arc::new(AtomicU32::new(0)),
        };

        let progress = player.track_progress.clone();
        let sink = player.sink.clone();
        let event_tx = player.event_tx.clone();
        let playing = player.is_playing.clone();
        let offset = player.playback_offset_millis.clone();

        thread::spawn(move || {
            loop {
                let sink_pos = sink.get_pos();
                let sink_ms = sink_pos.as_millis() as i64;
                let base_ms = offset.load(Ordering::Relaxed);
                let current_ms = sink_ms.saturating_add(base_ms).max(0) as u64;
                progress.set_current_position(Duration::from_millis(current_ms));
                let is_playing = playing.load(Ordering::Relaxed);

                if is_playing && sink.empty() {
                    playing.store(false, Ordering::Relaxed);
                    let _ = event_tx.send(Event::TrackEnded);
                }

                thread::sleep(Duration::from_millis(1000 / 8));
            }
        });

        Ok(player)
    }

    pub async fn play_track(&mut self, track: Track) {
        self.stop_track();

        if let Some(task) = &self.current_playback_task {
            task.abort();
        }

        self.is_ready.store(false, Ordering::Relaxed);

        self.playback_offset_millis.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.stream_controller.lock() {
            if let Some(controller) = guard.take() {
                controller.stop();
            }
        }

        let generation = self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let playback_generation = self.playback_generation.clone();
        let api = self.api.clone();
        let http_client = self.http_client.clone();
        let sink = self.sink.clone();
        let track_progress = self.track_progress.clone();
        let ready = self.is_ready.clone();
        let playing = self.is_playing.clone();
        let event_tx = self.event_tx.clone();
        let track_clone = track.clone();
        let playback_offset = self.playback_offset_millis.clone();
        let stream_controller = Arc::clone(&self.stream_controller);
        let amplitude = self.current_amplitude.clone();

        self.current_playback_task = Some(tokio::task::spawn(async move {
            let (url, codec, bitrate) = api.fetch_track_url(track_clone.id.clone()).await.unwrap();
            track_progress.set_bitrate(bitrate.try_into().unwrap());

            let progress = track_progress.clone();
            let codec_clone = codec.clone();
            let session = tokio::task::spawn_blocking(move || {
                stream::create_streaming_session(http_client, url, codec_clone, bitrate, progress)
            })
            .await
            .expect("stream session task panicked");

            let session = match session {
                Ok(session) => session,
                Err(err) => {
                    eprintln!("failed to start streaming session: {err}");
                    return;
                }
            };

            if playback_generation.load(Ordering::SeqCst) != generation {
                session.controller.stop();
                return;
            }

            if let Ok(mut guard) = stream_controller.lock() {
                *guard = Some(session.controller.clone());
            }

            playback_offset.store(0, Ordering::Relaxed);

            let _ = event_tx.send(Event::TrackStarted(track_clone.clone(), 0));

            let mut source = FxSource::new(session.source);
            source.add_effect(AudioAnalyzer::new(amplitude));
            sink.append(source);
            ready.store(true, Ordering::Relaxed);
            playing.store(true, Ordering::Relaxed);
        }));

        self.current_track = Some(track);
    }

    pub fn stop_track(&mut self) {
        self.is_ready.store(false, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        self.track_progress.reset();
        if let Ok(mut guard) = self.stream_controller.lock() {
            if let Some(controller) = guard.take() {
                controller.stop();
            }
        }
        self.sink.stop();
        if let Some(task) = &self.current_playback_task {
            task.abort();
        }
        self.current_playback_task = None;
        self.current_track = None;
    }

    pub fn play_pause(&mut self) {
        let is_paused = self.sink.is_paused();
        if is_paused {
            self.sink.play();
        } else {
            self.sink.pause();
            let sink_ms = self.sink.get_pos().as_millis() as i64;
            let base_ms = self.playback_offset_millis.load(Ordering::Relaxed);
            let current_ms = sink_ms.saturating_add(base_ms).max(0) as u64;
            self.track_progress
                .set_current_position(Duration::from_millis(current_ms));
        }
        self.is_playing.store(is_paused, Ordering::Relaxed);
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.is_muted = false;
        self.volume = volume.min(200);
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn volume_up(&mut self, volume: u8) {
        self.is_muted = false;
        self.volume = (self.volume.saturating_add(volume)).min(200);
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn volume_down(&mut self, volume: u8) {
        self.is_muted = false;
        self.volume = self.volume.saturating_sub(volume);
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn seek_backwards(&mut self, seconds: u64) {
        if !self.is_ready.load(Ordering::Relaxed) {
            return;
        }

        let (current_ms, _) = self.track_progress.get_progress();
        let delta = seconds.saturating_mul(1000);
        let target_ms = current_ms.saturating_sub(delta);
        self.perform_seek(Duration::from_millis(target_ms));
    }

    pub fn seek_forwards(&mut self, seconds: u64) {
        if !self.is_ready.load(Ordering::Relaxed) {
            return;
        }

        let (current_ms, total_ms) = self.track_progress.get_progress();
        let delta = seconds.saturating_mul(1000);
        let mut target_ms = current_ms.saturating_add(delta);
        if total_ms > 0 {
            target_ms = target_ms.min(total_ms);
        }
        self.perform_seek(Duration::from_millis(target_ms));
    }

    fn perform_seek(&mut self, target: Duration) {
        self.sink.pause();
        if self.sink.try_seek(target).is_ok() {
            let target_ms = target.as_millis() as i64;
            let sink_ms = self.sink.get_pos().as_millis() as i64;
            self.playback_offset_millis
                .store(target_ms - sink_ms, Ordering::Relaxed);
            self.track_progress.set_current_position(target);
        }
        self.sink.play();
    }

    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
        self.sink.set_volume(if self.is_muted {
            0.0
        } else {
            self.volume as f32 / 100.0
        });
    }
}
