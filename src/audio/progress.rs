use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

#[derive(Default)]
pub struct TrackProgress {
    current_position: Arc<RwLock<Duration>>,
    total_duration: Arc<RwLock<Duration>>,
}

impl TrackProgress {
    pub fn set_current_position(&self, position: Duration) {
        if let Ok(mut current) = self.current_position.write() {
            *current = position;
        }
    }

    pub fn set_total_duration(&self, duration: Duration) {
        if let Ok(mut total) = self.total_duration.write() {
            *total = duration;
        }
    }

    pub fn reset(&self) {
        self.set_current_position(Duration::ZERO);
        self.set_total_duration(Duration::ZERO);
    }

    pub fn get_progress(&self) -> (Duration, Duration) {
        (
            *self.current_position.read().unwrap(),
            *self.total_duration.read().unwrap(),
        )
    }
}
