use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use crate::audio::fx::Fx;

pub struct AudioAnalyzer {
    sum_squares: f32,
    count: usize,
    capacity: usize,
    pub amplitude: Arc<AtomicU32>,
}

impl AudioAnalyzer {
    pub fn new(amplitude: Arc<AtomicU32>) -> Self {
        Self {
            sum_squares: 0.0,
            count: 0,
            capacity: 1024,
            amplitude,
        }
    }
}

impl Fx for AudioAnalyzer {
    fn process(&mut self, sample: f32) -> f32 {
        self.sum_squares += sample * sample;
        self.count += 1;

        if self.count >= self.capacity {
            let rms = (self.sum_squares / self.capacity as f32).sqrt();

            let current_bits = self.amplitude.load(Ordering::Relaxed);
            let current_val = f32::from_bits(current_bits);
            let new_val = current_val * 0.8 + rms * 0.2;
            self.amplitude.store(new_val.to_bits(), Ordering::Relaxed);

            self.sum_squares = 0.0;
            self.count = 0;
        }
        sample
    }
}
