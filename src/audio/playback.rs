use crate::audio::util::{construct_sink, setup_device_config};
use rodio::{OutputStream, Sink, Source};
use std::sync::Arc;

pub struct PlaybackEngine {
    _stream: OutputStream,
    sink: Arc<Sink>,
}

impl PlaybackEngine {
    pub fn new() -> color_eyre::Result<Self> {
        let (device, stream_config, sample_format) = setup_device_config();
        let (stream, sink) = construct_sink(device, &stream_config, sample_format)?;

        Ok(Self {
            _stream: stream,
            sink: Arc::new(sink),
        })
    }

    pub fn play_source<S>(&self, source: S)
    where
        S: Source<Item = f32> + Send + 'static,
    {
        self.sink.append(source);
    }

    pub fn set_volume(&self, volume: f32) {
        self.sink.set_volume(volume);
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    pub fn play(&self) {
        self.sink.play();
    }

    pub fn stop(&self) {
        self.sink.stop();
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    pub fn is_empty(&self) -> bool {
        self.sink.empty()
    }

    pub fn get_pos(&self) -> std::time::Duration {
        self.sink.get_pos()
    }

    pub fn try_seek(&self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.sink.try_seek(pos)
    }
}
