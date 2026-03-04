use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
}

#[derive(Clone, Copy)]
struct Coeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Default for Coeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

pub struct StereoBiquad {
    c: Coeffs,
    s1l: f32,
    s2l: f32,
    s1r: f32,
    s2r: f32,
}

impl Default for StereoBiquad {
    fn default() -> Self {
        Self::new()
    }
}

impl StereoBiquad {
    pub fn new() -> Self {
        Self {
            c: Coeffs::default(),
            s1l: 0.0,
            s2l: 0.0,
            s1r: 0.0,
            s2r: 0.0,
        }
    }

    pub fn update(&mut self, ftype: FilterType, freq: f32, q: f32, gain_db: f32, sample_rate: f32) {
        let freq = freq.clamp(10.0, sample_rate * 0.499);
        let q = q.max(0.01);

        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match ftype {
            FilterType::LowPass => {
                let t = (1.0 - cos_w0) * 0.5;
                (t, 1.0 - cos_w0, t, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)
            }
            FilterType::HighPass => {
                let t = (1.0 + cos_w0) * 0.5;
                (
                    t,
                    -(1.0 + cos_w0),
                    t,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
            FilterType::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha),
            FilterType::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::Peak => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let a_alpha = alpha * a;
                let alpha_over_a = alpha / a;
                (
                    1.0 + a_alpha,
                    -2.0 * cos_w0,
                    1.0 - a_alpha,
                    1.0 + alpha_over_a,
                    -2.0 * cos_w0,
                    1.0 - alpha_over_a,
                )
            }
            FilterType::LowShelf => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
                let ap1 = a + 1.0;
                let am1 = a - 1.0;
                (
                    a * (ap1 - am1 * cos_w0 + two_sqrt_a_alpha),
                    2.0 * a * (am1 - ap1 * cos_w0),
                    a * (ap1 - am1 * cos_w0 - two_sqrt_a_alpha),
                    ap1 + am1 * cos_w0 + two_sqrt_a_alpha,
                    -2.0 * (am1 + ap1 * cos_w0),
                    ap1 + am1 * cos_w0 - two_sqrt_a_alpha,
                )
            }
            FilterType::HighShelf => {
                let a = 10.0f32.powf(gain_db / 40.0);
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
                let ap1 = a + 1.0;
                let am1 = a - 1.0;
                (
                    a * (ap1 + am1 * cos_w0 + two_sqrt_a_alpha),
                    -2.0 * a * (am1 + ap1 * cos_w0),
                    a * (ap1 + am1 * cos_w0 - two_sqrt_a_alpha),
                    ap1 - am1 * cos_w0 + two_sqrt_a_alpha,
                    2.0 * (am1 - ap1 * cos_w0),
                    ap1 - am1 * cos_w0 - two_sqrt_a_alpha,
                )
            }
        };

        let inv_a0 = 1.0 / a0;
        self.c = Coeffs {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
        };
    }

    #[inline(always)]
    pub fn process_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let Coeffs { b0, b1, b2, a1, a2 } = self.c;

        let (mut s1l, mut s2l) = (self.s1l, self.s2l);
        let (mut s1r, mut s2r) = (self.s1r, self.s2r);

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let xl = *l;
            let yl = b0 * xl + s1l;
            s1l = b1 * xl - a1 * yl + s2l;
            s2l = b2 * xl - a2 * yl;
            *l = yl;

            let xr = *r;
            let yr = b0 * xr + s1r;
            s1r = b1 * xr - a1 * yr + s2r;
            s2r = b2 * xr - a2 * yr;
            *r = yr;
        }

        self.s1l = s1l;
        self.s2l = s2l;
        self.s1r = s1r;
        self.s2r = s2r;
    }

    pub fn reset(&mut self) {
        self.s1l = 0.0;
        self.s2l = 0.0;
        self.s1r = 0.0;
        self.s2r = 0.0;
    }
}
