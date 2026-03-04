use crate::audio::fx::Effect;
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct CompressorEffect {
    params: Arc<EffectParams>,
    envelope_l: f32,
    envelope_r: f32,
    sample_rate: f32,
}

impl CompressorEffect {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        Self {
            params,
            envelope_l: 0.0,
            envelope_r: 0.0,
            sample_rate,
        }
    }

    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    #[inline]
    fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.abs().max(1e-6).log10()
    }
}

impl Effect for CompressorEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let threshold_db = self.params.get(0);
        let ratio = self.params.get(1).max(1.0);
        let attack_ms = self.params.get(2).max(0.1);
        let release_ms = self.params.get(3).max(10.0);

        let threshold_lin = Self::db_to_linear(threshold_db);

        let makeup_db = (-threshold_db) * (1.0 - 1.0 / ratio) * 0.5;
        let makeup_lin = Self::db_to_linear(makeup_db);

        let attack_coef = (-1.0 / (attack_ms * 0.001 * self.sample_rate)).exp();
        let release_coef = (-1.0 / (release_ms * 0.001 * self.sample_rate)).exp();

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let input_level = (l.abs().max(r.abs())).max(1e-6);

            let coef = if input_level > self.envelope_l {
                attack_coef
            } else {
                release_coef
            };
            self.envelope_l = coef * self.envelope_l + (1.0 - coef) * input_level;

            let gain = if self.envelope_l > threshold_lin {
                let over_db = Self::linear_to_db(self.envelope_l / threshold_lin);
                let gr_db = over_db * (1.0 - 1.0 / ratio);
                Self::db_to_linear(-gr_db)
            } else {
                1.0
            };

            *l *= gain * makeup_lin;
            *r *= gain * makeup_lin;
        }

        self.envelope_r = self.envelope_l;
    }

    fn reset(&mut self) {
        self.envelope_l = 0.0;
        self.envelope_r = 0.0;
    }
}
