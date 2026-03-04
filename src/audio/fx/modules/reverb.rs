use crate::audio::fx::Effect;
use crate::audio::fx::delay::{DcCut, ModulatedAllpass, OnePole, TankAllpass, TankDelay};
use crate::audio::fx::param::EffectParams;
use std::sync::Arc;

pub struct Reverb {
    params: Arc<EffectParams>,
    sample_rate: f32,

    dc_cut: DcCut,
    input_lpf: OnePole,
    input_diffusion: [TankAllpass; 4],

    mod_ap_left: ModulatedAllpass,
    tank_delay_l1: TankDelay,
    damp_left: OnePole,
    allpass_l: TankAllpass,
    tank_delay_l2: TankDelay,

    mod_ap_right: ModulatedAllpass,
    tank_delay_r1: TankDelay,
    damp_right: OnePole,
    allpass_r: TankAllpass,
    tank_delay_r2: TankDelay,

    lfo1_phase: f32,
    lfo2_phase: f32,
    lfo1_rate: f32,
    lfo2_rate: f32,
}

#[inline]
fn lush_scale(sample_rate: f32) -> f32 {
    sample_rate / 32000.0
}

const DIFF_SIZES: [usize; 4] = [151, 113, 401, 283];
const DIFF_COEFF_1: f32 = 0.742;
const DIFF_COEFF_2: f32 = 0.633;

const TANK_MOD_L_SIZE: usize = 683;
const TANK_MOD_R_SIZE: usize = 921;
const TANK_EXCURSION: usize = 18;
const TANK_D1_L_SIZE: usize = 4507;
const TANK_D1_R_SIZE: usize = 4273;
const TANK_AP_L_SIZE: usize = 1843;
const TANK_AP_R_SIZE: usize = 2707;
const TANK_D2_L_SIZE: usize = 3791;
const TANK_D2_R_SIZE: usize = 3221;

const FEEDBACK_DIFF_BASE: f32 = 0.51;

const OUT_L_TAP_1: usize = 277;
const OUT_L_TAP_2: usize = 3011;
const OUT_L_TAP_3: usize = 1943;
const OUT_L_TAP_4: usize = 2021;
const OUT_L_TAP_5: usize = 2017;
const OUT_L_TAP_6: usize = 193;
const OUT_L_TAP_7: usize = 1087;

const OUT_R_TAP_1: usize = 367;
const OUT_R_TAP_2: usize = 3691;
const OUT_R_TAP_3: usize = 1253;
const OUT_R_TAP_4: usize = 2713;
const OUT_R_TAP_5: usize = 2141;
const OUT_R_TAP_6: usize = 347;
const OUT_R_TAP_7: usize = 131;

#[inline]
fn scaled(base: usize, rate_scale: f32, room_scale: f32) -> usize {
    ((base as f32) * rate_scale * room_scale).round().max(1.0) as usize
}

#[inline]
fn scaled_tap(base: usize, rate_scale: f32, room_scale: f32, max_size: usize) -> usize {
    let t = ((base as f32) * rate_scale * room_scale).round().max(0.0) as usize;
    if t >= max_size {
        max_size.saturating_sub(1)
    } else {
        t
    }
}

impl Reverb {
    pub fn new(params: Arc<EffectParams>, sample_rate: f32) -> Self {
        let s = lush_scale(sample_rate);
        let room = params.get(0);
        let rs = ((room / 10.0).powf(1.18)).max(0.25).min(7.0);

        let input_diffs = [DIFF_COEFF_1, DIFF_COEFF_1, DIFF_COEFF_2, DIFF_COEFF_2];
        let input_diffusion =
            std::array::from_fn(|i| TankAllpass::new(scaled(DIFF_SIZES[i], s, rs), input_diffs[i]));

        let exc = ((TANK_EXCURSION as f32) * s).round().max(1.0) as usize;

        Self {
            params,
            sample_rate,
            dc_cut: DcCut::new(sample_rate),
            input_lpf: OnePole::new(),
            input_diffusion,

            mod_ap_left: ModulatedAllpass::new(
                scaled(TANK_MOD_L_SIZE, s, rs),
                exc,
                FEEDBACK_DIFF_BASE,
            ),
            tank_delay_l1: TankDelay::new(scaled(TANK_D1_L_SIZE, s, rs)),
            damp_left: OnePole::new(),
            allpass_l: TankAllpass::new(scaled(TANK_AP_L_SIZE, s, rs), FEEDBACK_DIFF_BASE),
            tank_delay_l2: TankDelay::new(scaled(TANK_D2_L_SIZE, s, rs)),

            mod_ap_right: ModulatedAllpass::new(
                scaled(TANK_MOD_R_SIZE, s, rs),
                exc,
                FEEDBACK_DIFF_BASE,
            ),
            tank_delay_r1: TankDelay::new(scaled(TANK_D1_R_SIZE, s, rs)),
            damp_right: OnePole::new(),
            allpass_r: TankAllpass::new(scaled(TANK_AP_R_SIZE, s, rs), FEEDBACK_DIFF_BASE),
            tank_delay_r2: TankDelay::new(scaled(TANK_D2_R_SIZE, s, rs)),

            lfo1_phase: 0.1,
            lfo2_phase: 0.44,
            lfo1_rate: 0.72 / sample_rate,
            lfo2_rate: 0.47 / sample_rate,
        }
    }

    #[inline]
    fn compute_decay(&self, rt60: f32) -> f32 {
        if rt60 <= 0.0 {
            return 0.0;
        }
        let avg_delay = (self.tank_delay_l1.size()
            + self.tank_delay_l2.size()
            + self.tank_delay_r1.size()
            + self.tank_delay_r2.size()) as f32
            / 4.0;
        10.0_f32
            .powf(-3.1 / (rt60 * self.sample_rate / avg_delay))
            .clamp(0.0, 0.9997)
    }

