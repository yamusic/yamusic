use crossterm::event::KeyCode;
use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use tracing::info;
use yandex_music::model::info::lyrics::LyricsFormat;

use crate::{
    audio::queue::PlaybackContext,
    event::events::Event,
    ui::{
        app::App,
        input::InputHandler,
        message::AppMessage,
        traits::Action,
        tui::{TerminalEvent, Tui},
        views::{AlbumDetail, ArtistDetail, PlaylistDetail, TrackDetail},
    },
    util::track::extract_track_ids,
};

const PLAYLIST_PAGE_SIZE: usize = 10;

pub struct EventHandler;

impl EventHandler {
    pub async fn handle_events(app: &mut App, tui: &mut Tui) -> color_eyre::Result<bool> {
        let mut should_render = false;
        if let Some(evt) = tui.next().await {
            if Self::handle_event(app, evt, tui).await? {
                should_render = true;
            }
        }

        while let Ok(evt) = app.event_rx.try_recv() {
            Self::handle_action(app, evt).await;
            should_render = true;
        }

        Ok(should_render)
    }

    pub async fn handle_event(
        app: &mut App,
        evt: TerminalEvent,
        tui: &mut Tui,
    ) -> color_eyre::Result<bool> {
        match evt {
            TerminalEvent::Init => {}
            TerminalEvent::Quit => app.should_quit = true,
            TerminalEvent::FocusGained => {
                app.has_focus = true;
                tui.clear()?;
            }
            TerminalEvent::FocusLost => app.has_focus = false,
            TerminalEvent::Key(key) => Self::handle_key_event(app, key).await,
            TerminalEvent::Mouse(mouse) => Self::handle_mouse_event(app, mouse).await,
            TerminalEvent::Tick => {
                return Ok(app.has_focus);
            }
            _ => {}
        }

        Ok(true)
    }

