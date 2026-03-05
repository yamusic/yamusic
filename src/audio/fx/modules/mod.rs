pub mod biquad;
pub mod chorus;
pub mod compressor;
pub mod dc_block;
pub mod delay;
pub mod eq;
pub mod fade;
pub mod monitor;
pub mod overdrive;
pub mod reverb;

pub use biquad::BiquadEffect;
pub use chorus::ChorusEffect;
pub use compressor::CompressorEffect;
pub use dc_block::DcBlockEffect;
pub use delay::StereoDelayEffect;
pub use eq::{EQ_FREQUENCIES, Equalizer};
pub use fade::FadeEffect;
pub use monitor::MonitorEffect;
pub use overdrive::OverdriveEffect;
pub use reverb::Reverb;

use std::sync::Arc;

use crate::audio::fx::Effect;

use super::biquad::FilterType;
use super::param::{EffectParams, ParamInfo};

pub fn bass_boost(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Frequency",
            min: 40.0,
            max: 250.0,
            default: 80.0,
        },
        ParamInfo {
            name: "Gain",
            min: 0.0,
            max: 12.0,
            default: 6.0,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::LowShelf,
        sample_rate,
    ));
    (effect, params)
}

pub fn treble_boost(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Frequency",
            min: 2000.0,
            max: 16000.0,
            default: 8000.0,
        },
        ParamInfo {
            name: "Gain",
            min: 0.0,
            max: 12.0,
            default: 3.0,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::HighShelf,
        sample_rate,
    ));
    (effect, params)
}

pub fn lowpass(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Cutoff",
            min: 200.0,
            max: 20000.0,
            default: 5000.0,
        },
        ParamInfo {
            name: "Q",
            min: 0.3,
            max: 5.0,
            default: 0.7,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::LowPass,
        sample_rate,
    ));
    (effect, params)
}

pub fn highpass(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Cutoff",
            min: 20.0,
            max: 2000.0,
            default: 80.0,
        },
        ParamInfo {
            name: "Q",
            min: 0.3,
            max: 5.0,
            default: 0.7,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::HighPass,
        sample_rate,
    ));
    (effect, params)
}

pub fn bandpass(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Center",
            min: 100.0,
            max: 10000.0,
            default: 440.0,
        },
        ParamInfo {
            name: "Q",
            min: 0.5,
            max: 30.0,
            default: 10.0,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::BandPass,
        sample_rate,
    ));
    (effect, params)
}

pub fn notch(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Center",
            min: 20.0,
            max: 10000.0,
            default: 60.0,
        },
        ParamInfo {
            name: "Q",
            min: 0.5,
            max: 30.0,
            default: 5.0,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(BiquadEffect::new(
        params.clone(),
        FilterType::Notch,
        sample_rate,
    ));
    (effect, params)
}

pub fn chorus(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Rate",
            min: 0.1,
            max: 5.0,
            default: 1.5,
        },
        ParamInfo {
            name: "Depth",
            min: 0.0,
            max: 1.0,
            default: 0.7,
        },
        ParamInfo {
            name: "Mix",
            min: 0.0,
            max: 1.0,
            default: 0.7,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(ChorusEffect::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn reverb(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Room Size",
            min: 5.0,
            max: 50.0,
            default: 10.0,
        },
        ParamInfo {
            name: "Decay",
            min: 0.5,
            max: 10.0,
            default: 2.5,
        },
        ParamInfo {
            name: "Damping",
            min: 0.0,
            max: 1.0,
            default: 0.5,
        },
        ParamInfo {
            name: "Mix",
            min: 0.0,
            max: 1.0,
            default: 0.15,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(Reverb::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn dc_block(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info: Vec<ParamInfo> = vec![];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(DcBlockEffect::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn eq(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let names: [&str; 15] = [
        "25 Hz", "40 Hz", "63 Hz", "100 Hz", "160 Hz", "250 Hz", "400 Hz", "630 Hz", "1 kHz",
        "1.6 kHz", "2.5 kHz", "4 kHz", "6.3 kHz", "10 kHz", "16 kHz",
    ];
    let info: Vec<ParamInfo> = names
        .iter()
        .map(|name| ParamInfo {
            name,
            min: -12.0,
            max: 12.0,
            default: 0.0,
        })
        .collect();
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(Equalizer::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn delay(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Time L",
            min: 10.0,
            max: 2000.0,
            default: 375.0,
        },
        ParamInfo {
            name: "Time R",
            min: 10.0,
            max: 2000.0,
            default: 500.0,
        },
        ParamInfo {
            name: "Feedback",
            min: 0.0,
            max: 0.95,
            default: 0.4,
        },
        ParamInfo {
            name: "Mix",
            min: 0.0,
            max: 1.0,
            default: 0.3,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(StereoDelayEffect::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn compressor(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Threshold",
            min: -40.0,
            max: 0.0,
            default: -12.0,
        },
        ParamInfo {
            name: "Ratio",
            min: 1.0,
            max: 20.0,
            default: 4.0,
        },
        ParamInfo {
            name: "Attack",
            min: 0.1,
            max: 100.0,
            default: 5.0,
        },
        ParamInfo {
            name: "Release",
            min: 10.0,
            max: 1000.0,
            default: 100.0,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(CompressorEffect::new(params.clone(), sample_rate));
    (effect, params)
}

pub fn overdrive(sample_rate: f32) -> (Box<dyn Effect>, Arc<EffectParams>) {
    let info = vec![
        ParamInfo {
            name: "Drive",
            min: 0.0,
            max: 1.0,
            default: 0.5,
        },
        ParamInfo {
            name: "Tone",
            min: 1000.0,
            max: 10000.0,
            default: 3000.0,
        },
        ParamInfo {
            name: "Mix",
            min: 0.0,
            max: 1.0,
            default: 0.5,
        },
    ];
    let params = Arc::new(EffectParams::new(&info));
    let effect = Box::new(OverdriveEffect::new(params.clone(), sample_rate));
    (effect, params)
}
