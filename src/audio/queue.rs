use super::enums::RepeatMode;
use crate::audio::cache::UrlCache;
use crate::audio::stream_manager::StreamManager;
use crate::event::events::Event;
use crate::http::ApiService;
use crate::util::track::extract_track_ids;
use flume::Sender;
use rand::{rng, seq::SliceRandom};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{error, info, warn};

use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, rotor::session::Session, track::Track,
};

const FETCH_BATCH_SIZE: usize = 10;
const URL_PREFETCH_WINDOW: usize = 5;
const URL_PREFETCH_BATCH_SIZE: usize = 3;
const FETCH_THRESHOLD: usize = 2;

#[derive(Debug)]
enum PrefetchMessage {
    UpdateInterest {
        needed_ids: Vec<String>,
        current_id: Option<String>,
    },
    Reset,
}

struct UrlPrefetcher {
    tx: mpsc::UnboundedSender<PrefetchMessage>,
}

impl UrlPrefetcher {
    fn new(api: Arc<ApiService>, url_cache: UrlCache) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<PrefetchMessage>();

        tokio::spawn(async move {
            let mut current_task: Option<JoinHandle<()>> = None;
            let mut current_task_ids: HashSet<String> = HashSet::new();
            let mut pending_ids: VecDeque<String> = VecDeque::new();

            loop {
                if current_task.is_none() && !pending_ids.is_empty() {
                    let mut batch = Vec::new();
                    while batch.len() < URL_PREFETCH_BATCH_SIZE {
                        if let Some(id) = pending_ids.pop_front() {
                            if url_cache.get(&id).is_none() {
                                batch.push(id);
                            }
                        } else {
                            break;
                        }
                    }

                    if !batch.is_empty() {
                        let api = api.clone();
                        let cache = url_cache.clone();
                        let batch_ids = batch.clone();

                        current_task_ids = batch.iter().cloned().collect();

                        info!(count = batch.len(), "spawn_url_fetch_task");

                        current_task = Some(tokio::spawn(async move {
                            let result = tokio::time::timeout(
                                Duration::from_secs(10),
                                api.fetch_track_urls_batch(batch_ids.clone()),
                            )
                            .await;

                            match result {
                                Ok(Ok(urls)) => {
                                    for (id, url, codec, bitrate) in urls {
                                        cache.insert(id, url, codec, bitrate);
                                    }
                                }
                                Ok(Err(e)) => {
                                    error!(error = %e, "url_fetch_failed");
                                }
                                Err(_) => {
                                    warn!("url_fetch_timeout");
                                }
                            }
                        }));
                    }
                }

                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(PrefetchMessage::UpdateInterest { needed_ids, current_id }) => {
                                let should_abort = if let Some(_) = &current_task {
                                    if let Some(focus) = &current_id {
                                        !current_task_ids.contains(focus)
                                    } else {
                                        true
                                    }
                                } else {
                                    false
                                };

                                if should_abort {
                                    if let Some(task) = current_task.take() {
                                        info!("aborting_stale_url_task");
                                        task.abort();
                                    }
                                    current_task_ids.clear();
                                }
                                pending_ids.clear();
                                for id in needed_ids {
                                    if url_cache.get(&id).is_none() {
                                        if !current_task_ids.contains(&id) {
                                            pending_ids.push_back(id);
                                        }
                                    }
                                }
                            }
                            Some(PrefetchMessage::Reset) => {
                                if let Some(task) = current_task.take() {
                                    task.abort();
                                }
                                current_task_ids.clear();
                                pending_ids.clear();
                            }
                            None => break,
                        }
                    }
                    _ = async {
                        if let Some(task) = &mut current_task {
                             let _ = task.await;
                        } else {
                             std::future::pending::<()>().await;
                        }
                    } => {
                        current_task = None;
                        current_task_ids.clear();
                    }
                }
            }
        });

        Self { tx }
    }

    fn update(&self, needed: Vec<String>, current: Option<String>) {
        let _ = self.tx.send(PrefetchMessage::UpdateInterest {
            needed_ids: needed,
            current_id: current,
        });
    }

    fn reset(&self) {
        let _ = self.tx.send(PrefetchMessage::Reset);
    }
}

pub struct QueueManager {
    pub api: Arc<ApiService>,
    pub url_cache: UrlCache,
    pub stream_manager: Arc<StreamManager>,
    url_prefetcher: UrlPrefetcher,

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

    pub fetch_task: Option<JoinHandle<(Vec<Track>, Option<Session>)>>,
    pub event_tx: Option<Sender<Event>>,
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
    pub fn new(
        api: Arc<ApiService>,
        url_cache: UrlCache,
        stream_manager: Arc<StreamManager>,
    ) -> Self {
        let url_prefetcher = UrlPrefetcher::new(api.clone(), url_cache.clone());
        Self {
            api,
            url_cache,
            stream_manager,
            url_prefetcher,
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
            fetch_task: None,
            event_tx: None,
        }
    }

