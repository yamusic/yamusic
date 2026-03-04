use std::sync::Arc;

use crate::audio::{fx::Effect, monitor::Monitor};

pub struct MonitorEffect {
    inner: Arc<Monitor>,
}

impl MonitorEffect {
    pub fn new(monitor: Arc<Monitor>) -> Self {
        Self { inner: monitor }
    }
}

impl Effect for MonitorEffect {
    #[inline]
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            self.inner.process_stereo(left[i], right[i]);
        }
    }

    fn reset(&mut self) {
        self.inner.reset_position();
    }
}
