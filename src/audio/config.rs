#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub volume: u8,
    pub volume_step: u8,
    pub seek_step_secs: u64,
    pub buffer_size_ms: u64,
    pub fade_duration_secs: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            volume: 100,
            volume_step: 5,
            seek_step_secs: 5,
            buffer_size_ms: 500,
            fade_duration_secs: 2.0,
        }
    }
}
