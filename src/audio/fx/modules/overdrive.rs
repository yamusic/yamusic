use crate::audio::fx::Effect;
use crate::audio::fx::biquad::{FilterType, StereoBiquad};
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct OverdriveEffect {
    params: Arc<EffectParams>,
    pre_filter: StereoBiquad,
    tone_filter: StereoBiquad,
    sample_rate: f32,
}

impl OverdriveEffect {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        Self {
            params,
            pre_filter: StereoBiquad::new(),
            tone_filter: StereoBiquad::new(),
            sample_rate,
        }
    }

    #[inline(always)]
    fn soft_clip(&self, x: f32) -> f32 {
        if x > 1.0 {
            2.0 / 3.0
        } else if x < -1.0 {
            -2.0 / 3.0
        } else {
            x - x.powi(3) / 3.0
        }
    }
}

impl Effect for OverdriveEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let drive = self.params.get(0) * 10.0 + 1.0;
        let tone_cutoff = self.params.get(1);
        let mix = self.params.get(2);
        let dry = 1.0 - mix;

        self.pre_filter
            .update(FilterType::HighPass, 80.0, 0.707, 0.0, self.sample_rate);
        self.tone_filter.update(
            FilterType::LowPass,
            tone_cutoff,
            0.707,
            0.0,
            self.sample_rate,
        );

        let dry_left: Vec<f32> = left.to_vec();
        let dry_right: Vec<f32> = right.to_vec();

        self.pre_filter.process_block(left, right);

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.soft_clip(*l * drive) / drive.sqrt();
            *r = self.soft_clip(*r * drive) / drive.sqrt();
        }

        self.tone_filter.process_block(left, right);

        for i in 0..left.len() {
            left[i] = left[i] * mix + dry_left[i] * dry;
            right[i] = right[i] * mix + dry_right[i] * dry;
        }
    }

    fn reset(&mut self) {
        self.pre_filter.reset();
        self.tone_filter.reset();
    }
}
