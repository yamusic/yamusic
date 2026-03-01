use std::{io::Write, sync::Arc};

use flume::{Receiver, Sender};
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::{Constraint, Direction, Layout},
};
use tokio::sync::RwLock;

use crate::{
    audio::{queue::PlaybackContext, system::AudioSystem},
    event::events::Event,
    http::ApiService,
};
use im::Vector;

use super::{
    actions::{Action, Route},
    components::{
        Lyrics, PlayerBar, PlayerSignals, Sidebar, ToastManager, Visualizer, tick_global,
    },
    data::{
        AlbumTracksSource, ArtistTracksSource, LikedTracksSource, PlaylistDataSource,
        PlaylistTracksSource,
    },
    keymap::{
        Intent, Key, KeyResolver, NavigationIntent, PlaybackIntent, QueueIntent, Target,
        ViewIntent, normalize,
    },
    signals::{AppSignals, LibrarySignals, LyricsSignals, NavigationSignals},
    state::{SearchState, WaveSessionState},
    terminal::{Terminal, TerminalEvent, TickRate},
    views::{
        HomeView, OverlayRenderer, PlaylistListView, SearchView, TrackListContext, TrackListView,
    },
};
use crate::framework::component::Component;
use crate::framework::reactive::{Signal, With, memo};
use crate::framework::theme::{Theme, ThemeStyles};

pub struct App {
    signals: Arc<AppSignals>,
    audio: Arc<RwLock<AudioSystem>>,
    api: Arc<ApiService>,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
    player_bar: PlayerBar,
    visualizer: Visualizer,
    lyrics: Lyrics,
    theme: Signal<ThemeStyles>,
    sidebar: Sidebar,
    sidebar_visible: bool,
    should_quit: bool,

    search_state: SearchState,
    wave_state: WaveSessionState,

    home_view: HomeView,
    playlist_list_view: Option<PlaylistListView>,
    liked_view: Option<TrackListView>,
    search_view: SearchView,
    track_list_view: Option<TrackListView>,

    current_route: Route,

    key_resolver: KeyResolver,

    toast_manager: ToastManager,
}

