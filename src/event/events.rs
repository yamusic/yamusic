use std::time::Duration;

use yandex_music::model::track_model::track::Track;

pub enum Event {
    // Events
    Initialize,
    TracksFetched(Vec<Track>),
    TrackEnded,

    // Commands
    Play(i32),
    Resume,
    Pause,
    Volume(u8),
    VolumeUp(u8),
    VolumeDown(u8),
    Next,
    Previous,
    Seek(u32),
    SeekForward(u32),
    SeekBackward(u32),
    ToggleMute,
}

pub enum ControlSignal {
    Stop,
    Seek(u64),
    SeekForward(u64),
    SeekBackward(u64),
}

pub enum PlayerCommand {
    Play,
    Pause,
    Volume(f32),
    Seek(Duration),
}
