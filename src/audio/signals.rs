use std::sync::Arc;

use im::Vector;
use yandex_music::model::track::Track;

use crate::audio::enums::RepeatMode;
use crate::framework::audio_bridge::AudioBridge;
use crate::framework::signals::Signal;

#[derive(Clone)]
pub struct AudioSignals {
    pub is_playing: Signal<bool>,

    pub is_paused: Signal<bool>,

    pub is_stopped: Signal<bool>,

    pub is_buffering: Signal<bool>,

    pub current_track: Signal<Option<Track>>,

    pub current_track_id: Signal<Option<String>>,

    pub track_title: Signal<Option<String>>,

    pub track_artists: Signal<Option<String>>,

    pub position_ms: Signal<u64>,

    pub duration_ms: Signal<u64>,

    pub progress_ratio: Signal<f32>,

    pub buffered_ratio: Signal<f32>,

    pub volume: Signal<u8>,

    pub is_muted: Signal<bool>,

    pub queue: Signal<Vector<Track>>,

    pub history: Signal<Vector<Track>>,

    pub queue_index: Signal<usize>,

    pub queue_length: Signal<usize>,

    pub repeat_mode: Signal<RepeatMode>,

    pub is_shuffled: Signal<bool>,

    pub amplitude: Signal<f32>,

    pub bridge: Arc<AudioBridge>,
}

impl AudioSignals {
    pub fn new() -> Self {
        Self {
            is_playing: Signal::new(false),
            is_paused: Signal::new(false),
            is_stopped: Signal::new(true),
            is_buffering: Signal::new(false),
            current_track: Signal::new(None),
            current_track_id: Signal::new(None),
            track_title: Signal::new(None),
            track_artists: Signal::new(None),
            position_ms: Signal::new(0),
            duration_ms: Signal::new(0),
            progress_ratio: Signal::new(0.0),
            buffered_ratio: Signal::new(0.0),
            volume: Signal::new(100),
            is_muted: Signal::new(false),
            queue: Signal::new(Vector::new()),
            history: Signal::new(Vector::new()),
            queue_index: Signal::new(0),
            queue_length: Signal::new(0),
            repeat_mode: Signal::new(RepeatMode::None),
            is_shuffled: Signal::new(false),
            amplitude: Signal::new(0.0),
            bridge: Arc::new(AudioBridge::new(1024)),
        }
    }

    pub fn set_current_track(&self, track: Option<Track>) {
        if let Some(t) = &track {
            self.track_title.set(t.title.clone());
            self.track_artists.set(Some(
                t.artists
                    .iter()
                    .filter_map(|a| a.name.as_ref())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
            self.current_track_id.set(Some(t.id.clone()));

            if let Some(duration) = t.duration {
                self.duration_ms.set(duration.as_millis() as u64);
            }
            self.is_stopped.set(false);
        } else {
            self.track_title.set(None);
            self.track_artists.set(None);
            self.current_track_id.set(None);
            self.duration_ms.set(0);
            self.is_stopped.set(true);
        }
        self.current_track.set(track);
    }

    pub fn set_playing(&self, playing: bool) {
        self.is_playing.set(playing);
        self.is_paused.set(!playing && !self.is_stopped.get());
    }

    pub fn update_progress(&self, position_ms: u64, duration_ms: u64) {
        self.position_ms.set(position_ms);
        self.duration_ms.set(duration_ms);

        let ratio = if duration_ms > 0 {
            (position_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.progress_ratio.set(ratio);
    }

    pub fn update_buffered_ratio(&self, ratio: f32) {
        self.buffered_ratio.set(ratio.clamp(0.0, 1.0));
    }

    pub fn update_queue(&self, queue: Vector<Track>, index: usize) {
        let len = queue.len();
        self.queue.set(queue);
        self.queue_index.set(index);
        self.queue_length.set(len);
    }

    pub fn set_queue(&self, queue: Vector<Track>, history: Vector<Track>, index: usize) {
        self.queue.set(queue.clone());
        self.history.set(history);
        self.queue_index.set(index);
        self.queue_length.set(queue.len());
    }

    pub fn set_volume(&self, volume: u8, muted: bool) {
        self.volume.set(volume);
        self.is_muted.set(muted);
    }

    pub fn set_modes(&self, repeat: RepeatMode, shuffled: bool) {
        self.repeat_mode.set(repeat);
        self.is_shuffled.set(shuffled);
    }
}

impl Default for AudioSignals {
    fn default() -> Self {
        Self::new()
    }
}