impl App {
    pub async fn new(
        audio: AudioSystem,
        api: Arc<ApiService>,
        event_tx: Sender<Event>,
        event_rx: Receiver<Event>,
    ) -> color_eyre::Result<Self> {
        let audio_signals = audio.signals().clone();

        let audio = Arc::new(RwLock::new(audio));

        let lyrics_signals = LyricsSignals::new(api.clone(), &audio_signals);

        let signals = Arc::new(AppSignals {
            audio: audio_signals,
            navigation: NavigationSignals::new(),
            library: LibrarySignals::new(api.clone()),
            lyrics: lyrics_signals.clone(),
            theme: Arc::new(Theme::default()),
            is_focused: crate::framework::reactive::signal(true),
        });

        let theme_styles = signals.theme.styles().clone();

        let is_current_liked = memo({
            let track_id = signals.audio.current_track_id.clone();
            let liked_ids = signals.library.liked_track_ids.clone();
            move |_| {
                if let Some(id) = track_id.get() {
                    With::with(&liked_ids, |ids| ids.contains(&id))
                } else {
                    false
                }
            }
        });

        let is_current_disliked = memo({
            let track_id = signals.audio.current_track_id.clone();
            let disliked_ids = signals.library.disliked_track_ids.clone();
            move |_| {
                if let Some(id) = track_id.get() {
                    With::with(&disliked_ids, |ids| ids.contains(&id))
                } else {
                    false
                }
            }
        });

        let player_signals = PlayerSignals {
            track_title: signals.audio.track_title.clone(),
            track_artists: signals.audio.track_artists.clone(),
            is_playing: signals.audio.is_playing.clone(),
            is_liked: is_current_liked.0.clone(),
            is_disliked: is_current_disliked.0.clone(),
            position_ms: signals.audio.position_ms.clone(),
            duration_ms: signals.audio.duration_ms.clone(),
            buffered_ratio: signals.audio.buffered_ratio.clone(),
            volume: signals.audio.volume.clone(),
            is_muted: signals.audio.is_muted.clone(),
            is_shuffled: signals.audio.is_shuffled.clone(),
            repeat_mode: signals.audio.repeat_mode.clone(),
        };
        let player_bar = PlayerBar::new(player_signals, theme_styles.clone());

        let visualizer = Visualizer::new(
            signals.audio.amplitude.clone(),
            signals.audio.is_playing.clone(),
            signals.audio.current_track.clone(),
            theme_styles.clone(),
        );

        let lyrics = Lyrics::new(signals.lyrics.clone(), signals.audio.position_ms.clone());

        let wave_state = WaveSessionState::new(api.clone(), event_tx.clone());
        let search_state = SearchState::new();

        let api_clone = api.clone();
        let event_tx_clone = event_tx.clone();
        let audio_clone = audio.clone();
        tokio::spawn(async move {
            let audio_guard = audio_clone.read().await;
            let state = audio_guard.state_handle();
            drop(audio_guard);

            AudioSystem::sync_liked_collection_with(api_clone, state.clone()).await;
            if let Ok(state_guard) = state.try_read() {
                let _ =
                    event_tx_clone.send(Event::LikedStatusUpdated(state_guard.liked.snapshot()));
            }
        });

        let wave_state_waves = wave_state.waves.clone();
        let wave_state_loading = wave_state.is_loading.clone();

        Ok(Self {
            signals: signals.clone(),
            audio,
            api: api.clone(),
            event_tx: event_tx.clone(),
            event_rx,
            player_bar,
            sidebar: Sidebar::new(theme_styles.clone()),
            sidebar_visible: true,
            should_quit: false,
            search_state,
            wave_state,
            home_view: HomeView::new(wave_state_waves, wave_state_loading, theme_styles.clone()),
            playlist_list_view: None,
            liked_view: None,
            search_view: SearchView::new(&signals, theme_styles.clone()),
            track_list_view: None,
            current_route: Route::Home,
            key_resolver: KeyResolver::new(),
            visualizer,
            lyrics,
            theme: theme_styles.clone(),
            toast_manager: ToastManager::new(theme_styles),
        })
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn update_bridge_state(&self) {
        let should_enable = self.signals.is_focused.get() && self.current_route == Route::Home;
        self.signals.audio.bridge.set_enabled(should_enable);
        self.signals
            .audio
            .bridge
            .set_focused(self.signals.is_focused.get());
    }

    pub fn signals(&self) -> &Arc<AppSignals> {
        &self.signals
    }

    pub async fn process_event(&mut self, event: Event) {
        match event {
            Event::TrackStarted(_, _) | Event::PlaybackProgress(_) => {}
            Event::QueueUpdated => {
                self.audio.write().await.sync_queue().await;
            }
            Event::TrackEnded => {
                let audio = self.audio.clone();
                tokio::spawn(async move {
                    let mut audio = audio.write().await;
                    audio.play_next().await;
                });
            }
            Event::LikedStatusUpdated(snapshot) => {
                let (liked, disliked) = snapshot;
                self.signals.library.set_liked_snapshot(liked, disliked);
            }
            Event::PlaylistsFetched(_)
            | Event::PlaylistFetched(_)
            | Event::PlaylistTracksFetched(_, _)
            | Event::PlaylistTracksPageFetched(_, _, _) => {}
            Event::SearchResults(results) => {
                let optimal_tab = self.search_state.apply_results(results);
                self.search_view.apply(
                    self.search_state.tracks(),
                    self.search_state.albums(),
                    self.search_state.artists(),
                    self.search_state.playlists(),
                    optimal_tab,
                );
            }
            Event::SearchPageFetched(results, page) => {
                self.search_state.merge_results(results, page);
                self.search_view.apply_merged(
                    self.search_state.tracks(),
                    self.search_state.albums(),
                    self.search_state.artists(),
                    self.search_state.playlists(),
                );
            }
            Event::FetchError(_) => {
                self.search_state.is_loading = false;
                self.search_view.set_loading(false);
            }
            Event::WaveReady(session, tracks) => {
                let audio = self.audio.clone();
                tokio::spawn(async move {
                    let mut audio = audio.write().await;
                    audio
                        .load_context(PlaybackContext::Wave(session), Vector::from(tracks), 0)
                        .await;
                });
            }
            _ => {}
        }
    }

    pub async fn process_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Redraw => {}
            Action::Quit => {
                self.should_quit = true;
            }
            Action::Navigate(route) => {
                self.navigate(route).await;
            }
            Action::Back => {
                self.signals.navigation.back();
                self.current_route = self.signals.navigation.current_route.get();
            }
            Action::Overlay(route) => {
                self.signals.navigation.show_overlay(route);
            }
            Action::DismissOverlay => {
                self.signals.navigation.dismiss_overlay();
            }
            Action::PlayContext {
                context,
                tracks,
                start_index,
            } => {
                let mut audio = self.audio.write().await;
                audio.load_context(context, tracks, start_index).await;
            }
            Action::PlayTrack(track) => {
                let mut audio = self.audio.write().await;
                audio.play_single_track(track).await;
            }
            Action::TogglePlayback => {
                let mut audio = self.audio.write().await;
                audio.play_pause().await;
            }
            Action::NextTrack => {
                let mut audio = self.audio.write().await;
                audio.play_next().await;
            }
            Action::PreviousTrack => {
                let mut audio = self.audio.write().await;
                audio.play_previous().await;
            }
            Action::SeekForward(secs) => {
                let mut audio = self.audio.write().await;
                audio.seek_forwards(secs).await;
            }
            Action::SeekBackward(secs) => {
                let mut audio = self.audio.write().await;
                audio.seek_backwards(secs).await;
            }
            Action::SetVolume(vol) => {
                let mut audio = self.audio.write().await;
                audio.set_volume(vol);
            }
            Action::ToggleMute => {
                let mut audio = self.audio.write().await;
                audio.toggle_mute();
            }
            Action::ToggleShuffle => {
                let mut audio = self.audio.write().await;
                audio.toggle_shuffle();
            }
            Action::CycleRepeat => {
                let mut audio = self.audio.write().await;
                audio.toggle_repeat_mode();
            }
            Action::QueueTrack(track) => {
                let mut audio = self.audio.write().await;
                let title = track
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown Track".to_string());
                audio.queue_track(track);
                self.toast_manager.push_line(
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::raw("Queued: "),
                        ratatui::text::Span::styled(
                            title,
                            ratatui::style::Style::default()
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        ),
                    ]),
                    Some("󰐍".to_string()),
                );
            }
            Action::PlayNext(track) => {
                let mut audio = self.audio.write().await;
                let title = track
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown Track".to_string());
                audio.play_track_next(track);
                self.toast_manager.push_line(
                    ratatui::text::Line::from(vec![
                        ratatui::text::Span::raw("Next: "),
                        ratatui::text::Span::styled(
                            title,
                            ratatui::style::Style::default()
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        ),
                    ]),
                    Some("󰐊".to_string()),
                );
            }
            Action::RemoveFromQueue(idx) => {
                let mut audio = self.audio.write().await;
                audio.remove_from_queue(idx);
                self.toast_manager
                    .push_with_icon("Removed from queue".to_string(), Some("󰛌".to_string()));
            }
            Action::ClearQueue => {
                let mut audio = self.audio.write().await;
                audio.clear_queue();
                self.toast_manager
                    .push_with_icon("Queue cleared".to_string(), Some("󰛌".to_string()));
            }
            Action::LikeContext => {
                let api = self.api.clone();
                let audio_sys = self.audio.clone();
                match &self.current_route {
                    Route::Album { id, title } => {
                        let album_id = id.parse::<u32>().unwrap_or(0);
                        let title = title.clone();
                        let audio = self.audio.clone();
                        tokio::spawn(async move {
                            let is_liked = audio.read().await.is_album_liked(album_id).await;
                            if is_liked {
                                let _ = api.remove_like_album(album_id).await;
                            } else {
                                let _ = api.add_like_album(album_id).await;
                            }
                            audio.write().await.sync_liked_collection().await;
                        });

                        let is_liked = audio_sys.read().await.is_album_liked(album_id).await;
                        if is_liked {
                            self.toast_manager.push_with_icon(
                                format!("Removed Album: {title}"),
                                Some("󰋕".to_string()),
                            );
                        } else {
                            self.toast_manager.push_with_icon(
                                format!("Liked Album: {title}"),
                                Some("󰋑".to_string()),
                            );
                        }
                    }
                    Route::Artist { id, name } => {
                        let id = id.clone();
                        let name = name.clone();
                        let audio = self.audio.clone();
                        let id_clone = id.clone();
                        tokio::spawn(async move {
                            let is_liked = audio.read().await.is_artist_liked(&id_clone).await;
                            if is_liked {
                                let _ = api.remove_like_artist(id_clone).await;
                            } else {
                                let _ = api.add_like_artist(id_clone).await;
                            }
                            audio.write().await.sync_liked_collection().await;
                        });

                        let is_liked = audio_sys.read().await.is_artist_liked(&id).await;
                        if is_liked {
                            self.toast_manager.push_with_icon(
                                format!("Unfollowed: {name}"),
                                Some("󰋕".to_string()),
                            );
                        } else {
                            self.toast_manager
                                .push_with_icon(format!("Followed: {name}"), Some("󰋑".to_string()));
                        }
                    }
                    Route::Playlist { kind, title } => {
                        let kind = *kind;
                        let title = title.clone();
                        let uid = api.current_user_id();
                        let audio = self.audio.clone();
                        tokio::spawn(async move {
                            let is_liked = audio.read().await.is_playlist_liked(uid, kind).await;
                            if is_liked {
                                let _ = api.remove_like_playlist(uid, kind).await;
                            } else {
                                let _ = api.add_like_playlist(uid, kind).await;
                            }
                            audio.write().await.sync_liked_collection().await;
                        });

                        let is_liked = audio_sys.read().await.is_playlist_liked(uid, kind).await;
                        if is_liked {
                            self.toast_manager.push_with_icon(
                                format!("Removed Playlist: {title}"),
                                Some("󰋕".to_string()),
                            );
                        } else {
                            self.toast_manager.push_with_icon(
                                format!("Liked Playlist: {title}"),
                                Some("󰋑".to_string()),
                            );
                        }
                    }
                    _ => {
                        self.toast_manager
                            .push_with_icon("Nothing to like".to_string(), Some("".to_string()));
                    }
                };
            }
            Action::DislikeContext => {
                let api = self.api.clone();
                let audio_sys = self.audio.clone();
                if let Route::Artist { id, name } = &self.current_route {
                    let id = id.clone();
                    let name = name.clone();
                    let audio = self.audio.clone();
                    let id_clone = id.clone();
                    tokio::spawn(async move {
                        let is_disliked = audio.read().await.is_artist_disliked(&id_clone).await;
                        if is_disliked {
                            let _ = api.remove_dislike_artist(id_clone.clone()).await;
                        } else {
                            let _ = api.add_dislike_artist(id_clone.clone()).await;
                        }
                        audio.write().await.sync_liked_collection().await;
                    });

                    let is_disliked = audio_sys.read().await.is_artist_disliked(&id).await;
                    if is_disliked {
                        self.toast_manager.push_with_icon(
                            format!("Removed dislike: {name}"),
                            Some("󰋖".to_string()),
                        );
                    } else {
                        self.toast_manager
                            .push_with_icon(format!("Disliked: {name}"), Some("󰋖".to_string()));
                    }
                } else {
                    self.toast_manager
                        .push_with_icon("Nothing to dislike".to_string(), Some("".to_string()));
                }
            }
            Action::QueueAll => {
                if let Some(view) = &self.track_list_view {
                    let tracks = view.items();
                    let count = tracks.len();
                    let mut audio = self.audio.write().await;
                    for track in tracks {
                        audio.queue_track(track.clone());
                    }
                    self.toast_manager
                        .push_with_icon(format!("Queued {count} tracks"), Some("󰐍".to_string()));
                }
            }
            Action::PlayAllNext => {
                if let Some(view) = &self.track_list_view {
                    let tracks = view.items();
                    let count = tracks.len();
                    let mut audio = self.audio.write().await;
                    for track in tracks.into_iter().rev() {
                        audio.play_track_next(track.clone());
                    }
                    self.toast_manager
                        .push_with_icon(format!("Next: {count} tracks"), Some("󰐊".to_string()));
                }
            }
            Action::LikeTrack(track) => {
                let id = track.id.clone();
                let title = track
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown Track".to_string());
                let was_liked = self.signals.library.is_liked(&id);
                if was_liked {
                    self.signals.library.remove_like(&id);
                    let api = self.api.clone();
                    let id_clone = id.clone();
                    tokio::spawn(async move {
                        let _ = api.remove_like_track(id_clone).await;
                    });
                    self.toast_manager.push_line(
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::raw("Removed "),
                            ratatui::text::Span::styled(
                                title,
                                ratatui::style::Style::default()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::raw(" from liked"),
                        ]),
                        Some("󰋕".to_string()),
                    );
                } else {
                    self.signals.library.add_like(id.clone());
                    let api = self.api.clone();
                    let id_clone = id.clone();
                    tokio::spawn(async move {
                        let _ = api.add_like_track(id_clone).await;
                    });
                    self.toast_manager.push_line(
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::raw("Added "),
                            ratatui::text::Span::styled(
                                title,
                                ratatui::style::Style::default()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::raw(" to liked"),
                        ]),
                        Some("󰋑".to_string()),
                    );

                    if self.current_route == Route::Home {
                        self.visualizer.trigger_like_glow();
                    }
                }
            }
            Action::DislikeTrack(track) => {
                let id = track.id.clone();
                let title = track
                    .title
                    .clone()
                    .unwrap_or_else(|| "Unknown Track".to_string());
                if !self.signals.library.is_disliked(&id) {
                    self.signals.library.add_dislike(id.clone());

                    if self.signals.library.is_liked(&id) {
                        self.signals.library.remove_like(&id);
                        let api = self.api.clone();
                        let id_clone = id.clone();
                        tokio::spawn(async move {
                            let _ = api.remove_like_track(id_clone).await;
                        });
                    }

                    let api = self.api.clone();
                    let id_for_api = id.clone();
                    tokio::spawn(async move {
                        let _ = api.add_dislike_track(id_for_api).await;
                    });

                    if let Some(current) = self.signals.audio.current_track_id.get()
                        && current == id
                    {
                        let mut audio = self.audio.write().await;
                        audio.play_next().await;
                    }
                    self.toast_manager.push_line(
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::raw("Added "),
                            ratatui::text::Span::styled(
                                title,
                                ratatui::style::Style::default()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::raw(" to disliked"),
                        ]),
                        Some("󰋖".to_string()),
                    );
                } else {
                    self.signals.library.remove_dislike(&id);
                    let api = self.api.clone();
                    let id_for_api = id.clone();
                    tokio::spawn(async move {
                        let _ = api.remove_dislike_track(id_for_api).await;
                    });
                    self.toast_manager.push_line(
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::raw("Removed "),
                            ratatui::text::Span::styled(
                                title,
                                ratatui::style::Style::default()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::raw(" from disliked"),
                        ]),
                        Some("󰋕".to_string()),
                    );
                }
            }
            Action::Toast(msg) => {
                self.toast_manager.push(msg);
            }
            Action::Batch(actions) => {
                for action in actions {
                    Box::pin(self.process_action(action)).await;
                }
            }
            Action::Search(query) => {
                self.search_state.begin_search();
                let api = self.api.clone();
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    match api.search(&query).await {
                        Ok(results) => {
                            let _ = tx.send(Event::SearchResults(results));
                        }
                        Err(e) => {
                            let _ = tx.send(Event::FetchError(e.to_string()));
                        }
                    }
                });
            }
            Action::SearchNextPage => {
                let tab = self.search_view.current_tab();
                let sel = self.search_view.current_selection();
                let count = self.search_view.current_tab_count();
                if self.search_state.should_load_more(tab, sel, count) {
                    self.search_state.is_loading_more = true;
                    self.search_view.set_loading_more(true);
                    let query = self.search_view.query();
                    let page = self.search_state.current_page + 1;
                    let api = self.api.clone();
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        match api.search_paginated(&query, page).await {
                            Ok(results) => {
                                let _ = tx.send(Event::SearchPageFetched(results, page));
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    });
                }
            }
            Action::StartWave {
                seeds,
                title,
                toast_message,
            } => {
                if let Some(lines) = toast_message {
                    self.toast_manager.push_lines(lines, Some("󰎈".to_string()));
                } else {
                    let msg = if let Some(t) = title {
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::raw("Starting a wave for "),
                            ratatui::text::Span::styled(
                                t,
                                ratatui::style::Style::default()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                        ])
                    } else {
                        ratatui::text::Line::from("Starting a new wave".to_string())
                    };
                    self.toast_manager
                        .push_lines(vec![msg], Some("󰎈".to_string()));
                }
                self.wave_state.start_with_seeds(seeds);
            }
            Action::RefreshWaves => {
                self.wave_state.fetch();
            }
            Action::ScrollTop => match &self.current_route {
                Route::Search => self.search_view.scroll_top(),
                Route::Liked => {
                    if let Some(view) = &mut self.liked_view {
                        view.scroll_top();
                    }
                }
                Route::Playlists => {
                    if let Some(view) = &mut self.playlist_list_view {
                        view.scroll_top();
                    }
                }
                Route::Playlist { .. }
                | Route::Album { .. }
                | Route::Artist { .. }
                | Route::Queue => {
                    if let Some(view) = &mut self.track_list_view {
                        view.scroll_top();
                    }
                }
                _ => {}
            },
            Action::ScrollBottom => match &self.current_route {
                Route::Search => self.search_view.scroll_bottom(),
                Route::Liked => {
                    if let Some(view) = &mut self.liked_view {
                        view.scroll_bottom();
                    }
                }
                Route::Playlists => {
                    if let Some(view) = &mut self.playlist_list_view {
                        view.scroll_bottom();
                    }
                }
                Route::Playlist { .. }
                | Route::Album { .. }
                | Route::Artist { .. }
                | Route::Queue => {
                    if let Some(view) = &mut self.track_list_view {
                        view.scroll_bottom();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    async fn navigate(&mut self, route: Route) {
        let is_top_level_nav = is_top_level(&self.current_route) && is_top_level(&route);
        if is_top_level_nav {
            self.signals.navigation.set_route(route.clone());
        } else {
            self.signals.navigation.navigate(route.clone());
        }
        self.current_route = route.clone();

        self.update_bridge_state();

        match &route {
            Route::Liked => {
                if self.liked_view.is_none() {
                    let source = Arc::new(LikedTracksSource::new(self.api.clone()));
                    let playlist_info = source.playlist_info();
                    let context = TrackListContext::Playlist {
                        kind: 3,
                        title: "Liked Tracks".to_string(),
                        owner: String::new(),
                        owner_uid: 0,
                        track_count: 0,
                    };
                    let view = TrackListView::new(context, source.clone(), &self.signals)
                        .with_playlist_info(playlist_info);
                    self.liked_view = Some(view);
                }
            }
            Route::Playlists => {
                if self.playlist_list_view.is_none() {
                    let source = Arc::new(PlaylistDataSource::new(
                        self.signals.library.playlists.clone(),
                    ));
                    self.playlist_list_view =
                        Some(PlaylistListView::new(source.clone(), self.theme.clone()));
                }
            }
            Route::Playlist { kind, title } => {
                let source = Arc::new(PlaylistTracksSource::new(*kind, self.api.clone()));
                let playlist_info = source.playlist_info();

                let context = TrackListContext::Playlist {
                    kind: *kind,
                    title: title.clone(),
                    owner: String::new(),
                    owner_uid: 0,
                    track_count: 0,
                };

                let view = TrackListView::new(context, source.clone(), &self.signals)
                    .with_playlist_info(playlist_info);
                self.track_list_view = Some(view);
            }
            Route::Queue => {
                use crate::app::data::QueueDataSource;
                let source = Arc::new(QueueDataSource::new(self.signals.audio.queue.clone()));
                let context = TrackListContext::Queue;

                let view = TrackListView::new(context, source.clone(), &self.signals);
                self.track_list_view = Some(view);
            }
            Route::Album { id, title } => {
                let album_id = id.parse::<u32>().unwrap_or(0);
                let source = Arc::new(AlbumTracksSource::new(album_id, self.api.clone()));

                let context = TrackListContext::Album {
                    id: id.clone(),
                    title: title.clone(),
                    artists: String::new(),
                    year: None,
                    track_count: 0,
                };

                let view = TrackListView::new(context, source.clone(), &self.signals);
                self.track_list_view = Some(view);
            }
            Route::Artist { id, name } => {
                let source = Arc::new(ArtistTracksSource::new(id.clone(), self.api.clone()));

                let context = TrackListContext::Artist {
                    id: id.clone(),
                    name: name.clone(),
                    genres: String::new(),
                    likes: 0,
                    track_count: 0,
                };

                let view = TrackListView::new(context, source.clone(), &self.signals);
                self.track_list_view = Some(view);
            }
            _ => {}
        }
    }

    pub async fn handle_key(&mut self, ev: KeyEvent) -> Action {
        let Some(key) = normalize(ev) else {
            return Action::None;
        };

        if matches!(key, Key::Ctrl('c') | Key::Ctrl('q')) {
            return Action::Quit;
        }

        let in_overlay = self.signals.navigation.overlay.get().is_some();

        if in_overlay {
            self.key_resolver.reset();

            if matches!(key, Key::Esc) {
                return Action::DismissOverlay;
            }

            if let Some(intent) = self.key_resolver.advance(&key)
                && let Intent::Playback(_) = &intent
            {
                return self.execute_intent(intent).await;
            }
            return Action::None;
        }

        let prefix = self.key_resolver.peek_prefix();
        let view_action = self.dispatch_to_view(&key, prefix).await;
        if !view_action.is_none() {
            self.key_resolver.reset();
            return view_action;
        }

        if let Some(intent) = self.key_resolver.advance(&key) {
            return self.execute_intent(intent).await;
        }

        Action::None
    }

    async fn dispatch_to_view(&mut self, key: &Key, prefix: Option<char>) -> Action {
        match &self.current_route.clone() {
            Route::Home => self.home_view.handle_key(key, prefix),
            Route::Search => self.search_view.handle_key(key, prefix),
            Route::Liked => {
                if let Some(view) = &mut self.liked_view {
                    view.handle_key(key, prefix)
                } else {
                    Action::None
                }
            }
            Route::Playlists => {
                if let Some(view) = &mut self.playlist_list_view {
                    view.handle_key(key, prefix)
                } else {
                    Action::None
                }
            }
            Route::Playlist { .. } | Route::Album { .. } | Route::Artist { .. } | Route::Queue => {
                if let Some(view) = &mut self.track_list_view {
                    let action = view.handle_key(key, prefix);
                    if matches!(self.current_route, Route::Queue) {
                        let cursor = view.selected_index();
                        let audio = self.audio.clone();
                        tokio::spawn(async move {
                            audio.write().await.maybe_trigger_fetch(cursor);
                        });
                    }
                    action
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    async fn execute_intent(&mut self, intent: Intent) -> Action {
        match intent {
            Intent::Quit => Action::Quit,
            Intent::Playback(p) => self.execute_playback_intent(p),
            Intent::Navigate(n) => self.execute_navigation_intent(n),
            Intent::View(v) => self.execute_view_intent(v),
            Intent::Queue(q) => self.execute_queue_intent(q),
        }
    }

    fn execute_playback_intent(&self, intent: PlaybackIntent) -> Action {
        match intent {
            PlaybackIntent::Toggle => Action::TogglePlayback,
            PlaybackIntent::Next => Action::NextTrack,
            PlaybackIntent::Previous => Action::PreviousTrack,
            PlaybackIntent::SeekForward(s) => Action::SeekForward(s),
            PlaybackIntent::SeekBackward(s) => Action::SeekBackward(s),
            PlaybackIntent::VolumeUp(n) => {
                let vol = self.signals.audio.volume.get();
                Action::SetVolume((vol + n).min(100))
            }
            PlaybackIntent::VolumeDown(n) => {
                let vol = self.signals.audio.volume.get();
                Action::SetVolume(vol.saturating_sub(n))
            }
            PlaybackIntent::ToggleMute => Action::ToggleMute,
            PlaybackIntent::ToggleShuffle => Action::ToggleShuffle,
            PlaybackIntent::CycleRepeat => Action::CycleRepeat,
            PlaybackIntent::Like(Target::Current) => self
                .signals
                .audio
                .current_track
                .get()
                .map(Action::LikeTrack)
                .unwrap_or(Action::None),
            PlaybackIntent::Like(Target::Selected) => self
                .current_selection_track()
                .map(Action::LikeTrack)
                .unwrap_or(Action::None),
            PlaybackIntent::Dislike(Target::Current) => self
                .signals
                .audio
                .current_track
                .get()
                .map(Action::DislikeTrack)
                .unwrap_or(Action::None),
            PlaybackIntent::Dislike(Target::Selected) => self
                .current_selection_track()
                .map(Action::DislikeTrack)
                .unwrap_or(Action::None),
            PlaybackIntent::StartWave(Target::Current) => self
                .signals
                .audio
                .current_track
                .get()
                .map(|t| Action::StartWave {
                    seeds: vec![format!("track:{}", t.id)],
                    title: t.title.clone(),
                    toast_message: None,
                })
                .unwrap_or(Action::None),
            PlaybackIntent::StartWave(Target::Selected) => self
                .current_selection_track()
                .map(|t| Action::StartWave {
                    seeds: vec![format!("track:{}", t.id)],
                    title: t.title.clone(),
                    toast_message: None,
                })
                .unwrap_or(Action::None),
        }
    }

    fn execute_view_intent(&self, intent: ViewIntent) -> Action {
        match intent {
            ViewIntent::Like => Action::LikeContext,
            ViewIntent::Dislike => Action::DislikeContext,
            ViewIntent::QueueAll => Action::QueueAll,
            ViewIntent::PlayAllNext => Action::PlayAllNext,
            ViewIntent::StartWave => match &self.current_route {
                Route::Album { id, title } => Action::StartWave {
                    seeds: vec![format!("album:{id}")],
                    title: Some(title.clone()),
                    toast_message: None,
                },
                Route::Artist { id, name } => Action::StartWave {
                    seeds: vec![format!("artist:{id}")],
                    title: Some(name.clone()),
                    toast_message: None,
                },
                Route::Playlist { kind, title } => {
                    let owner = self.api.current_user_id();
                    let owner = self
                        .track_list_view
                        .as_ref()
                        .and_then(|view| {
                            if let TrackListContext::Playlist { owner_uid, .. } = view.context() {
                                if *owner_uid > 0 {
                                    Some(*owner_uid)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .unwrap_or(owner);

                    Action::StartWave {
                        seeds: vec![format!("playlist:{owner}_{kind}")],
                        title: Some(title.clone()),
                        toast_message: None,
                    }
                }
                _ => Action::StartWave {
                    seeds: vec!["user:onyourwave".to_string()],
                    title: None,
                    toast_message: None,
                },
            },
        }
    }

    fn execute_queue_intent(&self, intent: QueueIntent) -> Action {
        match intent {
            QueueIntent::Add => self
                .current_selection_track()
                .map(Action::QueueTrack)
                .unwrap_or(Action::None),
            QueueIntent::PlayNext => self
                .current_selection_track()
                .map(Action::PlayNext)
                .unwrap_or(Action::None),
            QueueIntent::Remove => self
                .track_list_view
                .as_ref()
                .map(|v| Action::RemoveFromQueue(v.selected_index()))
                .unwrap_or(Action::None),
            QueueIntent::Clear => Action::ClearQueue,
        }
    }

    fn execute_navigation_intent(&mut self, intent: NavigationIntent) -> Action {
        match intent {
            NavigationIntent::Go(route) => Action::Navigate(route),
            NavigationIntent::Back => {
                if self.signals.navigation.history.with(|h| !h.is_empty()) {
                    Action::Back
                } else {
                    Action::None
                }
            }
            NavigationIntent::NextTab => {
                let next = match &self.current_route {
                    Route::Search => Route::Home,
                    Route::Home => Route::Liked,
                    Route::Liked => Route::Playlists,
                    Route::Playlists => Route::Search,
                    _ => Route::Search,
                };
                Action::Navigate(next)
            }
            NavigationIntent::PrevTab => {
                let prev = match &self.current_route {
                    Route::Search => Route::Playlists,
                    Route::Home => Route::Search,
                    Route::Liked => Route::Home,
                    Route::Playlists => Route::Liked,
                    _ => Route::Search,
                };
                Action::Navigate(prev)
            }
            NavigationIntent::ShowOverlay(route) => Action::Overlay(route),
            NavigationIntent::DismissOverlay => Action::DismissOverlay,
            NavigationIntent::ScrollTop => Action::ScrollTop,
            NavigationIntent::ScrollBottom => Action::ScrollBottom,
        }
    }

    fn current_selection_track(&self) -> Option<yandex_music::model::track::Track> {
        match &self.current_route {
            Route::Playlist { .. } | Route::Album { .. } | Route::Artist { .. } | Route::Queue => {
                self.track_list_view.as_ref()?.selected_item()
            }
            Route::Liked => self.liked_view.as_ref()?.selected_item(),
            _ => None,
        }
    }

    pub fn view(&mut self, frame: &mut Frame) {
        tick_global();

        let area = frame.area();

        let styles = self.theme.get();
        frame.buffer_mut().set_style(area, styles.text);

        let popup_open =
            matches!(self.current_route, Route::Home) && self.home_view.is_popup_open();
        let bg_border = if popup_open {
            styles.block
        } else {
            styles.block_focused
        };

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let content_area = if self.sidebar_visible {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(0)])
                .split(main_chunks[0]);

            self.sidebar
                .view(frame, horizontal[0], &self.current_route, bg_border);

            use ratatui::symbols::{self, border};
            use ratatui::widgets::{Block, Borders};
            let content_block = Block::default()
                .borders(Borders::ALL)
                .border_style(bg_border)
                .border_set(border::Set {
                    ..symbols::border::ROUNDED
                });
            let content_inner = content_block.inner(horizontal[1]);
            frame.render_widget(content_block, horizontal[1]);

            content_inner
        } else {
            main_chunks[0]
        };

        match &self.current_route {
            Route::Home => {
                if self.signals.is_focused.get() {
                    self.visualizer.view(frame, content_area);
                }
                self.home_view.view(frame, content_area);
            }
            Route::Search => self.search_view.view(frame, content_area),
            Route::Liked => {
                if let Some(view) = &mut self.liked_view {
                    view.view(frame, content_area);
                }
            }
            Route::Playlists => {
                if let Some(view) = &mut self.playlist_list_view {
                    view.view(frame, content_area);
                }
            }
            Route::Playlist { .. } | Route::Album { .. } | Route::Artist { .. } | Route::Queue => {
                if let Some(view) = &mut self.track_list_view {
                    view.view(frame, content_area);
                }
            }
            _ => {}
        }

        self.player_bar.view(frame, main_chunks[1]);

        if let Some(overlay) = self.signals.navigation.overlay.get() {
            OverlayRenderer::render(frame, content_area, &overlay, &self.theme, &mut self.lyrics);
        }

        self.toast_manager.view(frame, area);
    }
}

fn is_top_level(route: &Route) -> bool {
    matches!(
        route,
        Route::Home | Route::Search | Route::Liked | Route::Playlists
    )
}

impl App {
    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut terminal = Terminal::new()?;
        terminal.enter()?;

        let tick_tx = terminal.tick_tx.clone();
        let current_route = self.signals.navigation.current_route.clone();
        let overlay = self.signals.navigation.overlay.clone();
        let animating = self.toast_manager.is_animating();
        let focused = self.signals.is_focused.clone();

        crate::framework::reactive::effect(move || {
            let route = current_route.get();
            let overlay = overlay.get();
            let animating = animating.get();
            let focused = focused.get();

            let rate = if !focused {
                TickRate::Idle
            } else if animating {
                TickRate::Animation
            } else if overlay.is_some() {
                TickRate::Normal
            } else {
                match route {
                    Route::Home | Route::Lyrics => TickRate::High,
                    _ => TickRate::Normal,
                }
            };

            let _ = tick_tx.send(rate);
        });

        loop {
            tokio::select! {
                Some(term_event) = terminal.next() => {
                    match term_event {
                        TerminalEvent::Key(key) => {
                            let action = self.handle_key(key).await;
                            self.process_action(action).await;
                        }
                        TerminalEvent::Tick => {
                        }
                        TerminalEvent::FocusLost => {
                            self.signals.is_focused.set(false);
                            self.update_bridge_state();
                        }
                        TerminalEvent::FocusGained => {
                            terminal.backend_mut().flush().unwrap();
                            self.signals.is_focused.set(true);
                            self.update_bridge_state();
                        }
                        TerminalEvent::Resize(_, _) => {
                            terminal.backend_mut().flush().unwrap();
                        }
                        TerminalEvent::Quit | TerminalEvent::Closed => {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
                Ok(event) = self.event_rx.recv_async() => {
                    self.process_event(event).await;
                }
            }

            if !self.should_quit && self.signals.is_focused.get() {
                terminal.draw(|f| self.view(f))?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }
}
