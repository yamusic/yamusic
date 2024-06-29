use yandex_music::model::track_model::track::Track;

pub enum GlobalEvent {
    // Events
    Initialize,
    TracksFetched(Vec<Track>),

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
    Seek(u32),
}
