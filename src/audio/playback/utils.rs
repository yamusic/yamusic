use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, SampleFormat, StreamConfig,
};
use url::Url;
use yandex_music::YandexMusicClient;

pub async fn fetch_track_url(
    client: &YandexMusicClient,
    track_id: i32,
) -> (Url, String) {
    let download_info = client.get_track_download_info(track_id).await.unwrap();
    let info = download_info
        .iter()
        .max_by_key(|info| info.bitrate_in_kbps)
        .unwrap();
    let url = info.get_direct_link(&client.client).await.unwrap();

    (url::Url::parse(&url).unwrap(), info.codec.clone())
}

pub fn setup_device_config() -> (Device, StreamConfig, SampleFormat) {
    let host = cpal::default_host();
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
            buffer_size: cpal::BufferSize::Fixed(4096),
        };
        sample_format = default_config.sample_format();
    } else {
        config = StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(48000),
            buffer_size: cpal::BufferSize::Fixed(4096),
        };
        sample_format = cpal::SampleFormat::F32;
    }

    (device, config, sample_format)
}
