use super::enums::RepeatMode;
use super::signals::AudioSignals;
use crate::audio::cache::UrlCache;
use crate::audio::stream_manager::StreamManager;
use crate::event::events::Event;
use crate::framework::signals::Signal;
use crate::http::ApiService;
use crate::util::track::extract_ids;
use flume::Sender;
use im::Vector;
use rand::{rng, seq::SliceRandom};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::{error, warn};

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
                                let should_abort = if current_task.is_some() {
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
                                        task.abort();
                                    }
                                    current_task_ids.clear();
                                }
                                pending_ids.clear();
                                for id in needed_ids {
                                    if url_cache.get(&id).is_none() && !current_task_ids.contains(&id) {
                                        pending_ids.push_back(id);
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

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackContext {
    Playlist(Playlist),
    Artist(Artist),
    Album(Album),
    Track(Track),
    Wave(Session),
    Standalone,
}

struct ShuffleState {
    original_queue: Option<Vector<Track>>,
    index_map: Vec<Option<usize>>,
    is_active: bool,
}

impl ShuffleState {
    fn inactive() -> Self {
        Self {
            original_queue: None,
            index_map: Vec::new(),
            is_active: false,
        }
    }

    fn reset(&mut self) {
        self.original_queue = None;
        self.index_map.clear();
        self.is_active = false;
    }

    fn enable(&mut self, queue: Vector<Track>, current_index: usize) -> (Vector<Track>, usize) {
        debug_assert!(!self.is_active, "enable called while already shuffled");

        self.original_queue = Some(queue.clone());

        let mut indices: Vec<Option<usize>> = (0..queue.len()).map(Some).collect();
        let mut queue_vec: Vec<Track> = queue.into_iter().collect();

        if !queue_vec.is_empty() && current_index < queue_vec.len() {
            let current_track = queue_vec.remove(current_index);
            let current_index_val = indices.remove(current_index);

            let mut rest: Vec<(Track, Option<usize>)> =
                queue_vec.into_iter().zip(indices).collect();
            rest.shuffle(&mut rng());

            let mut new_queue_vec = Vec::with_capacity(rest.len() + 1);
            let mut new_indices = Vec::with_capacity(rest.len() + 1);
            new_queue_vec.push(current_track);
            new_indices.push(current_index_val);
            for (t, i) in rest {
                new_queue_vec.push(t);
                new_indices.push(i);
            }

            self.index_map = new_indices;
            self.is_active = true;
            (Vector::from(new_queue_vec), 0)
        } else {
            let mut combined: Vec<(Track, Option<usize>)> =
                queue_vec.into_iter().zip(indices).collect();
            combined.shuffle(&mut rng());

            let (new_queue_vec, new_indices): (Vec<_>, Vec<_>) = combined.into_iter().unzip();
            self.index_map = new_indices;
            self.is_active = true;
            (Vector::from(new_queue_vec), 0)
        }
    }

    fn disable(&mut self, current_shuffled_index: usize) -> Option<(Vector<Track>, usize)> {
        debug_assert!(self.is_active, "disable called while not shuffled");

        let original_queue = self.original_queue.take()?;
        let restored_index = self
            .index_map
            .get(current_shuffled_index)
            .and_then(|i| *i)
            .unwrap_or(0);

        self.index_map.clear();
        self.is_active = false;
        Some((original_queue, restored_index))
    }

    fn record_inserted(&mut self, at: usize) {
        if self.is_active && at <= self.index_map.len() {
            self.index_map.insert(at, None);
        }
    }
}

struct HistoryState {
    entries: Vector<Track>,
    cursor: usize,
}

impl HistoryState {
    fn empty() -> Self {
        Self {
            entries: Vector::new(),
            cursor: 0,
        }
    }

    fn reset(&mut self) {
        self.entries = Vector::new();
        self.cursor = 0;
    }

    fn push(&mut self, track: Track) {
        if !self.entries.is_empty() && self.cursor < self.entries.len() {
            self.entries.truncate(self.cursor + 1);
        }
        self.entries.push_back(track);
        self.cursor = self.entries.len().saturating_sub(1);
    }

    fn as_vector(&self) -> Vector<Track> {
        self.entries.clone()
    }
}

struct FetchState {
    task: Option<JoinHandle<(Vec<Track>, Option<Session>)>>,
    pending_track_ids: Vec<String>,
    wave_session: Arc<Mutex<Option<Session>>>,
}

impl FetchState {
    fn new() -> Self {
        Self {
            task: None,
            pending_track_ids: Vec::new(),
            wave_session: Arc::new(Mutex::new(None)),
        }
    }

    fn reset(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.pending_track_ids.clear();
        *self.wave_session.lock().unwrap() = None;
    }

    fn set_pending_ids(&mut self, ids: Vec<String>) {
        debug_assert!(
            self.pending_track_ids.is_empty(),
            "set_pending_ids called with non-empty list; call reset() first"
        );
        self.pending_track_ids = ids;
    }

    fn is_fetching(&self) -> bool {
        self.task.is_some()
    }

    fn is_finished(&self) -> bool {
        self.task.as_ref().map(|t| t.is_finished()).unwrap_or(false)
    }

    fn set_wave_session(&self, session: Session) {
        *self.wave_session.lock().unwrap() = Some(session);
    }

    fn wave_session_clone(&self) -> Option<Session> {
        self.wave_session.lock().unwrap().clone()
    }

    fn wave_session_arc(&self) -> Arc<Mutex<Option<Session>>> {
        self.wave_session.clone()
    }

    fn trigger_playlist_batch(&mut self, api: Arc<ApiService>, event_tx: Option<Sender<Event>>) {
        debug_assert!(!self.is_fetching());
        let count = FETCH_BATCH_SIZE.min(self.pending_track_ids.len());
        let ids: Vec<String> = self.pending_track_ids.drain(0..count).collect();

        self.task = Some(tokio::spawn(async move {
            match api.fetch_tracks_by_ids(ids).await {
                Ok(tracks) => {
                    let valid: Vec<Track> = tracks
                        .into_iter()
                        .filter(|t| t.available.unwrap_or(false))
                        .collect();
                    if !valid.is_empty()
                        && let Some(tx) = event_tx
                    {
                        let _ = tx.send(Event::QueueUpdated);
                    }
                    (valid, None)
                }
                Err(e) => {
                    error!(error = %e, "track_fetch_failed");
                    (vec![], None)
                }
            }
        }));
    }

    fn trigger_wave_batch(
        &mut self,
        api: Arc<ApiService>,
        event_tx: Option<Sender<Event>>,
        history_seeds: Vec<String>,
    ) {
        debug_assert!(!self.is_fetching());
        let session = match self.wave_session_clone() {
            Some(s) => s,
            None => return,
        };
        let session_id = session
            .radio_session_id
            .clone()
            .unwrap_or(session.batch_id.clone());

        self.task = Some(tokio::spawn(async move {
            match api.get_session_tracks(session_id, history_seeds).await {
                Ok(response) => {
                    let new_tracks: Vec<Track> = response
                        .sequence
                        .iter()
                        .map(|item| item.track.clone())
                        .collect();
                    if !new_tracks.is_empty()
                        && let Some(tx) = event_tx
                    {
                        let _ = tx.send(Event::QueueUpdated);
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

    async fn await_task(&mut self) -> Option<(Vec<Track>, Option<Session>)> {
        let task = self.task.take()?;
        (task.await).ok()
    }
}

struct WaveExtensionHandles {
    queue: Signal<Vector<Track>>,
    queue_length: Signal<usize>,
    wave_session: Arc<Mutex<Option<Session>>>,
    event_tx: Option<Sender<Event>>,
}

impl WaveExtensionHandles {
    fn apply(self, additional: Vector<Track>, session: Session) {
        *self.wave_session.lock().unwrap() = Some(session);

        self.queue.update(|q| q.extend(additional));
        self.queue_length
            .set(self.queue.with(|q: &Vector<Track>| q.len()));

        if let Some(tx) = self.event_tx {
            let _ = tx.send(Event::QueueUpdated);
        }
    }
}

struct PlaybackPolicy;

impl PlaybackPolicy {
    fn try_advance(current: usize, queue_len: usize) -> Option<usize> {
        let next = current + 1;
        if next < queue_len { Some(next) } else { None }
    }

    fn repeat_wrap_index(repeat: RepeatMode, queue_len: usize) -> Option<usize> {
        if repeat == RepeatMode::All && queue_len > 0 {
            Some(0)
        } else {
            None
        }
    }

    fn prev_index(current: usize, queue_len: usize, repeat: RepeatMode) -> Option<usize> {
        if current > 0 {
            Some(current - 1)
        } else if repeat == RepeatMode::All && queue_len > 0 {
            Some(queue_len - 1)
        } else {
            None
        }
    }
}

struct QueueSignals {
    inner: AudioSignals,
}

impl QueueSignals {
    fn new(inner: AudioSignals) -> Self {
        inner.set_queue(Vector::new(), Vector::new(), 0);
        inner.set_modes(RepeatMode::None, false);
        Self { inner }
    }

    fn queue(&self) -> Vector<Track> {
        self.inner.queue.with(|q| q.clone())
    }

    fn index(&self) -> usize {
        self.inner.queue_index.get()
    }

    fn repeat_mode(&self) -> RepeatMode {
        self.inner.repeat_mode.get()
    }

    fn is_shuffled(&self) -> bool {
        self.inner.is_shuffled.get()
    }

    fn write_queue(&self, queue: Vector<Track>) {
        let len = queue.len();
        self.inner.queue.set(queue);
        self.inner.queue_length.set(len);
    }

    fn write_history(&self, history: Vector<Track>) {
        self.inner.history.set(history);
    }

    fn write_index(&self, index: usize) {
        self.inner.queue_index.set(index);
    }

    fn write_repeat_mode(&self, mode: RepeatMode) {
        self.inner.repeat_mode.set(mode);
    }

    fn write_shuffled(&self, shuffled: bool) {
        self.inner.is_shuffled.set(shuffled);
    }

    fn raw_queue_handle(&self) -> Signal<Vector<Track>> {
        self.inner.queue.clone()
    }

    fn raw_queue_length_handle(&self) -> Signal<usize> {
        self.inner.queue_length.clone()
    }
}

pub struct QueueManager {
    pub api: Arc<ApiService>,
    pub url_cache: UrlCache,
    pub stream_manager: Arc<StreamManager>,
    url_prefetcher: UrlPrefetcher,

    signals: QueueSignals,

    pub playback_context: PlaybackContext,

    shuffle: ShuffleState,

    history: HistoryState,

    fetch: FetchState,

    pub event_tx: Option<Sender<Event>>,
}

impl QueueManager {
    pub fn new(
        api: Arc<ApiService>,
        url_cache: UrlCache,
        stream_manager: Arc<StreamManager>,
        signals: AudioSignals,
    ) -> Self {
        let url_prefetcher = UrlPrefetcher::new(api.clone(), url_cache.clone());

        Self {
            api,
            url_cache,
            stream_manager,
            url_prefetcher,
            signals: QueueSignals::new(signals),
            playback_context: PlaybackContext::Standalone,
            shuffle: ShuffleState::inactive(),
            history: HistoryState::empty(),
            fetch: FetchState::new(),
            event_tx: None,
        }
    }

    pub fn set_event_tx(&mut self, tx: Sender<Event>) {
        self.event_tx = Some(tx);
    }

    pub async fn load(
        &mut self,
        context: PlaybackContext,
        mut tracks: Vector<Track>,
        mut start_index: usize,
    ) -> Option<Track> {
        self.fetch.reset();
        self.url_prefetcher.reset();

        self.playback_context = context;
        self.shuffle.reset();
        self.history.reset();
        self.signals.write_history(Vector::new());
        self.signals.write_shuffled(false);

        match self.playback_context {
            PlaybackContext::Playlist(ref playlist) => {
                let all_track_ids = playlist
                    .tracks
                    .as_ref()
                    .map(extract_ids)
                    .unwrap_or_default();

                let loaded_count = (start_index + tracks.len()).min(all_track_ids.len());
                self.fetch
                    .set_pending_ids(all_track_ids.into_iter().skip(loaded_count).collect());

                if start_index >= tracks.len() {
                    start_index = 0;
                }
                tracks = slice_from(tracks, start_index);
                self.signals.write_queue(tracks);
            }

            PlaybackContext::Artist(_)
            | PlaybackContext::Album(_)
            | PlaybackContext::Standalone => {
                if start_index >= tracks.len() {
                    start_index = 0;
                }
                tracks = slice_from(tracks, start_index);
                self.signals.write_queue(tracks);
            }

            PlaybackContext::Wave(ref session) => {
                if start_index >= tracks.len() {
                    start_index = 0;
                }
                tracks = slice_from(tracks, start_index);
                self.signals.write_queue(tracks);
                self.fetch.set_wave_session(session.clone());
            }

            PlaybackContext::Track(ref seed_track) => {
                let mut initial_queue = Vector::new();
                initial_queue.push_back(seed_track.clone());
                self.signals.write_queue(initial_queue);

                if seed_track.track_source.as_ref().is_none_or(|s| s != "UGC") {
                    self.spawn_wave_init_for_seed(seed_track);
                }
            }
        }

        self.signals.write_index(0);

        let track = self.signals.queue().get(0).cloned();
        if let Some(t) = &track {
            self.commit_track_to_history(t.clone());
            self.update_prefetch_interest();
        }
        track
    }

    fn spawn_wave_init_for_seed(&self, seed_track: &Track) {
        let track_id = seed_track.id.clone();
        let album_id = seed_track
            .albums
            .first()
            .and_then(|a| a.id.as_ref().map(|id| id.to_string()));

        let Some(album_id) = album_id else { return };

        let api = self.api.clone();
        let handles = WaveExtensionHandles {
            queue: self.signals.raw_queue_handle(),
            queue_length: self.signals.raw_queue_length_handle(),
            wave_session: self.fetch.wave_session_arc(),
            event_tx: self.event_tx.clone(),
        };

        tokio::spawn(async move {
            let Ok(session) = api.create_session(vec![format!("track:{track_id}")]).await else {
                return;
            };
            let seed = format!("{track_id}:{album_id}");
            let Ok(new_tracks) = api
                .get_session_tracks(session.batch_id.clone(), vec![seed])
                .await
            else {
                return;
            };

            let additional: Vector<Track> = new_tracks
                .sequence
                .iter()
                .map(|s| s.track.clone())
                .collect();

            if !additional.is_empty() {
                handles.apply(additional, session);
            }
        });
    }

    pub async fn get_next_track(&mut self) -> Option<Track> {
        if self.signals.queue().is_empty() {
            return None;
        }

        if self.signals.repeat_mode() == RepeatMode::Single {
            return self.signals.queue().get(self.signals.index()).cloned();
        }

        self.poll_fetch().await;

        let current = self.signals.index();
        let queue_len = self.signals.queue().len();

        if current + 1 + FETCH_THRESHOLD >= queue_len {
            self.trigger_fetch();
        }

        if let Some(next) = PlaybackPolicy::try_advance(current, queue_len) {
            return self.advance_to(next);
        }

        let next = current + 1;
        if self.fetch.is_fetching()
            && let Some((new_tracks, session)) = self.fetch.await_task().await
            && !new_tracks.is_empty()
        {
            let mut q = self.signals.queue();
            q.extend(new_tracks);
            self.signals.write_queue(q);
            if let Some(s) = session {
                self.fetch.set_wave_session(s);
            }
            if next < self.signals.queue().len() {
                return self.advance_to(next);
            }
        }

        if let Some(wrap) = PlaybackPolicy::repeat_wrap_index(
            self.signals.repeat_mode(),
            self.signals.queue().len(),
        ) {
            return self.advance_to(wrap);
        }

        None
    }

    pub fn get_previous_track(&mut self) -> Option<Track> {
        let prev = PlaybackPolicy::prev_index(
            self.signals.index(),
            self.signals.queue().len(),
            self.signals.repeat_mode(),
        )?;
        self.advance_to(prev)
    }

    pub async fn play_track_at_index(&mut self, index: usize) -> Option<Track> {
        self.poll_fetch().await;
        if index >= self.signals.queue().len() {
            return None;
        }
        self.advance_to(index)
    }

    fn advance_to(&mut self, index: usize) -> Option<Track> {
        self.signals.write_index(index);
        let track = self.signals.queue().get(index).cloned()?;
        self.commit_track_to_history(track.clone());
        self.update_prefetch_interest();
        Some(track)
    }

    pub fn queue_track(&mut self, track: Track) {
        let mut queue = self.signals.queue();
        let current_index = self.signals.index();

        let insert_at = if queue.is_empty() {
            0
        } else {
            current_index + 1
        };

        if insert_at <= queue.len() {
            queue.insert(insert_at, track);
            self.signals.write_queue(queue);
            self.shuffle.record_inserted(insert_at);
        }
        self.update_prefetch_interest();
    }

    pub fn play_next(&mut self, track: Track) {
        self.queue_track(track);
    }

    pub fn remove_track(&mut self, index: usize) {
        let mut queue = self.signals.queue();
        if index < queue.len() {
            queue.remove(index);
            self.signals.write_queue(queue);

            let current_index = self.signals.index();
            if index < current_index {
                self.signals.write_index(current_index.saturating_sub(1));
            } else if index == current_index {
            }
            self.update_prefetch_interest();
        }
    }

    pub fn clear(&mut self) {
        self.signals.write_queue(Vector::new());
        self.signals.write_index(0);
        self.update_prefetch_interest();
    }

    pub fn trigger_fetch_if_needed(&mut self) {
        self.trigger_fetch();
    }

    fn trigger_fetch(&mut self) {
        if self.fetch.is_fetching() {
            return;
        }

        if !self.fetch.pending_track_ids.is_empty() {
            self.fetch
                .trigger_playlist_batch(self.api.clone(), self.event_tx.clone());
            return;
        }

        if self.fetch.wave_session_clone().is_some() {
            let history_seeds = self.build_wave_history_seeds();
            self.fetch
                .trigger_wave_batch(self.api.clone(), self.event_tx.clone(), history_seeds);
        }
    }

    fn build_wave_history_seeds(&self) -> Vec<String> {
        self.history
            .entries
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
            .collect()
    }

    pub async fn poll_fetch(&mut self) {
        if self.fetch.is_finished() {
            self.consume_fetch_result().await;
        }
    }

    async fn consume_fetch_result(&mut self) -> bool {
        let Some((tracks, session)) = self.fetch.await_task().await else {
            return false;
        };

        if tracks.is_empty() {
            return false;
        }

        let mut queue = self.signals.queue();
        queue.extend(tracks);
        self.signals.write_queue(queue);

        if let Some(s) = session {
            self.fetch.set_wave_session(s);
        }
        self.update_prefetch_interest();
        true
    }

    fn update_prefetch_interest(&self) {
        let queue = self.signals.queue();
        if queue.is_empty() {
            return;
        }

        let current_index = self.signals.index();
        let current_id = queue.get(current_index).map(|t| t.id.clone());

        let needed: Vec<String> = (0..URL_PREFETCH_WINDOW)
            .filter_map(|i| queue.get(current_index + i))
            .map(|t| t.id.clone())
            .collect();

        if let Some(next_track) = queue.get(current_index + 1) {
            self.stream_manager.prewarm(next_track.clone());
        }

        self.url_prefetcher.update(needed, current_id);
    }

    fn commit_track_to_history(&mut self, track: Track) {
        self.history.push(track);
        self.signals.write_history(self.history.as_vector());
    }

    pub fn toggle_repeat_mode(&mut self) {
        let new_mode = match self.signals.repeat_mode() {
            RepeatMode::None => RepeatMode::All,
            RepeatMode::All => RepeatMode::Single,
            RepeatMode::Single => RepeatMode::None,
        };
        self.signals.write_repeat_mode(new_mode);
    }

    pub fn toggle_shuffle(&mut self) {
        if self.signals.is_shuffled() {
            let current_index = self.signals.index();
            if let Some((original_queue, restored_index)) = self.shuffle.disable(current_index) {
                self.signals.write_queue(original_queue);
                self.signals.write_index(restored_index);
            }
            self.signals.write_shuffled(false);
        } else {
            let queue = self.signals.queue();
            let current_index = self.signals.index();
            let (shuffled_queue, new_index) = self.shuffle.enable(queue, current_index);
            self.signals.write_queue(shuffled_queue);
            self.signals.write_index(new_index);
            self.signals.write_shuffled(true);
        }
        self.update_prefetch_interest();
    }
}

fn slice_from(mut v: Vector<Track>, start: usize) -> Vector<Track> {
    if start == 0 {
        v
    } else if start < v.len() {
        v.split_off(start)
    } else {
        Vector::new()
    }
}
