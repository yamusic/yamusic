use crate::audio::fx::Effect;
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct DcBlockEffect {
    params: Arc<EffectParams>,
    r: f32,
    prev_in_l: f32,
    prev_out_l: f32,
    prev_in_r: f32,
    prev_out_r: f32,
}

impl DcBlockEffect {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        let r = 1.0 - (2.0 * std::f32::consts::PI * 10.0 / sample_rate);
        Self {
            params,
            r: r.clamp(0.9, 0.9999),
            prev_in_l: 0.0,
            prev_out_l: 0.0,
            prev_in_r: 0.0,
            prev_out_r: 0.0,
        }
    }
}

impl Effect for DcBlockEffect {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let _ = self.params;
        let r = self.r;

        for (l, r_sample) in left.iter_mut().zip(right.iter_mut()) {
            let xl = *l;
            let yl = xl - self.prev_in_l + r * self.prev_out_l;
            self.prev_in_l = xl;
            self.prev_out_l = yl;
            *l = yl;

            let xr = *r_sample;
            let yr = xr - self.prev_in_r + r * self.prev_out_r;
            self.prev_in_r = xr;
            self.prev_out_r = yr;
            *r_sample = yr;
        }
    }

    fn reset(&mut self) {
        self.prev_in_l = 0.0;
        self.prev_out_l = 0.0;
        self.prev_in_r = 0.0;
        self.prev_out_r = 0.0;
    }
}