    pub async fn handle_action(app: &mut App, evt: Event) {
        app.router.on_event(&evt, &app.ctx).await;

        match evt {
            Event::Play(track_id) => {
                app.ctx
                    .audio_system
                    .play_track_at_index(track_id as usize)
                    .await;
            }
            Event::TrackEnded => {
                app.state.data.lyrics = None;
                app.ctx.audio_system.on_track_ended().await;
            }
            Event::QueueEnded => {
                app.state.data.lyrics = None;
            }
            Event::TracksFetched(tracks) => {
                app.ctx.audio_system.load_tracks(tracks).await;
            }
            Event::TrackFetched(track) => {
                app.ctx
                    .audio_system
                    .load_context(PlaybackContext::Track(track.clone()), vec![track], 0)
                    .await;
            }
            Event::PlaylistSelected(playlist) => {
                let state = PlaylistDetail::new(playlist.clone());
                app.router.push(Box::new(state));

                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                let playlist_kind = playlist.kind;

                app.task_manager.spawn(
                    "view_fetch",
                    tokio::spawn(async move {
                        match api.fetch_playlist_bare(playlist_kind).await {
                            Ok(playlist) => {
                                let _ = tx.send(Event::PlaylistFetched(playlist.clone()));

                                let track_ids = match &playlist.tracks {
                                    Some(tracks) => extract_track_ids(tracks),
                                    None => vec![],
                                };

                                if track_ids.is_empty() {
                                    return;
                                }

                                let first_batch: Vec<_> =
                                    track_ids.iter().take(PLAYLIST_PAGE_SIZE).cloned().collect();

                                match api.fetch_tracks_by_ids(first_batch).await {
                                    Ok(tracks) => {
                                        let tracks: Vec<_> = tracks
                                            .into_iter()
                                            .filter(|t| t.available.unwrap_or(false))
                                            .collect();
                                        let _ = tx.send(Event::PlaylistTracksFetched(
                                            playlist_kind,
                                            tracks,
                                        ));
                                    }
                                    Err(e) => {
                                        info!("Failed to fetch tracks: {}", e);
                                        let _ = tx.send(Event::FetchError(e.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    }),
                );
            }
            Event::PlaylistKindSelected(kind) => {
                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();

                app.task_manager.spawn(
                    "view_fetch",
                    tokio::spawn(async move {
                        match api.fetch_playlist_bare(kind).await {
                            Ok(playlist) => {
                                let _ = tx.send(Event::PlaylistFetched(playlist.clone()));

                                let track_ids = match &playlist.tracks {
                                    Some(tracks) => extract_track_ids(tracks),
                                    None => vec![],
                                };

                                if track_ids.is_empty() {
                                    return;
                                }

                                let first_batch: Vec<_> =
                                    track_ids.iter().take(PLAYLIST_PAGE_SIZE).cloned().collect();

                                match api.fetch_tracks_by_ids(first_batch).await {
                                    Ok(tracks) => {
                                        let tracks: Vec<_> = tracks
                                            .into_iter()
                                            .filter(|t| t.available.unwrap_or(false))
                                            .collect();
                                        let _ = tx.send(Event::PlaylistTracksFetched(kind, tracks));
                                    }
                                    Err(e) => {
                                        info!("Failed to fetch tracks: {}", e);
                                        let _ = tx.send(Event::FetchError(e.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    }),
                );
            }
            Event::PlaylistFetched(playlist) => {
                info!("Playlist '{}' fetched", playlist.title);
            }
            Event::AlbumSelected(album) => {
                let state = AlbumDetail::new(album.clone());
                app.router.push(Box::new(state));

                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                let album_id = album.id.unwrap_or_default();

                app.task_manager.spawn(
                    "view_fetch",
                    tokio::spawn(async move {
                        match api.fetch_album_with_tracks(album_id).await {
                            Ok(album) => {
                                let tracks =
                                    album.volumes.into_iter().flatten().collect::<Vec<_>>();
                                let _ = tx.send(Event::AlbumTracksFetched(tracks));
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    }),
                );
            }
            Event::ArtistSelected(artist) => {
                let state = ArtistDetail::new(artist.clone());
                app.router.push(Box::new(state));

                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                let artist_id = artist.id.clone().unwrap_or_default();

                app.task_manager.spawn(
                    "view_fetch",
                    tokio::spawn(async move {
                        match api.fetch_artist_tracks_paginated(artist_id, 0, 10).await {
                            Ok((tracks, pager)) => {
                                let _ = tx.send(Event::ArtistTracksFetched(tracks, pager));
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    }),
                );
            }
            Event::TrackSelected(track) => {
                let state = TrackDetail::new(track);
                app.router.push(Box::new(state));
            }
            Event::LyricsFetched(lyrics) => {
                app.state.data.lyrics = lyrics;
            }
            Event::Search(query) => {
                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                app.task_manager.spawn(
                    "view_fetch",
                    tokio::spawn(async move {
                        match api.search(&query).await {
                            Ok(results) => {
                                let _ = tx.send(Event::SearchResults(results));
                            }
                            Err(e) => {
                                let _ = tx.send(Event::FetchError(e.to_string()));
                            }
                        }
                    }),
                );
            }
            Event::FetchError(_e) => {}
            Event::TrackStarted(_track, _index) => {
                app.state.data.lyrics = None;
                if let Some(track) = app.ctx.audio_system.current_track() {
                    let format = track.lyrics_info.as_ref().and_then(|l| {
                        if l.has_available_sync_lyrics {
                            Some(LyricsFormat::LRC)
                        } else if l.has_available_text_lyrics {
                            Some(LyricsFormat::TEXT)
                        } else {
                            None
                        }
                    });

                    let format = match format {
                        Some(f) => f,
                        None => return,
                    };

                    let track_id = track.id.clone();
                    let api = app.ctx.api.clone();
                    let tx = app.ctx.event_tx.clone();

                    app.task_manager.spawn(
                        "fetch_lyrics",
                        tokio::spawn(async move {
                            match api.fetch_lyrics(track_id, format).await {
                                Ok(lyrics) => {
                                    let _ = tx.send(Event::LyricsFetched(lyrics));
                                }
                                Err(_) => {
                                    let _ = tx.send(Event::LyricsFetched(None));
                                }
                            }
                        }),
                    );
                }
            }
            Event::WaveReady(session, tracks) => {
                app.ctx
                    .audio_system
                    .load_context(PlaybackContext::Wave(session), tracks, 0)
                    .await;
            }
            Event::QueueUpdated => {
                app.ctx.audio_system.sync_queue().await;
            }
            _ => {}
        }
    }

    async fn handle_key_event(app: &mut App, evt: KeyEvent) {
        if evt.kind == KeyEventKind::Press {
            match evt.code {
                KeyCode::Char('c') if evt.modifiers == KeyModifiers::CONTROL => {
                    app.update(AppMessage::Quit).await;
                    return;
                }
                KeyCode::Tab => {
                    app.update(AppMessage::NextSidebarItem).await;
                    return;
                }
                KeyCode::BackTab => {
                    app.update(AppMessage::PreviousSidebarItem).await;
                    return;
                }
                _ => {}
            }

            let action = app.router.handle_input(evt, &app.state, &app.ctx).await;

            if let Some(action) = action {
                Self::dispatch_action(app, action).await;
                return;
            }

            if let Some(msg) = InputHandler::handle_key(evt) {
                app.update(msg).await;
            }
        }
    }

    async fn dispatch_action(app: &mut App, action: Action) {
        match action {
            Action::Quit => app.should_quit = true,
            Action::PlayPause => app.ctx.audio_system.play_pause().await,
            Action::PlayContext(context, tracks, start_index) => {
                app.ctx
                    .audio_system
                    .load_context(context, tracks, start_index)
                    .await;
            }
            _ => {}
        }
    }

    async fn handle_mouse_event(app: &mut App, evt: MouseEvent) {
        match (evt.kind, evt.modifiers) {
            (MouseEventKind::ScrollUp, KeyModifiers::SHIFT) => {
                app.ctx.audio_system.seek_forwards(1).await
            }
            (MouseEventKind::ScrollUp, _) => app.ctx.audio_system.volume_up(1),
            (MouseEventKind::ScrollDown, KeyModifiers::SHIFT) => {
                app.ctx.audio_system.seek_backwards(1).await
            }
            (MouseEventKind::ScrollDown, _) => app.ctx.audio_system.volume_down(1),
            _ => {}
        }
    }
}
