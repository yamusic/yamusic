use super::Fx;
use std::time::Duration;

pub struct Fade {
    sample_rate: u32,
    channels: u16,
    fade_in_start: u64,
    fade_in_end: u64,
    fade_out_start: u64,
    fade_out_end: u64,
    current_sample: u64,
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
        let to_samples = |t: f32| -> u64 { (t * sample_rate as f32 * channels as f32) as u64 };
        Self {
            sample_rate,
            channels,
            fade_in_start: to_samples(in_start),
            fade_in_end: to_samples(in_stop),
            fade_out_start: to_samples(out_start),
            fade_out_end: to_samples(out_stop),
            current_sample: 0,
        }
    }
}

impl Fx for Fade {
    fn process(&mut self, sample: f32) -> f32 {
        let pos = self.current_sample;
        self.current_sample += 1;

        let mut gain = 1.0;

        if pos < self.fade_in_start {
            gain *= 0.0;
        } else if pos < self.fade_in_end {
            let duration = self.fade_in_end - self.fade_in_start;
            if duration > 0 {
                gain *= (pos - self.fade_in_start) as f32 / duration as f32;
            }
        }

        if pos >= self.fade_out_end {
            gain *= 0.0;
        } else if pos >= self.fade_out_start {
            let duration = self.fade_out_end - self.fade_out_start;
            if duration > 0 {
                gain *= 1.0 - (pos - self.fade_out_start) as f32 / duration as f32;
            }
        }

        sample * gain
    }

    fn seek(&mut self, pos: Duration) {
        let total_samples =
            (pos.as_secs_f64() * self.sample_rate as f64 * self.channels as f64) as u64;
        self.current_sample = total_samples;
    }
}
