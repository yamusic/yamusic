use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use crate::{
    audio::playback::utils::fetch_track_url,
    event::events::{ControlSignal, GlobalEvent, PlayerCommand},
    stream::streamer::AudioStreamer,
};
use flume::{Receiver, Sender};
use rodio::{cpal::StreamConfig, Decoder, OutputStream, Sink, Source};
use tracing::info;
use yandex_music::{model::track_model::track::Track, YandexMusicClient};

use super::{playback::player::init, progress::TrackProgress};

#[allow(dead_code)]
pub struct AudioPlayer {
    stream: OutputStream,
    sink: Arc<Sink>,
    stream_config: StreamConfig,
    client: Arc<YandexMusicClient>,
    event_tx: Sender<GlobalEvent>,
    tx_audio: Sender<(f32, i64)>,
    rx_audio: Receiver<(f32, i64)>,
    control_tx: Sender<ControlSignal>,
    control_rx: Receiver<ControlSignal>,
    stopper_tx: Sender<bool>,
    stopper_rx: Receiver<bool>,
    player_tx: Sender<PlayerCommand>,
    player_rx: Receiver<PlayerCommand>,

    pub track: Option<Track>,
    pub tracks: Vec<Track>,
    pub track_index: usize,
    pub volume: u8,

    pub track_progress: Arc<TrackProgress>,
    pub playing: Arc<AtomicBool>,
}

impl AudioPlayer {
    pub async fn new(
        event_tx: flume::Sender<GlobalEvent>,
    ) -> color_eyre::Result<Self> {
        let client = Arc::new(YandexMusicClient::new(&std::env::var(
            "YANDEX_MUSIC_TOKEN",
        )?));
        let (tx_audio, rx_audio) = flume::bounded(128 * 1024);
        let (control_tx, control_rx) = flume::unbounded();
        let (stopper_tx, stopper_rx) = flume::unbounded::<bool>();
        let (player_tx, player_rx) = flume::unbounded::<PlayerCommand>();
        let (stream, sink, stream_config) = init()?;

        let player = Self {
            stream,
            sink: Arc::new(sink),
            stream_config,
            client,
            event_tx,
            tx_audio,
            rx_audio,
            control_tx,
            control_rx,
            stopper_tx,
            stopper_rx,
            player_tx,
            player_rx,

            track: None,
            tracks: Vec::new(),
            track_index: 0,
            volume: 100,

            track_progress: Arc::new(TrackProgress::default()),
            playing: Arc::new(AtomicBool::new(false)),
        };

        let progress = player.track_progress.clone();
        let sink = player.sink.clone();
        let event_tx = player.event_tx.clone();
        let playing = player.playing.clone();
        thread::spawn(move || loop {
            progress.set_current_position(sink.get_pos());

            if playing.load(Ordering::Relaxed) && sink.empty() {
                event_tx.send(GlobalEvent::TrackEnded).unwrap();
            }

            thread::sleep(Duration::from_secs(1));
        });

        Ok(player)
    }

    pub async fn init(&mut self) -> color_eyre::Result<()> {
        YandexMusicClient::fetch_tracks(self).await;

        Ok(())
    }

    pub fn previous_track(&mut self) {
        if self.track_index != 0 {
            self.track_index -= 1;
        }
        self.track = Some(self.tracks[self.track_index].clone());
    }

    pub fn next_track(&mut self) {
        self.track_index = if self.track_index < self.tracks.len() - 1 {
            self.track_index + 1
        } else {
            0
        };
        self.track = Some(self.tracks[self.track_index].clone());
    }

    pub async fn play_previous(&mut self) {
        self.previous_track();
        self.play_track(self.track.as_ref().unwrap().id).await
    }

    pub async fn play_next(&mut self) {
        self.next_track();
        self.play_track(self.track.as_ref().unwrap().id).await
    }

    pub async fn play_track(&mut self, track_id: i32) {
        self.stop_track();

        let client = self.client.clone();
        let sink = self.sink.clone();
        let track_progress = self.track_progress.clone();
        let playing = self.playing.clone();
        tokio::spawn(async move {
            let (url, codec, bitrate) =
                fetch_track_url(&client, track_id).await;
            let stream = AudioStreamer::new(url, 256 * 1024).unwrap();
            let total_bytes = stream.total_bytes;
            let decoder = if codec == "mp3" {
                Decoder::new_mp3(stream)
            } else {
                Decoder::new_aac(stream)
            }
            .unwrap();

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
            playing.store(true, Ordering::Relaxed);
        });
    }

    pub fn stop_track(&mut self) {
        self.playing.store(false, Ordering::Relaxed);
        self.sink.stop();
    }

    pub fn play_pause(&mut self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume;
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn volume_up(&mut self, volume: u8) {
        self.volume = self.volume.saturating_add(volume);
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn volume_down(&mut self, volume: u8) {
        self.volume = self.volume.saturating_sub(volume);
        self.sink.set_volume(self.volume as f32 / 100.0);
    }

    pub fn seek_backwards(&mut self, seconds: u64) {
        self.sink
            .try_seek(self.sink.get_pos() - Duration::from_secs(seconds))
            .unwrap();
    }

    pub fn seek_forwards(&mut self, seconds: u64) {
        self.sink
            .try_seek(self.sink.get_pos() + Duration::from_secs(seconds))
            .unwrap();
    }
}

trait Player {
    async fn fetch_tracks(player: &mut AudioPlayer);
}

impl Player for YandexMusicClient {
    async fn fetch_tracks(player: &mut AudioPlayer) {
        let uid = player.client.get_account_settings().await.unwrap().uid;
        let tracks = player.client.get_liked_tracks(uid).await.unwrap().tracks;
        let track_ids = tracks.iter().map(|t| t.id).collect::<Vec<_>>();

        let tracks = player.client.get_tracks(&track_ids, true).await.unwrap();

        player.tracks = tracks;
    }
}
