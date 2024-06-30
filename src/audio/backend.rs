use crate::event::events::{ControlSignal, GlobalEvent};
use cpal::StreamConfig;
use crossbeam_channel::{Receiver, Sender};
use yandex_music::YandexMusicClient;

use super::playback::player::{init, play};

#[allow(dead_code)]
pub struct AudioPlayer {
    started: bool,
    stream_config: StreamConfig,
    client: YandexMusicClient,
    pub tx: Sender<GlobalEvent>,
    pub rx: Receiver<GlobalEvent>,
    tx_audio: Sender<f32>,
    rx_audio: Receiver<f32>,
    control_tx: Sender<ControlSignal>,
    control_rx: Receiver<ControlSignal>,
    stopper_tx: Sender<()>,
    stopper_rx: Receiver<()>,
}

impl AudioPlayer {
    pub async fn new(
        tx: Sender<GlobalEvent>,
        rx: Receiver<GlobalEvent>,
    ) -> anyhow::Result<Self> {
        let client = YandexMusicClient::new(env!("YANDEX_MUSIC_TOKEN"));
        let (tx_audio, rx_audio) = crossbeam_channel::bounded(128 * 1024);
        let (control_tx, control_rx) = crossbeam_channel::unbounded();
        let (stopper_tx, stopper_rx) = crossbeam_channel::bounded(1);
        let stream_config = init(rx_audio.clone(), stopper_rx.clone())?;

        Ok(Self {
            started: false,
            stream_config,
            client,
            tx,
            rx,
            tx_audio,
            rx_audio,
            control_tx,
            control_rx,
            stopper_tx,
            stopper_rx,
        })
    }

    pub async fn fetch_tracks(&self) {
        let uid = self.client.get_account_settings().await.unwrap().uid;
        let tracks = self.client.get_liked_tracks(uid).await.unwrap().tracks;
        let track_ids = tracks.iter().map(|t| t.id).collect::<Vec<_>>();

        let tracks = self.client.get_tracks(&track_ids, true).await.unwrap();

        self.tx.send(GlobalEvent::TracksFetched(tracks)).unwrap();
    }

    pub async fn play_track(&mut self, track_id: i32) {
        if self.started {
            self.stopper_tx.send(()).unwrap();
            self.control_tx.send(ControlSignal::Stop).unwrap();

            let (stopper_tx, stopper_rx) = crossbeam_channel::bounded(1);
            let (control_tx, control_rx) = crossbeam_channel::unbounded();
            let (tx_audio, rx_audio) = crossbeam_channel::bounded(256 * 1024);

            self.stopper_tx = stopper_tx.clone();
            self.stopper_rx = stopper_rx.clone();
            self.control_tx = control_tx.clone();
            self.control_rx = control_rx.clone();
            self.tx_audio = tx_audio.clone();
            self.rx_audio = rx_audio.clone();

            init(rx_audio, stopper_rx).unwrap();
            play(
                &self.client,
                track_id,
                tx_audio,
                control_rx,
                self.stream_config.channels as usize,
            )
            .await
            .unwrap();
        } else {
            play(
                &self.client,
                track_id,
                self.tx_audio.clone(),
                self.control_rx.clone(),
                self.stream_config.channels as usize,
            )
            .await
            .unwrap();

            self.started = true;
        }
    }
}
