use super::enums::RepeatMode;
use crate::http::ApiService;
use rand::{rng, seq::SliceRandom};
use std::sync::Arc;
use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, rotor::session::Session, track::Track,
};

pub struct QueueManager {
    pub api: Arc<ApiService>,

    pub queue: Vec<Track>,
    pub original_queue: Option<Vec<Track>>,
    pub shuffled_index_map: Vec<Option<usize>>,
    pub current_track_index: usize,

    pub repeat_mode: RepeatMode,
    pub is_shuffled: bool,

    pub history: Vec<Track>,
    pub history_index: usize,

    pub playback_context: PlaybackContext,
    pub wave_session: Option<Session>,
}

pub enum PlaybackContext {
    Playlist(Playlist),
    Artist(Artist),
    Album(Album),
    Track,
    Unknown,
    List,
    Wave(Session),
}

impl QueueManager {
    pub fn new(api: Arc<ApiService>) -> Self {
        Self {
            api,
            queue: Vec::new(),
            original_queue: None,
            shuffled_index_map: Vec::new(),
            current_track_index: 0,
            repeat_mode: RepeatMode::None,
            is_shuffled: false,
            history: Vec::new(),
            history_index: 0,
            playback_context: PlaybackContext::Unknown,
            wave_session: None,
        }
    }

    pub async fn load(
        &mut self,
        context: PlaybackContext,
        mut tracks: Vec<Track>,
        start_index: usize,
    ) -> Option<Track> {
        if tracks.is_empty() || start_index >= tracks.len() {
            return None;
        }

        self.playback_context = context;
        self.original_queue = None;
        self.shuffled_index_map.clear();
        self.current_track_index = 0;
        self.history.clear();
        self.history_index = 0;
        self.wave_session = None;

        match self.playback_context {
            PlaybackContext::Playlist(_)
            | PlaybackContext::Artist(_)
            | PlaybackContext::Album(_)
            | PlaybackContext::List => {
                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
            }
            PlaybackContext::Wave(ref session) => {
                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
                self.wave_session = Some(session.clone());
            }
            PlaybackContext::Track | PlaybackContext::Unknown => {
                self.queue.clear();
                if start_index < tracks.len() {
                    let track = tracks.swap_remove(start_index);
                    let track_id = track.id.clone();
                    self.queue.push(track.clone());

                    if let PlaybackContext::Track = self.playback_context {
                        if !track.track_source.as_ref().is_some_and(|s| s == "UGC") {
                            let session = self
                                .api
                                .create_session(vec![format!("track:{track_id}")])
                                .await
                                .unwrap();

                            let album_id = track.albums[0].id.unwrap();
                            let session_tracks = self
                                .api
                                .get_session_tracks(
                                    session.batch_id.clone(),
                                    vec![format!("{track_id}:{album_id}")],
                                )
                                .await
                                .unwrap();

                            for sim_track in session_tracks.sequence {
                                self.queue.push(sim_track.track);
                            }
                        }
                    }
                }
            }
        }

        let track = self.queue.get(self.current_track_index).cloned();
        if let Some(t) = &track {
            if self.history.is_empty() || self.history.last().map(|h| h.id != t.id).unwrap_or(true)
            {
                self.add_to_history(t.clone());
            }
        }
        track
    }

