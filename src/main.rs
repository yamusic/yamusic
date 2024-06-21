use std::num::NonZeroUsize;

use rodio::{OutputStream, Sink};
use stream_download::{
    storage::{
        adaptive::AdaptiveStorageProvider, memory::MemoryStorageProvider,
    },
    Settings, StreamDownload,
};
use yandex_music::YandexMusicClient;

async fn fetch_url() -> String {
    let client = YandexMusicClient::new(env!("YANDEX_MUSIC_TOKEN"));
    client
        .get_track_download_info(61304855)
        .await
        .unwrap()
        .iter()
        .max_by_key(|info| info.bitrate_in_kbps)
        .unwrap()
        .get_direct_link(&client.client)
        .await
        .unwrap()
}

#[tokio::main]
async fn main() {
    let url_result = fetch_url().await;
    let parsed_url = match url::Url::parse(&url_result) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Failed to parse URL: {}", e);
            return;
        }
    };

    let reader_future = StreamDownload::new_http(
        parsed_url,
        AdaptiveStorageProvider::new(
            MemoryStorageProvider,
            NonZeroUsize::new(8192 * 1024).unwrap(),
        ),
        Settings::default().prefetch_bytes(512 * 1024),
    );

    let reader = match reader_future.await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to create stream downloader: {}", e);
            return;
        }
    };

    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("Failed to get default output stream: {}", e);
            return;
        }
    };

    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create audio sink: {}", e);
            return;
        }
    };

    let source = match rodio::Decoder::new(reader) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to decode audio: {}", e);
            return;
        }
    };

    sink.append(source);
    sink.sleep_until_end();
}
