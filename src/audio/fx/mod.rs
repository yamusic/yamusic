use rodio::Source;

use std::{collections::HashMap, num::NonZero, time::Duration};

pub mod biquad;
pub mod chain;
pub mod delay;
pub mod init;
pub mod modules;
pub mod param;

pub use param::EffectHandle;

use chain::EffectChain;

const BUFFER_SIZE: usize = 512;

pub trait Effect: Send + 'static {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);

    fn reset(&mut self);
}

pub struct FxSource<T: Source<Item = f32> + Send + 'static> {
    inner: T,
    chain: EffectChain,
    buffer: [f32; BUFFER_SIZE],
    buffer_pos: usize,
    buffer_len: usize,
}

impl<T: Source<Item = f32> + Send + 'static> FxSource<T> {
    pub fn new(inner: T) -> Self {
        let channels = inner.channels().get();
        let sample_rate = inner.sample_rate().get();

        Self {
            inner,
            chain: EffectChain::new(channels, sample_rate),
            buffer: [0.0; BUFFER_SIZE],
            buffer_pos: 0,
            buffer_len: 0,
        }
    }

    pub fn add_effect(
        &mut self,
        id: &str,
        name: &str,
        effect: Box<dyn Effect>,
        params: std::sync::Arc<param::EffectParams>,
    ) -> EffectHandle {
        self.chain.add(id, name, effect, params)
    }

    pub fn get_effect_handle(&self, name: &str) -> Option<&EffectHandle> {
        self.chain.get_handle(name)
    }

    pub fn toggle_effect(&mut self, name: &str) -> bool {
        if let Some(handle) = self.chain.get_handle(name) {
            let enabled = handle.is_enabled();
            handle.set_enabled(!enabled);
            true
        } else {
            false
        }
    }

    pub fn is_effect_enabled(&self, name: &str) -> Option<bool> {
        self.chain.get_handle(name).map(|h| h.is_enabled())
    }

    pub fn get_effect_handles(&self) -> HashMap<String, EffectHandle> {
        self.chain.handles()
    }

    pub fn clear_effects(&mut self) {
        self.chain.clear();
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

        self.chain.process_block(&mut self.buffer, self.buffer_len);

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
            self.chain.seek(pos);
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
