use super::Effect;

pub struct FadeEffect {
    fade_in_start: f32,
    fade_in_end: f32,
    fade_out_start: f32,
    fade_out_end: f32,
    inv_fade_in_duration: f32,
    inv_fade_out_duration: f32,
    current_frame: f32,
}

impl FadeEffect {
    pub fn new(
        in_start: f32,
        in_stop: f32,
        out_start: f32,
        out_stop: f32,
        sample_rate: u32,
        _channels: u16,
    ) -> Self {
        let to_frames = |t: f32| -> f32 { t * sample_rate as f32 };
        let fade_in_start = to_frames(in_start);
        let fade_in_end = to_frames(in_stop);
        let fade_out_start = to_frames(out_start);
        let fade_out_end = to_frames(out_stop);
        let fade_in_duration = fade_in_end - fade_in_start;
        let fade_out_duration = fade_out_end - fade_out_start;
        Self {
            fade_in_start,
            fade_in_end,
            fade_out_start,
            fade_out_end,
            inv_fade_in_duration: if fade_in_duration > 0.0 {
                1.0 / fade_in_duration
            } else {
                0.0
            },
            inv_fade_out_duration: if fade_out_duration > 0.0 {
                1.0 / fade_out_duration
            } else {
                0.0
            },
            current_frame: 0.0,
        }
    }

    #[inline(always)]
    fn apply_gain(&mut self, pos: f32) -> f32 {
        if pos >= self.fade_in_end && pos < self.fade_out_start {
            return 1.0;
        }

        if pos < self.fade_in_end {
            if pos < self.fade_in_start {
                return 0.0;
            }
            return (pos - self.fade_in_start) * self.inv_fade_in_duration;
        }

        if pos >= self.fade_out_end {
            return 0.0;
        }
        1.0 - (pos - self.fade_out_start) * self.inv_fade_out_duration
    }
}

impl Effect for FadeEffect {
    #[inline]
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let gain = self.apply_gain(self.current_frame);
            left[i] *= gain;
            right[i] *= gain;
            self.current_frame += 1.0;
        }
    }

    fn reset(&mut self) {
        self.current_frame = 0.0;
    }
}
