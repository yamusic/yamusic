use std::thread;

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    FromSample, SizedSample, StreamConfig,
};
use crossbeam_channel::{Receiver, Sender};
use yandex_music::YandexMusicClient;

use crate::{event::events::ControlSignal, stream::streamer::AudioStreamer};

use super::{
    decoder::decode_audio,
    utils::{fetch_track_url, setup_device_config},
};

pub fn init(
    rx: Receiver<f32>,
    stopper_rx: Receiver<()>,
) -> anyhow::Result<StreamConfig> {
    let (device, config, sample_format) = setup_device_config();
    let config_clone = config.clone();

    match sample_format {
        cpal::SampleFormat::I8 => run::<i8>(device, config, rx, stopper_rx),
        cpal::SampleFormat::I16 => run::<i16>(device, config, rx, stopper_rx),
        cpal::SampleFormat::I32 => run::<i32>(device, config, rx, stopper_rx),
        cpal::SampleFormat::U8 => run::<u8>(device, config, rx, stopper_rx),
        cpal::SampleFormat::U16 => run::<u16>(device, config, rx, stopper_rx),
        cpal::SampleFormat::U32 => run::<u32>(device, config, rx, stopper_rx),
        cpal::SampleFormat::F32 => run::<f32>(device, config, rx, stopper_rx),
        cpal::SampleFormat::F64 => run::<f64>(device, config, rx, stopper_rx),
        sample_format => Err(anyhow::Error::msg(format!(
            "Unsupported sample format '{:?}'",
            sample_format
        ))),
    }?;

    Ok(config_clone)
}

fn run<T>(
    device: cpal::Device,
    config: cpal::StreamConfig,
    rx: Receiver<f32>,
    stopper_rx: Receiver<()>,
) -> anyhow::Result<()>
where
    T: SizedSample + FromSample<f32>,
{
    thread::spawn(move || {
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    for sample in data.iter_mut() {
                        match rx.recv() {
                            Ok(x) => {
                                *sample = T::from_sample(x);
                            }
                            Err(_) => {
                                return;
                            }
                        }
                    }
                },
                |err| eprintln!("an error occurred on stream: {}", err),
                None,
            )
            .unwrap();
        stream.play().unwrap();

        let _ = stopper_rx.recv();
    });

    Ok(())
}

pub async fn play(
    client: &YandexMusicClient,
    track_id: i32,
    tx: Sender<f32>,
    control_rx: Receiver<ControlSignal>,
    channels: usize,
) -> anyhow::Result<()> {
    let (url, codec) = fetch_track_url(client, track_id).await;
    let stream = AudioStreamer::new(url, 32 * 1024, 128 * 1024).unwrap();

    thread::spawn(move || {
        decode_audio(stream, channels, codec, tx, control_rx)
    });

    Ok(())
}
