use super::enums::RepeatMode;
use crate::http::ApiService;
use crate::util::track::extract_track_ids;
use rand::{rng, seq::SliceRandom};
use std::sync::Arc;
use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, rotor::session::Session, track::Track,
};

const FETCH_BATCH_SIZE: usize = 10;

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

    pub pending_track_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackContext {
    Playlist(Playlist),

    Artist(Artist),

    Album(Album),

    Track(Track),

    Wave(Session),

    Standalone,
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
            playback_context: PlaybackContext::Standalone,
            wave_session: None,
            pending_track_ids: Vec::new(),
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
        self.pending_track_ids.clear();

        match self.playback_context {
            PlaybackContext::Playlist(ref playlist) => {
                let all_track_ids = playlist
                    .tracks
                    .as_ref()
                    .map(extract_track_ids)
                    .unwrap_or_default();

                let loaded_count = (start_index + tracks.len()).min(all_track_ids.len());
                self.pending_track_ids = all_track_ids.into_iter().skip(loaded_count).collect();

                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
            }
            PlaybackContext::Artist(_) | PlaybackContext::Album(_) => {
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
            PlaybackContext::Track(ref seed_track) => {
                self.queue.clear();
                self.queue.push(seed_track.clone());

                if !seed_track.track_source.as_ref().is_some_and(|s| s == "UGC") {
                    let track_id = seed_track.id.clone();
                    if let Some(album) = seed_track.albums.first() {
                        if let Some(album_id) = album.id {
                            let session = self
                                .api
                                .create_session(vec![format!("track:{track_id}")])
                                .await
                                .ok();

                            if let Some(session) = session {
                                let session_tracks = self
                                    .api
                                    .get_session_tracks(
                                        session.batch_id.clone(),
                                        vec![format!("{track_id}:{album_id}")],
                                    )
                                    .await
                                    .ok();

                                if let Some(session_tracks) = session_tracks {
                                    for sim_track in session_tracks.sequence {
                                        self.queue.push(sim_track.track);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            PlaybackContext::Standalone => {
                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
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
            && let PlaybackContext::Standalone = self.playback_context
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
            if !self.pending_track_ids.is_empty() {
                if self.fetch_pending_tracks().await {
                    if self.queue.len() > next_track_index {
                        self.current_track_index = next_track_index;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else if let PlaybackContext::Wave(ref mut session) = self.playback_context {
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

    async fn fetch_pending_tracks(&mut self) -> bool {
        if self.pending_track_ids.is_empty() {
            return false;
        }

        let batch_size = FETCH_BATCH_SIZE.min(self.pending_track_ids.len());
        let batch: Vec<String> = self.pending_track_ids.drain(..batch_size).collect();

        let api = self.api.clone();
        match api.fetch_tracks_by_ids(batch).await {
            Ok(new_tracks) => {
                let new_tracks: Vec<Track> = new_tracks
                    .into_iter()
                    .filter(|t| t.available.unwrap_or(false))
                    .collect();

                if new_tracks.is_empty() {
                    return false;
                }

                for track in new_tracks {
                    self.queue.push(track);
                }
                true
            }
            Err(_) => false,
        }
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
                    self.playback_context = PlaybackContext::Standalone;
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
                | PlaybackContext::Track(_)
                | PlaybackContext::Standalone => RepeatMode::All,
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
