pub mod handler;

use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_active_track_icon(is_playing: bool) -> &'static str {
    if is_playing {
        const FRAME_STEP_MS: u64 = 100;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let step = (now / FRAME_STEP_MS) as usize % 6;

        let level_idx = match step {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 2,
            4 => 1,
            _ => 0,
        };

        match level_idx {
            0 => "·",
            1 => "•",
            2 => "●",
            _ => "·",
        }
    } else {
        "•"
    }
}
