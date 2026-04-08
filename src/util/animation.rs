pub struct Animation;

impl Animation {
    #[inline]
    pub fn clamp01(t: f64) -> f64 {
        t.clamp(0.0, 1.0)
    }

    #[inline]
    pub fn lerp(from: f64, to: f64, t: f64) -> f64 {
        from + (to - from) * Self::clamp01(t)
    }

    #[inline]
    pub fn ease_in_quad(t: f64) -> f64 {
        let t = Self::clamp01(t);
        t * t
    }

    #[inline]
    pub fn ease_out_cubic(t: f64) -> f64 {
        let t = Self::clamp01(t) - 1.0;
        t * t * t + 1.0
    }

    #[inline]
    pub fn ease_in_out_cubic(t: f64) -> f64 {
        let t = Self::clamp01(t);
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - ((-2.0 * t + 2.0).powi(3) / 2.0)
        }
    }
}
