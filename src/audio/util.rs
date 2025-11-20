use rodio::{
    Device, DeviceTrait, OutputStream, OutputStreamBuilder, Sink,
    cpal::{BufferSize, SampleFormat, SampleRate, StreamConfig, default_host, traits::HostTrait},
};

pub fn setup_device_config() -> (Device, StreamConfig, SampleFormat) {
    let host = default_host();
    let device = host.default_output_device().unwrap();
    let config: StreamConfig;
    let sample_format: SampleFormat;

    if let Ok(default_config) = device.default_output_config() {
        config = default_config.config();
        sample_format = default_config.sample_format();
    } else {
        config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(44100),
            buffer_size: BufferSize::Default,
        };
        sample_format = SampleFormat::F32;
    }

    (device, config, sample_format)
}

pub fn construct_sink(
    device: Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
) -> color_eyre::Result<(OutputStream, Sink)> {
    let stream = OutputStreamBuilder::default()
        .with_buffer_size(config.buffer_size)
        .with_sample_rate(config.sample_rate.0)
        .with_device(device)
        .with_sample_format(sample_format)
        .open_stream_or_fallback()?;
    let mixer = stream.mixer();
    let sink = Sink::connect_new(mixer);

    Ok((stream, sink))
}
