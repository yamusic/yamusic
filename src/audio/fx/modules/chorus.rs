use crate::audio::fx::Effect;
use crate::audio::fx::delay::DelayLine;
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct ChorusEffect {
    params: Arc<EffectParams>,
    delay_l: DelayLine,
    delay_r: DelayLine,
    phase_l: f32,
    phase_r: f32,
    sample_rate: f32,
    base_delay: f32,
    max_depth: f32,
}

impl ChorusEffect {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        let base_delay = 0.020 * sample_rate;
        let max_depth = 0.003 * sample_rate;
        let buf_size = (base_delay + max_depth + 64.0) as usize;

        Self {
            params,
            delay_l: DelayLine::new(buf_size),
            delay_r: DelayLine::new(buf_size),
            phase_l: 0.0,
            phase_r: 0.25,
            sample_rate,
            base_delay,
            max_depth,
        }
    }
}

impl Effect for ChorusEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let rate = self.params.get(0);
        let depth = self.params.get(1);
        let mix = self.params.get(2);

        let phase_inc = rate / self.sample_rate;
        let depth_samples = self.max_depth * depth;
        let dry = 1.0 - mix;

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.delay_l.write_and_advance(*l);
            self.delay_r.write_and_advance(*r);

            let mod_l =
                (self.phase_l * std::f32::consts::TAU).sin() * depth_samples + self.base_delay;
            let mod_r =
                (self.phase_r * std::f32::consts::TAU).sin() * depth_samples + self.base_delay;

            let wet_l = self.delay_l.read_linear(mod_l);
            let wet_r = self.delay_r.read_linear(mod_r);

            *l = *l * dry + wet_l * mix;
            *r = *r * dry + wet_r * mix;

            self.phase_l += phase_inc;
            if self.phase_l >= 1.0 {
                self.phase_l -= 1.0;
            }
            self.phase_r += phase_inc;
            if self.phase_r >= 1.0 {
                self.phase_r -= 1.0;
            }
        }
    }

    fn reset(&mut self) {
        self.delay_l.clear();
        self.delay_r.clear();
        self.phase_l = 0.0;
        self.phase_r = 0.25;
    }
}
