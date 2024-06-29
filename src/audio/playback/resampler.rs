// TODO

#![allow(dead_code)]
pub struct Resampler {
    ratio: f32,
}

impl Resampler {
    pub fn new(src_rate: f32, dst_rate: f32) -> Self {
        Self {
            ratio: dst_rate / src_rate,
        }
    }

    pub fn resample(&self, samples: &[f32]) -> Vec<f32> {
        let src_len = samples.len() as f32;
        let dst_len = (src_len * self.ratio).ceil() as usize;

        let mut resampled = Vec::with_capacity(dst_len);

        for i in 0..dst_len {
            let pos = i as f32 / self.ratio;
            let floor = pos.floor();
            let src_index = floor as usize;
            let frac = pos - floor;

            if src_index + 1 < samples.len() {
                let sample = samples[src_index]
                    + frac * (samples[src_index + 1] - samples[src_index]);
                resampled.push(sample);
            } else {
                resampled.push(samples[src_index]);
            }
        }

        resampled
    }

    pub fn resample_single(
        &mut self,
        sample: f32,
        source_rate: f32,
        dest_rate: f32,
    ) -> f32 {
        let source_interval = 1.0 / source_rate;
        let dest_interval = 1.0 / dest_rate;

        let t = (sample / source_interval) % 1.0;
        let t_next = t + dest_interval / source_interval;
        let value = sample / source_interval;

        let resampled_value = value + (t_next - value) * (1.0 - t_next);

        resampled_value * dest_interval
    }
}
