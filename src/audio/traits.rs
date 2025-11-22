use crate::audio::error::AudioError;
use async_trait::async_trait;
use std::time::Duration;
use yandex_music::model::track::Track;

#[async_trait]
pub trait AudioSource: Send + Sync {
    async fn get_stream_url(&self, track_id: &str) -> Result<String, AudioError>;
}

#[async_trait]
pub trait TrackProvider: Send + Sync {
    async fn get_track(&self, id: &str) -> Result<Track, AudioError>;
    async fn get_next_track(&self) -> Option<Track>;
    async fn get_previous_track(&self) -> Option<Track>;
}

pub trait PlaybackControl: Send + Sync {
    fn play(&self);
    fn pause(&self);
    fn stop(&self);
    fn seek(&self, position: Duration);
    fn set_volume(&self, volume: f32);
}
