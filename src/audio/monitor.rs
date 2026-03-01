use std::sync::Arc;

use crate::audio::fx::Fx;
use crate::framework::audio_bridge::AudioBridge;

pub struct BridgeMonitor {
    bridge: Arc<AudioBridge>,
}

impl BridgeMonitor {
    pub fn new(bridge: Arc<AudioBridge>) -> Self {
        Self { bridge }
    }
}

impl Fx for BridgeMonitor {
    #[inline(always)]
    fn process(&mut self, sample: f32) -> f32 {
        self.bridge.process_mono(sample);
        sample
    }
}
