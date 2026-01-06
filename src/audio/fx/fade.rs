use super::Fx;
use std::time::Duration;

pub struct Fade {
    sample_rate: u32,
    channels: u16,
    fade_in_start: f32,
    fade_in_end: f32,
    fade_out_start: f32,
    fade_out_end: f32,
    inv_fade_in_duration: f32,
    inv_fade_out_duration: f32,
    current_sample: f32,
}

impl Fade {
    pub fn new(
        in_start: f32,
        in_stop: f32,
        out_start: f32,
        out_stop: f32,
        sample_rate: u32,
        channels: u16,
    ) -> Self {
        let to_samples = |t: f32| -> f32 { t * sample_rate as f32 * channels as f32 };
        let fade_in_start = to_samples(in_start);
        let fade_in_end = to_samples(in_stop);
        let fade_out_start = to_samples(out_start);
        let fade_out_end = to_samples(out_stop);
        let fade_in_duration = fade_in_end - fade_in_start;
        let fade_out_duration = fade_out_end - fade_out_start;
        Self {
            sample_rate,
            channels,
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
            current_sample: 0.0,
        }
    }
}

impl Fx for Fade {
    fn process(&mut self, sample: f32) -> f32 {
        let pos = self.current_sample;
        self.current_sample += 1.0;

        if pos >= self.fade_in_end && pos < self.fade_out_start {
            return sample;
        }

        if pos < self.fade_in_end {
            if pos < self.fade_in_start {
                return 0.0;
            }
            let gain = (pos - self.fade_in_start) * self.inv_fade_in_duration;
            return sample * gain;
        }

        if pos >= self.fade_out_end {
            return 0.0;
        }
        let gain = 1.0 - (pos - self.fade_out_start) * self.inv_fade_out_duration;
        sample * gain
    }

    fn seek(&mut self, pos: Duration) {
        let total_samples = pos.as_secs_f32() * self.sample_rate as f32 * self.channels as f32;
        self.current_sample = total_samples;
    }
}