    #[inline]
    fn auto_diffusion(decay: f32) -> f32 {
        (decay * 0.45 + 0.18).clamp(0.28, 0.52)
    }
}

impl Effect for Reverb {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let room = self.params.get(0);
        let rt60 = self.params.get(1);
        let damping = self.params.get(2);
        let mix = self.params.get(3);

        let decay = self.compute_decay(rt60);
        let diff_fb = Self::auto_diffusion(decay);
        let dry = 1.0 - mix;

        self.damp_left.set_damp(damping);
        self.damp_right.set_damp(damping);

        self.input_lpf.set_damp(damping * 0.48);

        self.allpass_l.feedback = diff_fb;
        self.allpass_r.feedback = diff_fb;
        self.mod_ap_left.feedback = diff_fb;
        self.mod_ap_right.feedback = diff_fb;

        let s = lush_scale(self.sample_rate);
        let rs = ((room / 10.0).powf(1.18)).max(0.25).min(7.0);

        let tl_1 = scaled_tap(OUT_L_TAP_1, s, rs, self.tank_delay_r1.size());
        let tl_2 = scaled_tap(OUT_L_TAP_2, s, rs, self.tank_delay_r1.size());
        let tl_3 = scaled_tap(OUT_L_TAP_3, s, rs, self.allpass_r.size());
        let tl_4 = scaled_tap(OUT_L_TAP_4, s, rs, self.tank_delay_r2.size());
        let tl_5 = scaled_tap(OUT_L_TAP_5, s, rs, self.tank_delay_l1.size());
        let tl_6 = scaled_tap(OUT_L_TAP_6, s, rs, self.allpass_l.size());
        let tl_7 = scaled_tap(OUT_L_TAP_7, s, rs, self.tank_delay_l2.size());

        let tr_1 = scaled_tap(OUT_R_TAP_1, s, rs, self.tank_delay_l1.size());
        let tr_2 = scaled_tap(OUT_R_TAP_2, s, rs, self.tank_delay_l1.size());
        let tr_3 = scaled_tap(OUT_R_TAP_3, s, rs, self.allpass_l.size());
        let tr_4 = scaled_tap(OUT_R_TAP_4, s, rs, self.tank_delay_l2.size());
        let tr_5 = scaled_tap(OUT_R_TAP_5, s, rs, self.tank_delay_r1.size());
        let tr_6 = scaled_tap(OUT_R_TAP_6, s, rs, self.allpass_r.size());
        let tr_7 = scaled_tap(OUT_R_TAP_7, s, rs, self.tank_delay_r2.size());

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mono = (*l + *r) * 0.5;
            let mut x = self.input_lpf.process(self.dc_cut.process(mono));

            for ap in &mut self.input_diffusion {
                x = ap.process(x);
            }

            let tank_l_in = x + decay * self.tank_delay_r2.output();
            let tank_r_in = x + decay * self.tank_delay_l2.output();

            let lfo1 = (self.lfo1_phase * std::f32::consts::TAU).sin();
            let lfo2 = (self.lfo2_phase * std::f32::consts::TAU).sin();
            self.lfo1_phase += self.lfo1_rate;
            if self.lfo1_phase >= 1.0 {
                self.lfo1_phase -= 1.0;
            }
            self.lfo2_phase += self.lfo2_rate;
            if self.lfo2_phase >= 1.0 {
                self.lfo2_phase -= 1.0;
            }

            let tmp = self.mod_ap_left.process(tank_l_in, lfo1);
            self.tank_delay_l1.process(tmp);
            let tmp = self.damp_left.process(self.tank_delay_l1.output()) * decay;
            let tmp = self.allpass_l.process(tmp);
            self.tank_delay_l2.process(tmp);

            let tmp = self.mod_ap_right.process(tank_r_in, lfo2);
            self.tank_delay_r1.process(tmp);
            let tmp = self.damp_right.process(self.tank_delay_r1.output()) * decay;
            let tmp = self.allpass_r.process(tmp);
            self.tank_delay_r2.process(tmp);

            let out_l = self.tank_delay_r1.tap(tl_1) + self.tank_delay_r1.tap(tl_2)
                - self.allpass_r.tap(tl_3)
                + self.tank_delay_r2.tap(tl_4)
                - self.tank_delay_l1.tap(tl_5)
                - self.allpass_l.tap(tl_6)
                - self.tank_delay_l2.tap(tl_7);

            let out_r = self.tank_delay_l1.tap(tr_1) + self.tank_delay_l1.tap(tr_2)
                - self.allpass_l.tap(tr_3)
                + self.tank_delay_l2.tap(tr_4)
                - self.tank_delay_r1.tap(tr_5)
                - self.allpass_r.tap(tr_6)
                - self.tank_delay_r2.tap(tr_7);

            *l = *l * dry + out_l * mix * 0.6;
            *r = *r * dry + out_r * mix * 0.6;
        }
    }

    fn reset(&mut self) {
        self.dc_cut.reset();
        self.input_lpf.reset();
        for ap in &mut self.input_diffusion {
            ap.reset();
        }
        self.mod_ap_left.reset();
        self.tank_delay_l1.reset();
        self.damp_left.reset();
        self.allpass_l.reset();
        self.tank_delay_l2.reset();
        self.mod_ap_right.reset();
        self.tank_delay_r1.reset();
        self.damp_right.reset();
        self.allpass_r.reset();
        self.tank_delay_r2.reset();
        self.lfo1_phase = 0.1;
        self.lfo2_phase = 0.44;
    }
}
