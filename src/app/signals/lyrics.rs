use std::sync::Arc;
use yandex_music::model::info::lyrics::LyricsFormat;

use super::AudioSignals;
use crate::framework::reactive::Resource;
use crate::http::ApiService;

#[derive(Clone)]
pub struct LyricsSignals {
    pub content: Resource<Option<String>>,
}

impl LyricsSignals {
    pub fn new(api: Arc<ApiService>, audio: &AudioSignals) -> Self {
        let content = Resource::new({
            let api = api.clone();
            let current_track = audio.current_track.clone();

            move || {
                let api = api.clone();
                let track = current_track.get();

                async move {
                    let track = match track {
                        Some(t) => t,
                        None => return Ok(None),
                    };

                    let format = track.lyrics_info.as_ref().and_then(|l| {
                        if l.has_available_sync_lyrics {
                            Some(LyricsFormat::LRC)
                        } else if l.has_available_text_lyrics {
                            Some(LyricsFormat::TEXT)
                        } else {
                            None
                        }
                    });

                    let format = match format {
                        Some(f) => f,
                        None => return Ok(None),
                    };

                    tracing::debug!("Fetching lyrics for track: {} ({})", track.id, format);

                    match api.fetch_lyrics(track.id, format).await {
                        Ok(lyrics) => {
                            tracing::debug!("Fetched lyrics: {:?}", lyrics.is_some());
                            Ok(lyrics)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to fetch lyrics: {}", e);
                            Ok(None)
                        }
                    }
                }
            }
        });

        Self { content }
    }
}
