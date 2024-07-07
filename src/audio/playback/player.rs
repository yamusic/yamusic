use rodio::{cpal::StreamConfig, OutputStream, Sink};

use super::utils::setup_device_config;

pub fn init() -> color_eyre::Result<(OutputStream, Sink, StreamConfig)> {
    let (device, cfg, sample_format) = setup_device_config();

    let (stream, stream_handle) =
        OutputStream::try_from_device_config(&device, &cfg, &sample_format)?;
    let sink = Sink::try_new(&stream_handle)?;

    Ok((stream, sink, cfg))
}
