use crate::audio::fx::Effect;
use crate::audio::fx::biquad::FilterType;
use crate::audio::fx::biquad::StereoBiquad;
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct TenBandEq {
    params: Arc<EffectParams>,
    bands: [StereoBiquad; 10],
    sample_rate: f32,
}

pub const EQ_FREQUENCIES: [f32; 10] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
const EQ_Q: f32 = 1.0;

impl TenBandEq {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        Self {
            params,
            bands: std::array::from_fn(|_| StereoBiquad::new()),
            sample_rate,
        }
    }
}

impl Effect for TenBandEq {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (i, biquad) in self.bands.iter_mut().enumerate() {
            let gain_db = self.params.get(i);
            biquad.update(
                FilterType::Peak,
                EQ_FREQUENCIES[i],
                EQ_Q,
                gain_db,
                self.sample_rate,
            );
            biquad.process_block(left, right);
        }
    }

    fn reset(&mut self) {
        for b in &mut self.bands {
            b.reset();
        }
    }
}
