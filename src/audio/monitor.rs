use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
};

use crate::framework::reactive::Signal;

pub const DEFAULT_WAVEFORM_SIZE: usize = 2048;

pub const DEFAULT_SPECTRUM_SIZE: usize = 512;

#[allow(dead_code)]
pub struct AudioRingBuffer<const N: usize = DEFAULT_WAVEFORM_SIZE> {
    buffer: Box<[AtomicU32; N]>,
    write_head: AtomicUsize,
    read_head: AtomicUsize,
    samples_written: AtomicU64,
}

impl<const N: usize> AudioRingBuffer<N> {
    pub fn new() -> Self {
        let buffer: Box<[AtomicU32; N]> = {
            let mut v = Vec::with_capacity(N);
            for _ in 0..N {
                v.push(AtomicU32::new(0));
            }
            v.into_boxed_slice().try_into().ok().unwrap()
        };

        Self {
            buffer,
            write_head: AtomicUsize::new(0),
            read_head: AtomicUsize::new(0),
            samples_written: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn push(&self, sample: f32) {
        let head = self.write_head.load(Ordering::Relaxed);
        self.buffer[head].store(sample.to_bits(), Ordering::Relaxed);
        self.write_head
            .store((head + 1) & (N - 1), Ordering::Release);
        self.samples_written.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn push_slice(&self, samples: &[f32]) {
        let mut head = self.write_head.load(Ordering::Relaxed);

        for &sample in samples {
            self.buffer[head].store(sample.to_bits(), Ordering::Relaxed);
            head = (head + 1) & (N - 1);
        }

        self.write_head.store(head, Ordering::Release);
        self.samples_written
            .fetch_add(samples.len() as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn write_head(&self) -> usize {
        self.write_head.load(Ordering::Acquire)
    }

    #[inline]
    pub fn samples_written(&self) -> u64 {
        self.samples_written.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn read_latest(&self, count: usize, out: &mut [f32]) {
        let head = self.write_head.load(Ordering::Acquire);
        let count = count.min(N).min(out.len());

        for i in 0..count {
            let idx = (head + N - count + i) & (N - 1);
            out[i] = f32::from_bits(self.buffer[idx].load(Ordering::Relaxed));
        }
    }

    #[inline]
    pub fn sample_at(&self, offset: usize) -> f32 {
        let head = self.write_head.load(Ordering::Acquire);
        let idx = (head + N - 1 - offset) & (N - 1);
        f32::from_bits(self.buffer[idx].load(Ordering::Relaxed))
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        N
    }

    pub fn clear(&self) {
        for sample in self.buffer.iter() {
            sample.store(0, Ordering::Relaxed);
        }
        self.write_head.store(0, Ordering::Release);
        self.samples_written.store(0, Ordering::Relaxed);
    }
}

impl<const N: usize> Default for AudioRingBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<const N: usize> Send for AudioRingBuffer<N> {}
unsafe impl<const N: usize> Sync for AudioRingBuffer<N> {}

pub struct WaveformBridge<const N: usize = DEFAULT_WAVEFORM_SIZE> {
    buffer: Arc<AudioRingBuffer<N>>,
    head_signal: Signal<usize>,
    notify_threshold: usize,
    samples_since_notify: AtomicUsize,
}

impl<const N: usize> WaveformBridge<N> {
    pub fn new(notify_threshold: usize) -> Self {
        Self {
            buffer: Arc::new(AudioRingBuffer::new()),
            head_signal: Signal::new(0),
            notify_threshold,
            samples_since_notify: AtomicUsize::new(0),
        }
    }

    pub fn buffer(&self) -> Arc<AudioRingBuffer<N>> {
        Arc::clone(&self.buffer)
    }

    pub fn head_signal(&self) -> &Signal<usize> {
        &self.head_signal
    }

    #[inline]
    pub fn push(&self, sample: f32) {
        self.buffer.push(sample);

        let count = self.samples_since_notify.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.notify_threshold {
            self.samples_since_notify.store(0, Ordering::Relaxed);
            self.head_signal.set(self.buffer.write_head());
        }
    }

    #[inline]
    pub fn push_slice(&self, samples: &[f32]) {
        self.buffer.push_slice(samples);

        let count = self
            .samples_since_notify
            .fetch_add(samples.len(), Ordering::Relaxed)
            + samples.len();
        if count >= self.notify_threshold {
            self.samples_since_notify.store(0, Ordering::Relaxed);
            self.head_signal.set(self.buffer.write_head());
        }
    }

    pub fn notify(&self) {
        self.head_signal.set(self.buffer.write_head());
    }

    #[inline]
    pub fn read_latest(&self, count: usize, out: &mut [f32]) {
        self.buffer.read_latest(count, out);
    }
}

impl<const N: usize> Clone for WaveformBridge<N> {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            head_signal: self.head_signal.clone(),
            notify_threshold: self.notify_threshold,
            samples_since_notify: AtomicUsize::new(0),
        }
    }
}

pub struct AmplitudeTracker {
    current: AtomicU32,
    peak: AtomicU32,
    attack: f32,
    release: f32,
    peak_hold_samples: usize,
    samples_since_peak: AtomicUsize,
}

impl AmplitudeTracker {
    pub fn new(attack: f32, release: f32, peak_hold_ms: u32, sample_rate: u32) -> Self {
        let peak_hold_samples = (peak_hold_ms as f32 * sample_rate as f32 / 1000.0) as usize;

        Self {
            current: AtomicU32::new(0),
            peak: AtomicU32::new(0),
            attack: attack.clamp(0.0, 1.0),
            release: release.clamp(0.0, 1.0),
            peak_hold_samples,
            samples_since_peak: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn process(&self, sample: f32) {
        let abs_sample = sample.abs();
        let current_bits = self.current.load(Ordering::Relaxed);
        let current = f32::from_bits(current_bits);

        let new_value = if abs_sample > current {
            current + (abs_sample - current) * self.attack
        } else {
            current + (abs_sample - current) * self.release
        };

        self.current.store(new_value.to_bits(), Ordering::Relaxed);

        let peak_bits = self.peak.load(Ordering::Relaxed);
        let peak = f32::from_bits(peak_bits);

        if abs_sample > peak {
            self.peak.store(abs_sample.to_bits(), Ordering::Relaxed);
            self.samples_since_peak.store(0, Ordering::Relaxed);
        } else {
            let samples = self.samples_since_peak.fetch_add(1, Ordering::Relaxed);
            if samples >= self.peak_hold_samples {
                let new_peak = peak * 0.995;
                self.peak.store(new_peak.to_bits(), Ordering::Relaxed);
            }
        }
    }

    #[inline]
    pub fn amplitude(&self) -> f32 {
        f32::from_bits(self.current.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }

    pub fn reset(&self) {
        self.current.store(0, Ordering::Relaxed);
        self.peak.store(0, Ordering::Relaxed);
        self.samples_since_peak.store(0, Ordering::Relaxed);
    }
}

impl Default for AmplitudeTracker {
    fn default() -> Self {
        Self::new(0.3, 0.05, 500, 44100)
    }
}

pub struct SpectrumBridge<const N: usize = DEFAULT_SPECTRUM_SIZE> {
    magnitudes: Box<[AtomicU32; N]>,
    generation: AtomicU64,
    update_signal: Signal<u64>,
}

impl<const N: usize> SpectrumBridge<N> {
    pub fn new() -> Self {
        let magnitudes: Box<[AtomicU32; N]> = {
            let mut v = Vec::with_capacity(N);
            for _ in 0..N {
                v.push(AtomicU32::new(0));
            }
            v.into_boxed_slice().try_into().ok().unwrap()
        };

        Self {
            magnitudes,
            generation: AtomicU64::new(0),
            update_signal: Signal::new(0),
        }
    }

    pub fn update(&self, data: &[f32]) {
        let len = data.len().min(N);
        for (i, &value) in data.iter().take(len).enumerate() {
            self.magnitudes[i].store(value.to_bits(), Ordering::Relaxed);
        }

        let generation = self.generation.fetch_add(1, Ordering::Release) + 1;
        self.update_signal.set(generation);
    }

    pub fn read(&self, out: &mut [f32]) {
        let len = out.len().min(N);
        for i in 0..len {
            out[i] = f32::from_bits(self.magnitudes[i].load(Ordering::Relaxed));
        }
    }

    #[inline]
    pub fn bin(&self, index: usize) -> f32 {
        if index < N {
            f32::from_bits(self.magnitudes[index].load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    pub fn signal(&self) -> &Signal<u64> {
        &self.update_signal
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub const fn bins(&self) -> usize {
        N
    }
}

impl<const N: usize> Default for SpectrumBridge<N> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<const N: usize> Send for SpectrumBridge<N> {}
unsafe impl<const N: usize> Sync for SpectrumBridge<N> {}

pub struct Monitor {
    pub waveform: WaveformBridge,
    pub amplitude_left: AmplitudeTracker,
    pub amplitude_right: AmplitudeTracker,
    pub spectrum: SpectrumBridge,
    combined_amplitude: AtomicU32,
    playing: Signal<bool>,
    position: AtomicU64,
    enabled: std::sync::atomic::AtomicBool,
    focused: std::sync::atomic::AtomicBool,
}

impl Monitor {
    pub fn new(notify_samples: usize) -> Self {
        Self {
            waveform: WaveformBridge::new(notify_samples),
            amplitude_left: AmplitudeTracker::default(),
            amplitude_right: AmplitudeTracker::default(),
            spectrum: SpectrumBridge::new(),
            combined_amplitude: AtomicU32::new(0),
            playing: Signal::new(false),
            position: AtomicU64::new(0),
            enabled: std::sync::atomic::AtomicBool::new(false),
            focused: std::sync::atomic::AtomicBool::new(true),
        }
    }

    #[inline]
    pub fn process_stereo(&self, left: f32, right: f32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let mono = (left + right) * 0.5;
        self.waveform.push(mono);

        self.amplitude_left.process(left);
        self.amplitude_right.process(right);

        let combined = (self.amplitude_left.amplitude() + self.amplitude_right.amplitude()) * 0.5;
        self.combined_amplitude
            .store(combined.to_bits(), Ordering::Relaxed);

        self.position.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn process_mono(&self, sample: f32) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        self.waveform.push(sample);
        self.amplitude_left.process(sample);
        self.position.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn set_focused(&self, focused: bool) {
        self.focused.store(focused, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_focused(&self) -> bool {
        self.focused.load(Ordering::Relaxed)
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.set(playing);
    }

    pub fn is_playing(&self) -> bool {
        self.playing.get()
    }

    pub fn playing_signal(&self) -> &Signal<bool> {
        &self.playing
    }

    pub fn position(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn combined_amplitude(&self) -> f32 {
        f32::from_bits(self.combined_amplitude.load(Ordering::Relaxed))
    }

    pub fn reset_position(&self) {
        self.position.store(0, Ordering::Relaxed);
        self.amplitude_left.reset();
        self.amplitude_right.reset();
        self.combined_amplitude.store(0, Ordering::Relaxed);
        self.waveform.buffer().clear();
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Clone for Monitor {
    fn clone(&self) -> Self {
        Self {
            waveform: self.waveform.clone(),
            amplitude_left: AmplitudeTracker::default(),
            amplitude_right: AmplitudeTracker::default(),
            spectrum: SpectrumBridge::new(),
            combined_amplitude: AtomicU32::new(self.combined_amplitude.load(Ordering::Relaxed)),
            playing: self.playing.clone(),
            position: AtomicU64::new(self.position.load(Ordering::Relaxed)),
            enabled: std::sync::atomic::AtomicBool::new(self.enabled.load(Ordering::Relaxed)),
            focused: std::sync::atomic::AtomicBool::new(self.focused.load(Ordering::Relaxed)),
        }
    }
}