    pub fn set_event_tx(&mut self, tx: Sender<Event>) {
        self.event_tx = Some(tx);
    }

    pub async fn load(
        &mut self,
        context: PlaybackContext,
        mut tracks: Vec<Track>,
        mut start_index: usize,
    ) -> Option<Track> {
        if let Some(task) = self.fetch_task.take() {
            task.abort();
        }

        self.url_prefetcher.reset();

        self.playback_context = context;
        self.original_queue = None;
        self.shuffled_index_map.clear();
        self.history.clear();
        self.history_index = 0;
        self.wave_session = None;
        self.pending_track_ids.clear();
        self.is_shuffled = false;

        match self.playback_context {
            PlaybackContext::Playlist(ref playlist) => {
                let all_track_ids = playlist
                    .tracks
                    .as_ref()
                    .map(extract_track_ids)
                    .unwrap_or_default();

                let loaded_count = (start_index + tracks.len()).min(all_track_ids.len());
                self.pending_track_ids = all_track_ids.into_iter().skip(loaded_count).collect();

                if start_index >= tracks.len() {
                    start_index = 0;
                }

                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
            }
            PlaybackContext::Artist(_) | PlaybackContext::Album(_) => {
                if start_index >= tracks.len() {
                    start_index = 0;
                }
                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
            }
            PlaybackContext::Wave(ref session) => {
                if start_index >= tracks.len() {
                    start_index = 0;
                }
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
                            if let Ok(session) = self
                                .api
                                .create_session(vec![format!("track:{track_id}")])
                                .await
                            {
                                if let Ok(new_tracks) = self
                                    .api
                                    .get_session_tracks(
                                        session.batch_id.clone(),
                                        vec![format!("{track_id}:{album_id}")],
                                    )
                                    .await
                                {
                                    for sim in new_tracks.sequence {
                                        self.queue.push(sim.track);
                                    }
                                    self.playback_context = PlaybackContext::Wave(session);
                                    self.wave_session = match &self.playback_context {
                                        PlaybackContext::Wave(s) => Some(s.clone()),
                                        _ => None,
                                    };
                                }
                            }
                        }
                    }
                }
            }
            PlaybackContext::Standalone => {
                if start_index >= tracks.len() {
                    start_index = 0;
                }
                if start_index > 0 {
                    tracks.drain(0..start_index);
                }
                self.queue = tracks;
            }
        }

        self.current_track_index = 0;
        let track = self.queue.get(self.current_track_index).cloned();
        if let Some(t) = &track {
            self.add_to_history(t.clone());
            self.update_prefetch_interest();
        }
        track
    }

    pub async fn get_next_track(&mut self) -> Option<Track> {
        if self.queue.is_empty() {
            return None;
        }

        if self.repeat_mode == RepeatMode::Single {
            if let Some(track) = self.queue.get(self.current_track_index) {
                return Some(track.clone());
            }
        }

        self.poll_fetch().await;

        let next_index = self.current_track_index + 1;

        if next_index + FETCH_THRESHOLD >= self.queue.len() {
            self.trigger_fetch();
        }

        if next_index < self.queue.len() {
            self.current_track_index = next_index;
            let track = self.queue[self.current_track_index].clone();
            self.add_to_history(track.clone());
            self.update_prefetch_interest();
            return Some(track);
        }

        if let Some(task) = &mut self.fetch_task {
            if let Ok((new_tracks, session)) = task.await {
                self.fetch_task = None;
                if !new_tracks.is_empty() {
                    self.queue.extend(new_tracks);
                    if let Some(s) = session {
                        self.wave_session = Some(s);
                    }

                    if next_index < self.queue.len() {
                        self.current_track_index = next_index;
                        let track = self.queue[self.current_track_index].clone();
                        self.add_to_history(track.clone());
                        self.update_prefetch_interest();
                        return Some(track);
                    }
                }
            }
        }

        if self.repeat_mode == RepeatMode::All {
            self.current_track_index = 0;
            if let Some(track) = self.queue.get(0) {
                let t = track.clone();
                self.add_to_history(t.clone());
                self.update_prefetch_interest();
                return Some(t);
            }
        }

        None
    }

    pub fn get_previous_track(&mut self) -> Option<Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            let track = self.queue.get(self.current_track_index).cloned();
            if let Some(t) = &track {
                self.add_to_history(t.clone());
                self.update_prefetch_interest();
            }
            return track;
        }
        if self.repeat_mode == RepeatMode::All && !self.queue.is_empty() {
            self.current_track_index = self.queue.len() - 1;
            let track = self.queue.get(self.current_track_index).cloned();
            if let Some(t) = &track {
                self.add_to_history(t.clone());
                self.update_prefetch_interest();
            }
            return track;
        }

        None
    }

    pub async fn play_track_at_index(&mut self, index: usize) -> Option<Track> {
        self.poll_fetch().await;

        if index < self.queue.len() {
            self.current_track_index = index;
            let track = self.queue[index].clone();
            self.add_to_history(track.clone());
            self.update_prefetch_interest();
            return Some(track);
        }
        None
    }

    pub fn queue_track(&mut self, track: Track) {
        let insert_at = if self.queue.is_empty() {
            0
        } else {
            self.current_track_index + 1
        };
        if insert_at <= self.queue.len() {
            self.queue.insert(insert_at, track);
            if self.is_shuffled && insert_at < self.shuffled_index_map.len() {
                self.shuffled_index_map.insert(insert_at, None);
            }
        }
        self.update_prefetch_interest();
    }

    pub fn play_next(&mut self, track: Track) {
        self.queue_track(track);
    }

    fn trigger_fetch(&mut self) {
        if self.fetch_task.is_some() {
            return;
        }

        let api = self.api.clone();
        let event_tx = self.event_tx.clone();

        if !self.pending_track_ids.is_empty() {
            let count = FETCH_BATCH_SIZE.min(self.pending_track_ids.len());
            let ids: Vec<String> = self.pending_track_ids.drain(0..count).collect();

            self.fetch_task = Some(tokio::spawn(async move {
                match api.fetch_tracks_by_ids(ids).await {
                    Ok(tracks) => {
                        let valid: Vec<Track> = tracks
                            .into_iter()
                            .filter(|t| t.available.unwrap_or(false))
                            .collect();
                        if !valid.is_empty() {
                            if let Some(tx) = event_tx {
                                let _ = tx.send(Event::QueueUpdated);
                            }
                        }
                        (valid, None)
                    }
                    Err(e) => {
                        error!(error = %e, "track_fetch_failed");
                        (vec![], None)
                    }
                }
            }));
            return;
        }

        if let Some(session) = &self.wave_session {
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

            self.fetch_task = Some(tokio::spawn(async move {
                match api.get_session_tracks(session_id, queue_history).await {
                    Ok(response) => {
                        let new_tracks: Vec<Track> = response
                            .sequence
                            .iter()
                            .map(|item| item.track.clone())
                            .collect();
                        if !new_tracks.is_empty() {
                            if let Some(tx) = event_tx {
                                let _ = tx.send(Event::QueueUpdated);
                            }
                        }
                        (new_tracks, Some(response))
                    }
                    Err(e) => {
                        error!(error = %e, "wave_fetch_failed");
                        (vec![], None)
                    }
                }
            }));
        }
    }

    pub async fn poll_fetch(&mut self) {
        if let Some(task) = &self.fetch_task {
            if task.is_finished() {
                self.await_fetch().await;
            }
        }
    }

    async fn await_fetch(&mut self) -> bool {
        if let Some(task) = self.fetch_task.take() {
            if let Ok((tracks, session)) = task.await {
                if !tracks.is_empty() {
                    self.queue.extend(tracks);
                    if let Some(s) = session {
                        self.wave_session = Some(s);
                    }
                    self.update_prefetch_interest();
                    return true;
                }
            }
        }
        false
    }

    fn update_prefetch_interest(&self) {
        if self.queue.is_empty() {
            return;
        }

        let mut needed = Vec::new();
        let current_id = self
            .queue
            .get(self.current_track_index)
            .map(|t| t.id.clone());

        for i in 0..URL_PREFETCH_WINDOW {
            if let Some(track) = self.queue.get(self.current_track_index + i) {
                needed.push(track.id.clone());
            }
        }

        if let Some(next_track) = self.queue.get(self.current_track_index + 1) {
            self.stream_manager.prewarm(next_track.clone());
        }

        self.url_prefetcher.update(needed, current_id);
    }

    fn add_to_history(&mut self, track: Track) {
        if self.history_index < self.history.len() {
            self.history.truncate(self.history_index + 1);
        }
        self.history.push(track);
        self.history_index = self.history.len().saturating_sub(1);
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

                self.queue.insert(0, current_track);
                indices.insert(0, current_index);
                self.current_track_index = 0;
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
        } else {
            if let Some(original_queue) = self.original_queue.take() {
                let original_idx = self
                    .shuffled_index_map
                    .get(self.current_track_index)
                    .and_then(|i| *i);

                self.queue = original_queue;
                self.shuffled_index_map.clear();

                if let Some(idx) = original_idx {
                    self.current_track_index = idx;
                } else {
                    self.current_track_index = 0;
                }
            }
        }
        self.update_prefetch_interest();
    }
}
