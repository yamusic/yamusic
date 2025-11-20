use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use tracing::info;
use yandex_music::model::info::lyrics::LyricsFormat;
use yandex_music::model::playlist::PlaylistTracks;

use crate::{
    event::events::Event,
    ui::{
        app::App,
        traits::Action,
        tui::{TerminalEvent, Tui},
        views::{Lyrics, PlaylistDetail},
    },
};

pub struct EventHandler;

impl EventHandler {
    pub async fn handle_events(app: &mut App, tui: &Tui) -> color_eyre::Result<bool> {
        let mut should_render = false;
        if let Some(evt) = tui.next().await {
            if Self::handle_event(app, evt).await? {
                should_render = true;
            }
        }

        while let Ok(evt) = app.event_rx.try_recv() {
            Self::handle_action(app, evt).await;
            should_render = true;
        }

        Ok(should_render)
    }

    pub async fn handle_event(app: &mut App, evt: TerminalEvent) -> color_eyre::Result<bool> {
        match evt {
            TerminalEvent::Init => {
                app.state.is_loading = true;
                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                let api_clone = api.clone();
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    match api_clone.fetch_liked_tracks().await {
                        Ok(playlist) => {
                            info!("Liked tracks raw playlist tracks: {:?}", playlist.tracks);
                            let tracks = match playlist.tracks {
                                Some(PlaylistTracks::Full(tracks)) => tracks,
                                Some(PlaylistTracks::WithInfo(tracks)) => {
                                    tracks.into_iter().map(|t| t.track).collect()
                                }
                                Some(PlaylistTracks::Partial(partial_tracks)) => {
                                    match api_clone.fetch_tracks_partial(&partial_tracks).await {
                                        Ok(tracks) => tracks,
                                        Err(e) => {
                                            info!("Failed to fetch partial tracks: {}", e);
                                            vec![]
                                        }
                                    }
                                }
                                None => vec![],
                            };
                            let _ = tx_clone.send(Event::LikedTracksFetched(tracks.clone()));
                            let _ = tx_clone.send(Event::TracksFetched(tracks));
                        }
                        Err(e) => {
                            let _ = tx_clone.send(Event::FetchError(e.to_string()));
                        }
                    }
                });
                tokio::spawn(async move {
                    match api.fetch_all_playlists().await {
                        Ok(playlists) => {
                            let _ = tx.send(Event::PlaylistsFetched(playlists));
                        }
                        Err(e) => {
                            let _ = tx.send(Event::FetchError(e.to_string()));
                        }
                    }
                });
            }
            TerminalEvent::Quit => app.should_quit = true,
            TerminalEvent::FocusGained => app.has_focus = true,
            TerminalEvent::FocusLost => app.has_focus = false,
            TerminalEvent::Key(key) => Self::handle_key_event(app, key).await,
            TerminalEvent::Mouse(mouse) => Self::handle_mouse_event(app, mouse),
            TerminalEvent::Tick => {
                app.state.spinner_index = (app.state.spinner_index + 1) % 10;
                return Ok(true);
            }
            _ => {}
        }

        Ok(true)
    }

    pub async fn handle_action(app: &mut App, evt: Event) {
        info!("Handling action: {:?}", evt);
        match evt {
            Event::Play(track_id) => {
                app.ctx
                    .audio_system
                    .play_track_at_index(track_id as usize)
                    .await;
            }
            Event::TrackEnded => {
                app.state.lyrics = None;
                app.ctx.audio_system.on_track_ended().await;
            }
            Event::TracksFetched(tracks) => {
                app.state.is_loading = false;
                app.ctx.audio_system.load_tracks(tracks).await;
            }
            Event::LikedTracksFetched(tracks) => {
                info!("Liked tracks fetched: {} tracks", tracks.len());
                app.state.liked_tracks = tracks;
            }
            Event::PlaylistTracksFetched(tracks) => {
                info!("Playlist tracks fetched: {} tracks", tracks.len());
                app.state.is_loading = false;
                if let Some(view) = app.view_stack.last_mut() {
                    view.on_event(&Event::PlaylistTracksFetched(tracks));
                }
            }
            Event::PlaylistsFetched(playlists) => {
                app.state.playlists = playlists;
            }
            Event::PlaylistSelected(playlist) => {
                let state = PlaylistDetail::new(playlist.clone());
                app.view_stack.push(Box::new(state));

                app.state.is_loading = true;
                let api = app.ctx.api.clone();
                let tx = app.ctx.event_tx.clone();
                let playlist_kind = playlist.kind;

                tokio::spawn(async move {
                    match api.fetch_playlist(playlist_kind).await {
                        Ok(playlist) => {
                            let tracks = match playlist.tracks {
                                Some(PlaylistTracks::Full(tracks)) => tracks,
                                Some(PlaylistTracks::WithInfo(tracks)) => {
                                    tracks.into_iter().map(|t| t.track).collect()
                                }
                                Some(PlaylistTracks::Partial(partial_tracks)) => {
                                    match api.fetch_tracks_partial(&partial_tracks).await {
                                        Ok(tracks) => tracks,
                                        Err(e) => {
                                            info!("Failed to fetch partial tracks: {}", e);
                                            vec![]
                                        }
                                    }
                                }
                                None => vec![],
                            };
                            let _ = tx.send(Event::PlaylistTracksFetched(tracks));
                        }
                        Err(e) => {
                            let _ = tx.send(Event::FetchError(e.to_string()));
                        }
                    }
                });
            }
            Event::LyricsFetched(lyrics) => {
                app.state.lyrics = lyrics;
            }
            Event::FetchError(_e) => {
                app.state.is_loading = false;
            }
            Event::TrackStarted(_track, _index) => {
                if let Some(track) = app.ctx.audio_system.current_track() {
                    let format = if track
                        .lyrics_info
                        .as_ref()
                        .is_some_and(|l| l.has_available_sync_lyrics)
                    {
                        LyricsFormat::LRC
                    } else {
                        LyricsFormat::TEXT
                    };

                    let track_id = track.id.clone();
                    let api = app.ctx.api.clone();
                    let tx = app.ctx.event_tx.clone();

                    tokio::spawn(async move {
                        match api.fetch_lyrics(track_id, format).await {
                            Ok(lyrics) => {
                                let _ = tx.send(Event::LyricsFetched(lyrics));
                            }
                            Err(_) => {
                                let _ = tx.send(Event::LyricsFetched(None));
                            }
                        }
                    });
                }
            }
            _ => {}
        }
    }

    async fn handle_key_event(app: &mut App, evt: KeyEvent) {
        use ratatui::crossterm::event::KeyCode;

        if evt.kind == KeyEventKind::Press {
            let action = if let Some(view) = app.view_stack.last_mut() {
                view.handle_input(evt, &app.ctx, &app.state)
            } else {
                None
            };

            if let Some(action) = action {
                Self::dispatch_action(app, action).await;
                return;
            }

            match (evt.code, evt.modifiers) {
                (KeyCode::Char('c'), mods) if mods.contains(KeyModifiers::CONTROL) => {
                    app.should_quit = true;
                }
                (KeyCode::Char('q'), _) => app.should_quit = true,
                (KeyCode::Char(' '), _) => app.ctx.audio_system.play_pause(),
                (KeyCode::Char('p'), _) => app.ctx.audio_system.play_previous().await,
                (KeyCode::Char('n'), _) => app.ctx.audio_system.play_next().await,
                (KeyCode::Char('+'), _) => app.ctx.audio_system.volume_up(10),
                (KeyCode::Char('-'), _) => app.ctx.audio_system.volume_down(10),
                (KeyCode::Char('='), _) => app.ctx.audio_system.set_volume(100),
                (KeyCode::Char('H'), _) => app.ctx.audio_system.seek_backwards(10),
                (KeyCode::Char('L'), _) => app.ctx.audio_system.seek_forwards(10),
                (KeyCode::Char('r'), _) => app.ctx.audio_system.toggle_repeat_mode(),
                (KeyCode::Char('s'), _) => app.ctx.audio_system.toggle_shuffle(),
                (KeyCode::Char('m'), _) => app.ctx.audio_system.toggle_mute(),
                (KeyCode::Char('l'), _) => {
                    app.view_stack.push(Box::new(Lyrics::default()));
                }
                (KeyCode::Esc, _) => {
                    if app.view_stack.len() > 1 {
                        app.view_stack.pop();
                    }
                }
                (KeyCode::Tab, _) => {
                    app.state.sidebar_index = (app.state.sidebar_index + 1) % 3;
                    app.view_stack.clear();
                    match app.state.sidebar_index {
                        0 => app
                            .view_stack
                            .push(Box::new(crate::ui::views::MyVibe::default())),
                        1 => {
                            app.view_stack
                                .push(Box::new(crate::ui::views::TrackList::default()));
                        }
                        2 => app
                            .view_stack
                            .push(Box::new(crate::ui::views::Playlists::default())),
                        _ => {}
                    }
                }
                (KeyCode::BackTab, _) => {
                    app.state.sidebar_index = (app.state.sidebar_index + 2) % 3;
                    app.view_stack.clear();
                    match app.state.sidebar_index {
                        0 => app
                            .view_stack
                            .push(Box::new(crate::ui::views::MyVibe::default())),
                        1 => {
                            app.view_stack
                                .push(Box::new(crate::ui::views::TrackList::default()));
                        }
                        2 => app
                            .view_stack
                            .push(Box::new(crate::ui::views::Playlists::default())),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    async fn dispatch_action(app: &mut App, action: Action) {
        match action {
            Action::Quit => app.should_quit = true,
            Action::PlayPause => app.ctx.audio_system.play_pause(),
            _ => {}
        }
    }

    fn handle_mouse_event(app: &mut App, evt: MouseEvent) {
        match (evt.kind, evt.modifiers) {
            (MouseEventKind::ScrollUp, KeyModifiers::SHIFT) => {
                app.ctx.audio_system.seek_forwards(1)
            }
            (MouseEventKind::ScrollUp, _) => app.ctx.audio_system.volume_up(1),
            (MouseEventKind::ScrollDown, KeyModifiers::SHIFT) => {
                app.ctx.audio_system.seek_backwards(1)
            }
            (MouseEventKind::ScrollDown, _) => app.ctx.audio_system.volume_down(1),
            _ => {}
        }
    }
}
