use rodio::{cpal::StreamConfig, OutputStream, Sink};

use super::utils::setup_device_config;

pub fn init() -> color_eyre::Result<(OutputStream, Sink, StreamConfig)> {
    let (device, cfg, sample_format) = setup_device_config();

    // let cfg = SupportedStreamConfig::new(
    //     config.channels,
    //     config.sample_rate,
    //     SupportedBufferSize::Range {
    //         min: 1024 * 1024 * 8,
    //         max: 1024 * 1024 * 8,
    //     },
    //     sample_format,
    // );
    // let config = device.default_output_config().unwrap();

    // let config = SupportedStreamConfig::new(
    //     config.channels(),
    //     config.sample_rate(),
    //     SupportedBufferSize::Range {
    //         min: 1024 * 1024 * 16,
    //         max: 1024 * 1024 * 16,
    //     },
    //     config.sample_format(),
    // );

    let (stream, stream_handle) =
        OutputStream::try_from_device_config(&device, &cfg, &sample_format)?;
    let sink = Sink::try_new(&stream_handle)?;

    Ok((stream, sink, cfg))
}
