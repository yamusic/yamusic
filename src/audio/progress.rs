use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Default, Debug)]
pub struct TrackProgress {
    current_position_millis: Arc<AtomicU64>,
    total_duration_millis: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
    buffered_bytes: Arc<AtomicU64>,
    bitrate: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
}

impl TrackProgress {
    pub fn new() -> Self {
        Self {
            current_position_millis: Arc::new(AtomicU64::new(0)),
            total_duration_millis: Arc::new(AtomicU64::new(0)),
            total_bytes: Arc::new(AtomicU64::new(0)),
            buffered_bytes: Arc::new(AtomicU64::new(0)),
            bitrate: Arc::new(AtomicU64::new(0)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_current_position(&self, position: Duration) {
        self.current_position_millis
            .store(position.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn set_total_duration(&self, duration: Duration) {
        self.total_duration_millis
            .store(duration.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn set_total_bytes(&self, bytes: u64) {
        self.total_bytes.store(bytes, Ordering::Relaxed);

        let bitrate = self.bitrate.load(Ordering::Relaxed);
        if bitrate > 0 {
            self.set_total_duration(Duration::from_secs_f64(
                (bytes * 8) as f64 / (bitrate * 1000) as f64,
            ));
        }
    }

    pub fn set_buffered_bytes(&self, bytes: u64) {
        self.buffered_bytes.store(bytes, Ordering::Relaxed);
    }

    pub fn set_bitrate(&self, bitrate: u64) {
        self.bitrate.store(bitrate, Ordering::Relaxed);
    }

    pub fn get_progress(&self) -> (u64, u64) {
        (
            self.current_position_millis.load(Ordering::Relaxed),
            self.total_duration_millis.load(Ordering::Relaxed),
        )
    }

    pub fn get_bitrate(&self) -> u64 {
        self.bitrate.load(Ordering::Relaxed)
    }

    pub fn get_buffered_ratio(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            let buffered = self.buffered_bytes.load(Ordering::Relaxed);
            buffered as f64 / total as f64
        }
    }

    pub fn get_total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn get_buffered_bytes(&self) -> u64 {
        self.buffered_bytes.load(Ordering::Relaxed)
    }

    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.set_buffered_bytes(0);
        self.set_current_position(Duration::ZERO);
        self.set_total_duration(Duration::ZERO);
        self.set_total_bytes(0);
        self.set_bitrate(0);
    }
}
