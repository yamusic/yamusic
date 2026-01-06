use crate::audio::progress::TrackProgress;
use color_eyre::{Result, eyre::eyre};
use crossbeam_channel::{
    Receiver as CbReceiver, Sender as CbSender, TryRecvError, bounded as cb_bounded,
};
use flume::{Receiver, Sender};
use rodio::{Decoder, Source};
use std::num::NonZero;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;

use super::data_source::StreamingDataSource;

const PCM_CHUNK_SAMPLES: usize = 8192;
const SAMPLE_CHANNEL_CAPACITY: usize = 32;

enum SampleMessage {
    Samples(Vec<f32>, u64),
    Finished(u64),
}

enum DecoderCommand {
    Seek { position: Duration, generation: u64 },
    Stop,
}

#[derive(Clone)]
pub struct StreamController {
    cmd_tx: Sender<DecoderCommand>,
    generation: Arc<AtomicU64>,
}

impl StreamController {
    pub fn seek(&self, position: Duration) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.cmd_tx.send(DecoderCommand::Seek {
            position,
            generation,
        });
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.send(DecoderCommand::Stop);
    }
}

pub struct BufferedStreamingSource {
    rx: CbReceiver<SampleMessage>,
    pending_samples: Vec<f32>,
    sample_pos: usize,
    pending_generation: u64,
    generation: Arc<AtomicU64>,
    sample_rate: u32,
    channels: u16,
    total_duration: Option<Duration>,
    finished_generation: Option<u64>,
    controller: StreamController,
}

impl BufferedStreamingSource {
    fn new(
        rx: CbReceiver<SampleMessage>,
        generation: Arc<AtomicU64>,
        sample_rate: u32,
        channels: u16,
        total_duration: Option<Duration>,
        controller: StreamController,
    ) -> Self {
        let pending_generation = generation.load(Ordering::SeqCst);
        Self {
            rx,
            pending_samples: Vec::new(),
            sample_pos: 0,
            pending_generation,
            generation,
            sample_rate,
            channels,
            total_duration,
            finished_generation: None,
            controller,
        }
    }
}

impl Iterator for BufferedStreamingSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let current_generation = self.generation.load(Ordering::Acquire);
        if self.pending_generation != current_generation {
            self.pending_generation = current_generation;
            self.pending_samples.clear();
            self.sample_pos = 0;
            self.finished_generation = None;
        }

        if self.sample_pos < self.pending_samples.len() {
            let sample = self.pending_samples[self.sample_pos];
            self.sample_pos += 1;
            return Some(sample);
        }

        loop {
            match self.rx.try_recv() {
                Ok(SampleMessage::Samples(chunk, packet_generation)) => {
                    if packet_generation != current_generation {
                        continue;
                    }
                    self.pending_samples = chunk;
                    self.sample_pos = 0;
                    if self.sample_pos < self.pending_samples.len() {
                        let sample = self.pending_samples[self.sample_pos];
                        self.sample_pos += 1;
                        return Some(sample);
                    }
                }
                Ok(SampleMessage::Finished(packet_generation)) => {
                    if packet_generation == current_generation {
                        self.finished_generation = Some(packet_generation);
                        if self.sample_pos >= self.pending_samples.len() {
                            return None;
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    if self.finished_generation == Some(current_generation) {
                        return None;
                    }
                    return Some(0.0);
                }
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

impl Source for BufferedStreamingSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(self.channels).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(self.sample_rate).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.pending_samples.clear();
        self.sample_pos = 0;
        self.finished_generation = None;
        self.controller.seek(pos);
        Ok(())
    }
}

pub struct StreamingSession {
    pub source: BufferedStreamingSource,
    pub controller: StreamController,
}

pub fn create_streaming_session(
    client: Client,
    url: String,
    codec: String,
    _bitrate: u32,
    progress: Arc<TrackProgress>,
) -> Result<StreamingSession> {
    let data_source = StreamingDataSource::new(client, url, Arc::clone(&progress))?;
    let total_bytes = data_source.get_total_bytes();

    let decoder = Decoder::builder()
        .with_data(data_source)
        .with_hint(codec.as_str())
        .with_byte_len(total_bytes)
        .with_coarse_seek(true)
        .with_gapless(true)
        .build()
        .map_err(|err| eyre!(err))?;

    let sample_rate = decoder.sample_rate();
    let channels = decoder.channels();
    let total_duration = decoder.total_duration();
    if let Some(total) = total_duration {
        progress.set_total_duration(total);
    }

    let (sample_tx, sample_rx) = cb_bounded::<SampleMessage>(SAMPLE_CHANNEL_CAPACITY);
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let generation = Arc::new(AtomicU64::new(0));
    let controller = StreamController {
        cmd_tx,
        generation: generation.clone(),
    };

    let decoder_generation = generation.clone();
    let progress_clone = Arc::clone(&progress);
    let progress_generation = progress.get_generation();
    thread::Builder::new()
        .name("yamusic-stream".into())
        .spawn(move || {
            run_decode_loop(
                decoder,
                sample_tx,
                cmd_rx,
                decoder_generation,
                progress_clone,
                progress_generation,
            );
        })
        .map_err(|err| eyre!(err))?;

    let source = BufferedStreamingSource::new(
        sample_rx,
        generation,
        sample_rate.get(),
        channels.get(),
        total_duration,
        controller.clone(),
    );

    Ok(StreamingSession { source, controller })
}

fn run_decode_loop(
    mut decoder: Decoder<StreamingDataSource>,
    sample_tx: CbSender<SampleMessage>,
    cmd_rx: Receiver<DecoderCommand>,
    generation: Arc<AtomicU64>,
    progress: Arc<TrackProgress>,
    progress_generation: u64,
) {
    let mut active_generation = generation.load(Ordering::Acquire);
    let mut stopped = false;
    let mut chunk = Vec::with_capacity(PCM_CHUNK_SAMPLES);

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                DecoderCommand::Seek {
                    position,
                    generation,
                } => {
                    let _ = decoder.try_seek(position);
                    if progress_generation == progress.get_generation() {
                        progress.set_current_position(position);
                    }
                    active_generation = generation;
                }
                DecoderCommand::Stop => {
                    stopped = true;
                    break;
                }
            }
        }

        if stopped {
            break;
        }

        chunk.clear();
        for _ in 0..PCM_CHUNK_SAMPLES {
            match decoder.next() {
                Some(sample) => chunk.push(sample),
                None => break,
            }
        }

        if chunk.is_empty() {
            let _ = sample_tx.send(SampleMessage::Finished(active_generation));
            break;
        }

        let send_chunk = std::mem::replace(&mut chunk, Vec::with_capacity(PCM_CHUNK_SAMPLES));
        if sample_tx
            .send(SampleMessage::Samples(send_chunk, active_generation))
            .is_err()
        {
            break;
        }
    }
}
