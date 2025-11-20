use rodio::Source;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

pub trait Fx: Send + 'static {
    fn process(&mut self, sample: f32) -> f32;
}

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

impl<F: FnMut(f32) -> f32 + Send + 'static> Fx for F {
    fn process(&mut self, sample: f32) -> f32 {
        (self)(sample)
    }
}

pub struct FxSource<T: Source<Item = f32> + Send + 'static> {
    inner: T,
    effects: Vec<Box<dyn Fx>>,
}

impl<T: Source<Item = f32> + Send + 'static> FxSource<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            effects: Vec::new(),
        }
    }

    pub fn add_effect<E: Fx>(&mut self, effect: E) {
        self.effects.push(Box::new(effect));
    }

    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }
}

impl<T: Source<Item = f32> + Send + 'static> Source for FxSource<T> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)
    }
}

impl<T: Source<Item = f32> + Send + 'static> Iterator for FxSource<T> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut sample = self.inner.next()?;
        for effect in &mut self.effects {
            sample = effect.process(sample);
        }
        Some(sample)
    }
}
