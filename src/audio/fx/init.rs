use crate::audio::fx::{FxSource, modules::*};
use rodio::Source;

pub fn init_all<T: Source<Item = f32> + Send + 'static>(source: &mut FxSource<T>) {
    let sr = source.sample_rate().get() as f32;

    let fx = eq(sr);
    source.add_effect("eq", "Equalizer", fx.0, fx.1);

    let fx = chorus(sr);
    source.add_effect("chorus", "Chorus", fx.0, fx.1);

    let fx = lowpass(sr);
    source.add_effect("lowpass", "Lowpass", fx.0, fx.1);

    let fx = highpass(sr);
    source.add_effect("highpass", "Highpass", fx.0, fx.1);

    let fx = bandpass(sr);
    source.add_effect("bandpass", "Bandpass", fx.0, fx.1);

    let fx = notch(sr);
    source.add_effect("notch", "Notch", fx.0, fx.1);

    let fx = dc_block(sr);
    source.add_effect("dc_block", "DC Block", fx.0, fx.1);

    let fx = reverb(sr);
    source.add_effect("reverb", "Reverb", fx.0, fx.1);

    let fx = delay(sr);
    source.add_effect("delay", "Delay", fx.0, fx.1);

    let fx = compressor(sr);
    source.add_effect("compressor", "Compressor", fx.0, fx.1);

    let fx = overdrive(sr);
    source.add_effect("overdrive", "Overdrive", fx.0, fx.1);
}
