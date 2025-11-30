use std::time::Duration;
use yandex_music::model::track::Track;

#[derive(Debug, Clone)]
pub enum AudioCommand {
    PlayTrack(Track),
    Pause,
    Resume,
    Stop,
    Seek(Duration),
    SetVolume(f32),
    LoadTrack(Track),
}
