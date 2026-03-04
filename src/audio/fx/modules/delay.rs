use crate::audio::fx::Effect;
use crate::audio::fx::delay::DelayLine;
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct StereoDelayEffect {
    params: Arc<EffectParams>,
    delay_l: DelayLine,
    delay_r: DelayLine,
    sample_rate: f32,
}

impl StereoDelayEffect {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        let max_delay_samples = (2.0 * sample_rate) as usize;
        Self {
            params,
            delay_l: DelayLine::new(max_delay_samples),
            delay_r: DelayLine::new(max_delay_samples),
            sample_rate,
        }
    }
}

impl Effect for StereoDelayEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let time_l_ms = self.params.get(0);
        let time_r_ms = self.params.get(1);
        let feedback = self.params.get(2).clamp(0.0, 0.95);
        let mix = self.params.get(3);

        let delay_l_samples = (time_l_ms * 0.001 * self.sample_rate).max(1.0);
        let delay_r_samples = (time_r_ms * 0.001 * self.sample_rate).max(1.0);
        let dry = 1.0 - mix;

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let delayed_l = self.delay_l.read_linear(delay_l_samples);
            let delayed_r = self.delay_r.read_linear(delay_r_samples);

            let fb_l = *l + delayed_r * feedback;
            let fb_r = *r + delayed_l * feedback;

            self.delay_l.write_and_advance(fb_l);
            self.delay_r.write_and_advance(fb_r);

            *l = *l * dry + delayed_l * mix;
            *r = *r * dry + delayed_r * mix;
        }
    }

    fn reset(&mut self) {
        self.delay_l.clear();
        self.delay_r.clear();
    }
}
