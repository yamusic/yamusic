use crate::audio::fx::Effect;
use crate::audio::fx::biquad::{FilterType, StereoBiquad};
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct BiquadEffect {
    params: Arc<EffectParams>,
    filter: StereoBiquad,
    filter_type: FilterType,
    sample_rate: f32,
}

impl BiquadEffect {
    pub fn new(params: Arc<EffectParams>, filter_type: FilterType, sample_rate: f32) -> Self {
        Self {
            params,
            filter: StereoBiquad::new(),
            filter_type,
            sample_rate,
        }
    }
}

impl Effect for BiquadEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let (freq, q, gain_db) = match self.filter_type {
            FilterType::LowPass => (self.params.get(0), self.params.get(1), 0.0),
            FilterType::HighPass => (self.params.get(0), self.params.get(1), 0.0),
            FilterType::BandPass => (self.params.get(0), self.params.get(1), 0.0),
            FilterType::Notch => (self.params.get(0), self.params.get(1), 0.0),
            FilterType::LowShelf => (
                self.params.get(0),
                std::f32::consts::FRAC_1_SQRT_2,
                self.params.get(1),
            ),
            FilterType::HighShelf => (
                self.params.get(0),
                std::f32::consts::FRAC_1_SQRT_2,
                self.params.get(1),
            ),
            FilterType::Peak => (self.params.get(0), self.params.get(1), self.params.get(2)),
        };

        self.filter
            .update(self.filter_type, freq, q, gain_db, self.sample_rate);
        self.filter.process_block(left, right);
    }

    fn reset(&mut self) {
        self.filter.reset();
    }
}
