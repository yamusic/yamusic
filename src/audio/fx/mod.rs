use rodio::Source;

use std::{num::NonZero, time::Duration};

use crate::audio::monitor::BridgeMonitor;

pub mod analyzer;
pub mod fade;

const BUFFER_SIZE: usize = 512;

pub trait Fx: Send + 'static {
    fn process(&mut self, sample: f32) -> f32;

    fn seek(&mut self, _pos: Duration) {}
}

pub enum AudioEffect {
    Analyzer(analyzer::AudioAnalyzer),
    BridgeMonitor(BridgeMonitor),
    Fade(fade::Fade),
}

impl AudioEffect {
    #[inline(always)]
    pub fn process(&mut self, sample: f32) -> f32 {
        match self {
            AudioEffect::Analyzer(e) => e.process(sample),
            AudioEffect::BridgeMonitor(e) => e.process(sample),
            AudioEffect::Fade(e) => e.process(sample),
        }
    }

    #[inline(always)]
    pub fn seek(&mut self, pos: Duration) {
        match self {
            AudioEffect::Analyzer(e) => e.seek(pos),
            AudioEffect::BridgeMonitor(e) => e.seek(pos),
            AudioEffect::Fade(e) => e.seek(pos),
        }
    }
}

pub struct FxSource<T: Source<Item = f32> + Send + 'static> {
    inner: T,
    effects: Vec<AudioEffect>,
    buffer: [f32; BUFFER_SIZE],
    buffer_pos: usize,
    buffer_len: usize,
}

impl<T: Source<Item = f32> + Send + 'static> FxSource<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            effects: Vec::new(),
            buffer: [0.0; BUFFER_SIZE],
            buffer_pos: 0,
            buffer_len: 0,
        }
    }

    pub fn add_effect(&mut self, effect: AudioEffect) {
        self.effects.push(effect);
    }

    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    #[inline(always)]
    fn fill_buffer(&mut self) -> bool {
        self.buffer_pos = 0;
        self.buffer_len = 0;

        for i in 0..BUFFER_SIZE {
            match self.inner.next() {
                Some(sample) => {
                    self.buffer[i] = sample;
                    self.buffer_len += 1;
                }
                None => break,
            }
        }

        if self.buffer_len == 0 {
            return false;
        }

        for effect in &mut self.effects {
            for i in 0..self.buffer_len {
                self.buffer[i] = effect.process(self.buffer[i]);
            }
        }

        true
    }
}

impl<T: Source<Item = f32> + Send + 'static> Source for FxSource<T> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> NonZero<u16> {
        self.inner.channels()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let res = self.inner.try_seek(pos);
        if res.is_ok() {
            self.buffer_pos = 0;
            self.buffer_len = 0;
            for effect in &mut self.effects {
                effect.seek(pos);
            }
        }
        res
    }
}

impl<T: Source<Item = f32> + Send + 'static> Iterator for FxSource<T> {
    type Item = f32;

    #[inline(always)]
    fn next(&mut self) -> Option<f32> {
        if self.buffer_pos >= self.buffer_len && !self.fill_buffer() {
            return None;
        }

        let sample = self.buffer[self.buffer_pos];
        self.buffer_pos += 1;
        Some(sample)
    }
}
