use super::enums::RepeatMode;
use crate::{http::ApiService, util::utils::random};
use rand::{rng, seq::SliceRandom};
use std::sync::Arc;
use yandex_music::model::{
    album_model::album::Album, artist_model::artist::Artist,
    playlist_model::playlist::Playlist, track_model::track::Track,
};

pub struct QueueManager {
    pub api: Arc<ApiService>,

    pub queue: Vec<Track>,
    pub original_queue: Option<Vec<Track>>,
    pub current_track_index: usize,

    pub repeat_mode: RepeatMode,
    pub is_shuffled: bool,

    pub history: Vec<Track>,
    pub history_index: usize,

    pub playback_context: PlaybackContext,
}

pub enum PlaybackContext {
    Playlist(Playlist),
    Artist(Artist),
    Album(Album),
    Track,
    Unknown,
}

impl QueueManager {
    pub fn new(api: Arc<ApiService>) -> Self {
        Self {
            api,
            queue: Vec::new(),
            original_queue: None,
            current_track_index: 0,
            repeat_mode: RepeatMode::None,
            is_shuffled: false,
            history: Vec::new(),
            history_index: 0,
            playback_context: PlaybackContext::Unknown,
        }
    }

    pub async fn load(
        &mut self,
        context: PlaybackContext,
        tracks: Vec<Track>,
        start_index: usize,
    ) -> Option<Track> {
        if tracks.is_empty() || start_index >= tracks.len() {
            return None;
        }

        self.playback_context = context;
        self.original_queue = None;
        self.current_track_index = 0;

        match self.playback_context {
            PlaybackContext::Playlist(_)
            | PlaybackContext::Artist(_)
            | PlaybackContext::Album(_) => {
                self.queue = tracks;
                self.current_track_index = start_index;
            }
            PlaybackContext::Track | PlaybackContext::Unknown => {
                self.queue.clear();
                self.queue.push(tracks[start_index].clone());
                self.current_track_index = 0;

                let track_id = tracks[start_index].id;
                let similar_tracks = self
                    .api
                    .fetch_similar_tracks(track_id)
                    .await
                    .unwrap_or_default();
                for sim_track in similar_tracks {
                    self.queue.push(sim_track);
                }
            }
        }

        let track = self.queue.get(self.current_track_index).cloned();
        if let Some(t) = &track {
            self.add_to_history(t.clone());
        }
        track
    }

    pub async fn get_next_track(&mut self) -> Option<Track> {
        if self.queue.is_empty() {
            return None;
        }

        // If queue looping is enabled with the context being unclear, disable queue looping
        if matches!(self.repeat_mode, RepeatMode::All) {
            if let PlaybackContext::Unknown = self.playback_context {
                self.repeat_mode = RepeatMode::None;
                return None;
            }
        }

        if let RepeatMode::Single = self.repeat_mode {
            if let Some(track) = self.queue.get(self.current_track_index) {
                return Some(track.clone());
            }
        }

        let mut next_track_index = self.current_track_index + 1;

        if self.is_shuffled {
            let len = self.queue.len();
            if len > 0 {
                next_track_index = random(0, len as i32 - 1) as usize;
            }
        }

        if next_track_index >= self.queue.len() {
            if let RepeatMode::All = self.repeat_mode {
                self.current_track_index = 0;
            } else {
                return None;
            }
        } else {
            self.current_track_index = next_track_index;
        }

        let track = self.queue.get(self.current_track_index).cloned();

        if let Some(t) = &track {
            self.add_to_history(t.clone());
        }
        track
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
        self.queue.insert(self.current_track_index + 1, track);
    }

    pub fn play_next(&mut self, track: Track) {
        self.queue.insert(self.current_track_index + 1, track);
    }

    pub fn toggle_repeat_mode(&mut self) {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::None => {
                // Only allow RepeatMode::All if the context is clear
                match self.playback_context {
                    PlaybackContext::Album(_)
                    | PlaybackContext::Artist(_)
                    | PlaybackContext::Playlist(_)
                    | PlaybackContext::Track => RepeatMode::All,
                    _ => RepeatMode::None,
                }
            }
            RepeatMode::All => RepeatMode::Single,
            RepeatMode::Single => RepeatMode::None,
        };
    }

    pub fn toggle_shuffle(&mut self) {
        self.is_shuffled = !self.is_shuffled;
        if self.is_shuffled {
            self.original_queue = Some(self.queue.clone());

            if !self.queue.is_empty()
                && self.current_track_index < self.queue.len()
            {
                let current_track = self.queue.remove(self.current_track_index);
                self.queue.shuffle(&mut rng());
                self.queue.insert(self.current_track_index, current_track);
            } else {
                self.queue.shuffle(&mut rng());
                self.current_track_index = 0;
            }
        } else if let Some(original_queue) = self.original_queue.take() {
            let current_track_id =
                self.queue.get(self.current_track_index).map(|t| t.id);

            self.queue = original_queue;

            if let Some(track_id) = current_track_id {
                // TODO: This assumes that the track_id is unique, which is not always the case
                if let Some(new_index) =
                    self.queue.iter().position(|t| t.id == track_id)
                {
                    self.current_track_index = new_index;
                } else {
                    self.current_track_index = 0;
                }
            } else {
                self.current_track_index = 0;
            }
        }
    }

    fn add_to_history(&mut self, track: Track) {
        self.history.truncate(self.history_index);
        self.history.push(track);
        self.history_index = self.history.len();
    }

    pub async fn play_track_at_index(&mut self, index: usize) -> Option<Track> {
        if index < self.queue.len() {
            self.current_track_index = index;
            let track = self.queue.get(self.current_track_index).cloned();
            if let Some(t) = &track {
                self.add_to_history(t.clone());
            }
            track
        } else {
            None
        }
    }
}
