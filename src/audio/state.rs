use yandex_music::model::track::Track;

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing(Track),
    Paused(Track),
    Buffering(Track),
    Error(String),
}
