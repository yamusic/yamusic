use rodio::{
    cpal::{
        default_host, traits::HostTrait, BufferSize, SampleFormat, SampleRate,
        StreamConfig,
    },
    Device, DeviceTrait, OutputStream, Sink,
};

pub fn setup_device_config() -> (Device, StreamConfig, SampleFormat) {
    let host = default_host();
    let device = host.default_output_device().unwrap();
    let config: StreamConfig;
    let sample_format: SampleFormat;

    if let Ok(default_configs) = device.supported_output_configs() {
        let default_config = default_configs
            .max_by_key(|cfg| cfg.max_sample_rate().0)
            .unwrap();

        config = StreamConfig {
            channels: default_config.channels(),
            sample_rate: default_config.max_sample_rate(),
            buffer_size: BufferSize::Fixed(4096),
        };
        sample_format = default_config.sample_format();
    } else {
        config = StreamConfig {
            channels: 2,
            sample_rate: SampleRate(48000),
            buffer_size: BufferSize::Fixed(4096),
        };
        sample_format = SampleFormat::F32;
    }

    (device, config, sample_format)
}

pub fn construct_sink(
    device: &Device,
    config: &StreamConfig,
    sample_format: &SampleFormat,
) -> color_eyre::Result<(OutputStream, Sink)> {
    let (stream, stream_handle) =
        OutputStream::try_from_device_config(device, config, sample_format)?;
    let sink = Sink::try_new(&stream_handle)?;

    Ok((stream, sink))
}