    pub async fn get_next_track(&mut self) -> Option<Track> {
        if self.queue.is_empty() {
            return None;
        }

        if self.repeat_mode == RepeatMode::All
            && let PlaybackContext::Unknown = self.playback_context
        {
            self.repeat_mode = RepeatMode::None;
            return None;
        }

        if let RepeatMode::Single = self.repeat_mode
            && let Some(track) = self.queue.get(self.current_track_index)
        {
            return Some(track.clone());
        }

        let next_track_index = self.current_track_index + 1;

        if next_track_index >= self.queue.len() {
            if let PlaybackContext::Wave(ref mut session) = self.playback_context {
                let session_id = session
                    .radio_session_id
                    .clone()
                    .unwrap_or(session.batch_id.clone());
                let queue_history: Vec<String> = self
                    .history
                    .iter()
                    .rev()
                    .take(20)
                    .map(|t| {
                        format!(
                            "{}:{}",
                            t.id,
                            t.albums
                                .first()
                                .and_then(|a| a.id.as_ref().map(|id| id.to_string()))
                                .unwrap_or_default()
                        )
                    })
                    .collect();

                let api = self.api.clone();
                if let Ok(new_session) = api.get_session_tracks(session_id, queue_history).await {
                    *session = new_session.clone();
                    for item in new_session.sequence {
                        self.queue.push(item.track);
                    }
                    if self.queue.len() > next_track_index {
                        self.current_track_index = next_track_index;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else if let RepeatMode::All = self.repeat_mode {
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
        if self.history_index >= 2 {
            self.history_index -= 2;
            let track = self.history.get(self.history_index).cloned();
            self.history_index += 1;

            if let Some(t) = track {
                if let Some(index) = self.queue.iter().position(|q| q.id == t.id) {
                    self.current_track_index = index;
                } else {
                    self.queue.clear();
                    self.queue.push(t.clone());
                    if self.is_shuffled {
                        self.shuffled_index_map.clear();
                        self.shuffled_index_map.push(None);
                    }
                    self.current_track_index = 0;
                    self.playback_context = PlaybackContext::Unknown;
                }

                return Some(t);
            }
        }
        None
    }

    pub fn queue_track(&mut self, track: Track) {
        self.queue.insert(self.current_track_index + 1, track);
        if self.is_shuffled {
            self.shuffled_index_map
                .insert(self.current_track_index + 1, None);
        }
    }

    pub fn play_next(&mut self, track: Track) {
        self.queue.insert(self.current_track_index + 1, track);
        if self.is_shuffled {
            self.shuffled_index_map
                .insert(self.current_track_index + 1, None);
        }
    }

    pub fn toggle_repeat_mode(&mut self) {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::None => match self.playback_context {
                PlaybackContext::Album(_)
                | PlaybackContext::Artist(_)
                | PlaybackContext::Playlist(_)
                | PlaybackContext::Track
                | PlaybackContext::List => RepeatMode::All,
                _ => RepeatMode::None,
            },
            RepeatMode::All => RepeatMode::Single,
            RepeatMode::Single => RepeatMode::None,
        };
    }

    pub fn toggle_shuffle(&mut self) {
        self.is_shuffled = !self.is_shuffled;
        if self.is_shuffled {
            self.original_queue = Some(self.queue.clone());
            let mut indices: Vec<Option<usize>> = (0..self.queue.len()).map(Some).collect();

            if !self.queue.is_empty() && self.current_track_index < self.queue.len() {
                let current_track = self.queue.remove(self.current_track_index);
                let current_index = indices.remove(self.current_track_index);

                let mut combined: Vec<(Track, Option<usize>)> =
                    self.queue.drain(..).zip(indices.drain(..)).collect();
                combined.shuffle(&mut rng());

                for (t, i) in combined {
                    self.queue.push(t);
                    indices.push(i);
                }

                self.queue.insert(self.current_track_index, current_track);
                indices.insert(self.current_track_index, current_index);
            } else {
                let mut combined: Vec<(Track, Option<usize>)> =
                    self.queue.drain(..).zip(indices.drain(..)).collect();
                combined.shuffle(&mut rng());

                for (t, i) in combined {
                    self.queue.push(t);
                    indices.push(i);
                }
                self.current_track_index = 0;
            }
            self.shuffled_index_map = indices;
        } else if let Some(original_queue) = self.original_queue.take() {
            let original_index = self
                .shuffled_index_map
                .get(self.current_track_index)
                .and_then(|i| *i);

            self.queue = original_queue;
            self.shuffled_index_map.clear();

            if let Some(index) = original_index {
                self.current_track_index = index;
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
        if index >= self.queue.len() {
            return None;
        }

        if index > self.current_track_index {
            let remove_start = self.current_track_index + 1;
            if remove_start < index {
                self.queue.drain(remove_start..index);
                if self.is_shuffled {
                    self.shuffled_index_map.drain(remove_start..index);
                }
            }
            self.current_track_index += 1;
        } else {
            self.current_track_index = index;
        }

        let track = self.queue.get(self.current_track_index).cloned();
        if let Some(t) = &track {
            self.add_to_history(t.clone());
        }

        track
    }
}
