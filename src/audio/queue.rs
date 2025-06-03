use super::enums::RepeatMode;
use crate::{http::ApiService, util::utils::random};
use std::{collections::VecDeque, sync::Arc};
use yandex_music::model::track_model::track::Track;

pub struct QueueManager {
    pub api: Arc<ApiService>,

    pub queue: VecDeque<Track>,

    pub repeat_mode: RepeatMode,
    pub is_shuffled: bool,

    pub history: Vec<Track>,
    pub history_index: usize,
}

impl QueueManager {
    pub fn new(api: Arc<ApiService>) -> Self {
        Self {
            api,
            queue: VecDeque::new(),
            repeat_mode: RepeatMode::None,
            is_shuffled: false,
            history: Vec::new(),
            history_index: 0,
        }
    }

    pub async fn play_playlist(
        &mut self,
        tracks: Vec<Track>,
        start_index: usize,
    ) -> Option<Track> {
        if tracks.is_empty() || start_index >= tracks.len() {
            return None;
        }

        self.queue.clear();
        for track in tracks.into_iter().skip(start_index) {
            self.queue.push_back(track);
        }

        let track = self.queue.front().cloned();
        if let Some(t) = &track {
            self.add_to_history(t.clone());
        }
        track
    }

    pub async fn play_single_track(&mut self, track: Track) -> Track {
        self.queue.clear();
        self.queue.push_back(track.clone());

        let similar_tracks = self
            .api
            .fetch_similar_tracks(track.id)
            .await
            .unwrap_or_default();
        for sim_track in similar_tracks {
            self.queue.push_back(sim_track);
        }

        self.add_to_history(track.clone());
        track
    }

    pub async fn get_next_track(&mut self) -> Option<Track> {
        if let RepeatMode::Single = self.repeat_mode {
            if let Some(track) = self.queue.front() {
                return Some(track.clone());
            }
        }

        let next_track = if self.is_shuffled {
            let len = self.queue.len();
            if len > 0 {
                let random_index = random(0, len as i32 - 1) as usize;
                self.queue.remove(random_index)
            } else {
                None
            }
        } else {
            self.queue.pop_front()
        };

        if let Some(track) = next_track {
            self.add_to_history(track.clone());
            return Some(track);
        }

        if matches!(self.repeat_mode, RepeatMode::All) {
            // todo
            None
        } else {
            None
        }
    }

    pub fn get_previous_track(&mut self) -> Option<Track> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.history.get(self.history_index).cloned()
        } else {
            None
        }
    }

    pub fn queue_track(&mut self, track: Track) {
        self.queue.push_back(track);
    }

    pub fn play_next(&mut self, track: Track) {
        self.queue.push_front(track);
    }

    pub fn toggle_repeat_mode(&mut self) {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::None => RepeatMode::All,
            RepeatMode::All => RepeatMode::Single,
            RepeatMode::Single => RepeatMode::None,
        };
    }

    pub fn toggle_shuffle(&mut self) {
        self.is_shuffled = !self.is_shuffled;
    }

    fn add_to_history(&mut self, track: Track) {
        self.history.truncate(self.history_index);
        self.history.push(track);
        self.history_index = self.history.len();
    }

    pub async fn play_track_at_index(&mut self, index: usize) -> Option<Track> {
        if index < self.queue.len() {
            let track = self.queue.remove(index);
            if let Some(t) = &track {
                self.add_to_history(t.clone());
            }
            track
        } else {
            None
        }
    }
}
