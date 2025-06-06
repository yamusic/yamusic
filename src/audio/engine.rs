use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    audio::util::{construct_sink, setup_device_config},
    event::events::Event,
    http::ApiService,
    stream::streamer::AudioStreamer,
};
use flume::Sender;
use rodio::{Decoder, OutputStream, Sink, Source, cpal::StreamConfig};
use tracing::info;
use yandex_music::model::track_model::track::Track;

use super::progress::TrackProgress;

#[allow(dead_code)]
pub struct AudioPlayer {
    api: Arc<ApiService>,
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
}

impl AudioPlayer {
    pub async fn new(
        event_tx: flume::Sender<Event>,
        api: Arc<ApiService>,
    ) -> color_eyre::Result<Self> {
        let (device, stream_config, sample_format) = setup_device_config();
        let (stream, sink) =
            construct_sink(device, &stream_config, sample_format)?;

        let player = Self {
            api,
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
        };

        let progress = player.track_progress.clone();
        let sink = player.sink.clone();
        let event_tx = player.event_tx.clone();
        let playing = player.is_playing.clone();
        thread::spawn(move || {
            loop {
                progress.set_current_position(sink.get_pos());
                let is_playing = playing.load(Ordering::Relaxed);

                if is_playing && sink.empty() {
                    playing.store(false, Ordering::Relaxed);
                    let _ = event_tx.send(Event::TrackEnded);
                }

                thread::sleep(Duration::from_secs(1));
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

        let generation =
            self.playback_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let playback_generation = self.playback_generation.clone();
        let api = self.api.clone();
        let sink = self.sink.clone();
        let track_progress = self.track_progress.clone();
        let ready = self.is_ready.clone();
        let playing = self.is_playing.clone();
        let event_tx = self.event_tx.clone();
        let track_clone = track.clone();

        self.current_playback_task = Some(tokio::task::spawn(async move {
            let (url, _codec, bitrate) =
                api.fetch_track_url(track_clone.id.clone()).await.unwrap();

            let stream = AudioStreamer::new(url, 256 * 1024, 256 * 1024)
                .await
                .unwrap();

            let total_bytes = stream.total_bytes;
            let decoder = Decoder::new(stream).unwrap();

            if playback_generation.load(Ordering::SeqCst) != generation {
                return;
            }

            let _ = event_tx.send(Event::TrackChanged(track_clone.clone(), 0));

            if let Some(total) = decoder.total_duration() {
                track_progress.set_total_duration(total);
            } else {
                info!("total bytes: {}", total_bytes);
                info!("bitrate: {}", bitrate);
                track_progress.set_total_duration(Duration::from_secs_f64(
                    (total_bytes * 8) as f64 / (bitrate * 1000) as f64,
                ));
            }

            sink.append(decoder);
            ready.store(true, Ordering::Relaxed);
            playing.store(true, Ordering::Relaxed);
        }));

        self.current_track = Some(track);
    }

    pub fn stop_track(&mut self) {
        self.is_ready.store(false, Ordering::Relaxed);
        self.is_playing.store(false, Ordering::Relaxed);
        self.sink.stop();
        if let Some(task) = &self.current_playback_task {
            task.abort();
        }
        self.current_playback_task = None;
        self.current_track = None;
        self.track_progress.reset();
    }

    pub fn play_pause(&mut self) {
        let is_paused = self.sink.is_paused();
        if is_paused {
            self.sink.play();
        } else {
            self.sink.pause();
            self.track_progress
                .set_current_position(self.sink.get_pos());
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

        self.sink.pause();
        let _ = self.sink.try_seek(
            self.sink
                .get_pos()
                .saturating_sub(Duration::from_secs(seconds)),
        );
        self.sink.play();
    }

    pub fn seek_forwards(&mut self, seconds: u64) {
        if !self.is_ready.load(Ordering::Relaxed) {
            return;
        }

        self.sink.pause();
        let _ = self.sink.try_seek(
            self.sink
                .get_pos()
                .saturating_add(Duration::from_secs(seconds)),
        );
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
