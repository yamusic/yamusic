use std::sync::Arc;

use flume::Receiver;

use ratatui::Frame;

use crate::{
    audio::system::AudioSystem,
    event::events::Event,
    http::ApiService,
    ui::{
        context::AppContext,
        layout::AppLayout,
        message::{AppMessage, ViewRoute},
        router::Router,
        state::AppState,
        tui::{self, TerminalEvent},
        util::handler::EventHandler,
        views::{
            AlbumDetail, ArtistDetail, Lyrics, MyWave, PlaylistDetail, Playlists, Search,
            TrackDetail, TrackList,
        },
    },
    util::task::TaskManager,
};

pub struct App {
    pub ctx: AppContext,
    pub state: AppState,
    pub router: Router,
    pub event_rx: Receiver<Event>,
    pub has_focus: bool,
    pub should_quit: bool,
    pub task_manager: TaskManager,
}

impl App {
    pub async fn new() -> color_eyre::Result<Self> {
        let (event_tx, event_rx) = flume::unbounded();
        let api = Arc::new(ApiService::new().await?);
        let audio_system = AudioSystem::new(event_tx.clone(), api.clone()).await?;

        let ctx = AppContext {
            api,
            audio_system,
            event_tx,
        };

        let mut state = AppState::default();
        state.ui.sidebar_index = 1;
        let router = Router::new(Box::new(crate::ui::views::MyWave::default()));
        let task_manager = TaskManager::new();

        Ok(Self {
            ctx,
            state,
            router,
            event_rx,
            has_focus: true,
            should_quit: false,
            task_manager,
        })
    }

    pub async fn update(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::Quit => self.should_quit = true,
            AppMessage::TogglePlayPause => self.ctx.audio_system.play_pause().await,
            AppMessage::NextTrack => self.ctx.audio_system.play_next().await,
            AppMessage::PreviousTrack => self.ctx.audio_system.play_previous().await,
            AppMessage::VolumeUp => self.ctx.audio_system.volume_up(10),
            AppMessage::VolumeDown => self.ctx.audio_system.volume_down(10),
            AppMessage::SeekForward => self.ctx.audio_system.seek_forwards(10).await,
            AppMessage::SeekBackward => self.ctx.audio_system.seek_backwards(10).await,
            AppMessage::ToggleRepeat => self.ctx.audio_system.toggle_repeat_mode(),
            AppMessage::ToggleShuffle => self.ctx.audio_system.toggle_shuffle(),
            AppMessage::ToggleMute => self.ctx.audio_system.toggle_mute(),
            AppMessage::NavigateTo(route) => self.navigate(route).await,
            AppMessage::GoBack => {
                self.task_manager.abort("view_fetch");
                if self.router.has_overlay() {
                    self.router.clear_overlay();
                } else {
                    self.router.pop();
                }
            }
            AppMessage::NextSidebarItem => {
                self.state.ui.sidebar_index = (self.state.ui.sidebar_index + 1) % 4;
                self.update_sidebar_view().await;
            }
            AppMessage::PreviousSidebarItem => {
                self.state.ui.sidebar_index = (self.state.ui.sidebar_index + 3) % 4;
                self.update_sidebar_view().await;
            }
            AppMessage::SetSidebarIndex(index) => {
                self.state.ui.sidebar_index = index;
                self.update_sidebar_view().await;
            }
            AppMessage::ToggleQueue => {
                if self.router.has_overlay() {
                    self.router.clear_overlay();
                } else {
                    self.router.set_overlay(Box::new(TrackList::default()));
                }
            }
            _ => {}
        }
    }

    async fn update_sidebar_view(&mut self) {
        self.task_manager.abort("view_fetch");
        self.router.stack.clear();
        match self.state.ui.sidebar_index {
            0 => self.router.push(Box::new(Search::default())),
            1 => self.router.push(Box::new(MyWave::default())),
            2 => {
                self.router.push(Box::new(PlaylistDetail::loading()));
                let _ = self
                    .ctx
                    .event_tx
                    .send(crate::event::events::Event::PlaylistKindSelected(3));
                return;
            }
            3 => self.router.push(Box::new(Playlists::default())),
            _ => {}
        }
        if let Some(view) = self.router.active_view_mut() {
            view.on_mount(&self.ctx).await;
        }
    }

    async fn navigate(&mut self, route: ViewRoute) {
        self.task_manager.abort("view_fetch");
        match route {
            ViewRoute::Search => {
                self.state.ui.sidebar_index = 0;
                self.update_sidebar_view().await;
            }
            ViewRoute::MyWave => {
                self.state.ui.sidebar_index = 1;
                self.update_sidebar_view().await;
            }

            ViewRoute::Playlists => {
                self.state.ui.sidebar_index = 3;
                self.update_sidebar_view().await;
            }
            ViewRoute::TrackList => {
                if self.router.has_overlay() {
                    self.router.clear_overlay();
                } else {
                    self.router.set_overlay(Box::new(TrackList::default()));
                }
            }
            ViewRoute::PlaylistDetail(playlist) => {
                self.state.ui.current_route = crate::ui::state::Route::PlaylistDetail;
                let _ = self
                    .ctx
                    .event_tx
                    .send(crate::event::events::Event::PlaylistSelected(playlist));
            }
            ViewRoute::AlbumDetail(album) => {
                self.state.ui.current_route = crate::ui::state::Route::AlbumDetail;
                self.router.push(Box::new(AlbumDetail::new(album)));
                if let Some(view) = self.router.active_view_mut() {
                    view.on_mount(&self.ctx).await;
                }
            }
            ViewRoute::ArtistDetail(artist) => {
                self.state.ui.current_route = crate::ui::state::Route::ArtistDetail;
                self.router.push(Box::new(ArtistDetail::new(artist)));
                if let Some(view) = self.router.active_view_mut() {
                    view.on_mount(&self.ctx).await;
                }
            }
            ViewRoute::TrackDetail(track) => {
                self.state.ui.current_route = crate::ui::state::Route::TrackDetail;
                self.router.push(Box::new(TrackDetail::new(track)));
                if let Some(view) = self.router.active_view_mut() {
                    view.on_mount(&self.ctx).await;
                }
            }
            ViewRoute::Lyrics => {
                if self.router.has_overlay() {
                    self.router.clear_overlay();
                } else {
                    self.router.set_overlay(Box::new(Lyrics::default()));
                }
            }
        }
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut tui = tui::Tui::new()?.mouse(true);
        tui.enter()?;

        EventHandler::handle_event(self, TerminalEvent::Init, &mut tui).await?;

        tui.draw(|f| {
            self.ui(f);
        })?;

        while !self.should_quit {
            if EventHandler::handle_events(self, &mut tui).await? {
                if !self.has_focus {
                    continue;
                }
                tui.draw(|f| {
                    self.ui(f);
                })?;
            }
        }

        Ok(())
    }

    fn ui(&mut self, frame: &mut Frame) {
        if self.has_focus {
            AppLayout::new(self).render(frame, frame.area());
        }
    }
}
