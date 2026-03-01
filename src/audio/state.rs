use yandex_music::model::track::Track;

use crate::audio::liked::LikedCache;

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing(Track),
    Paused(Track),
    Buffering(Track),
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct SystemState {
    pub liked: LikedCache,
}
