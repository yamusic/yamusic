use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use crate::audio::fx::Fx;

pub struct AudioAnalyzer {
    sum_squares: f32,
    count: usize,
    capacity: usize,
    inv_capacity: f32,
    pub amplitude: Arc<AtomicU32>,
}

impl AudioAnalyzer {
    pub fn new(amplitude: Arc<AtomicU32>) -> Self {
        let capacity = 1024;
        Self {
            sum_squares: 0.0,
            count: 0,
            capacity,
            inv_capacity: 1.0 / capacity as f32,
            amplitude,
        }
    }
}

impl Fx for AudioAnalyzer {
    fn process(&mut self, sample: f32) -> f32 {
        self.sum_squares += sample * sample;
        self.count += 1;

        if self.count >= self.capacity {
            let rms = (self.sum_squares * self.inv_capacity).sqrt();

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
