use crate::event::events::Event;
use crate::ui::context::AppContext;
use crate::ui::state::AppState;
use async_trait::async_trait;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use yandex_music::model::{rotor::session::Session, track::Track};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Quit,
    PlayPause,
    NextTrack,
    PreviousTrack,
    VolumeUp,
    VolumeDown,
    SetVolume(u8),
    ToggleMute,
    SeekForward,
    SeekBackward,
    ToggleRepeat,
    ToggleShuffle,
    OpenLyrics,
    CloseLyrics,
    Navigate(Direction),
    Select,
    SwitchTab(usize),
    Back,
    None,
    PlayWave(Session, Vec<Track>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[async_trait]
pub trait View: Send {
    fn render(&mut self, f: &mut Frame, area: Rect, state: &AppState, ctx: &AppContext);
    async fn handle_input(
        &mut self,
        key: KeyEvent,
        state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action>;
    async fn on_event(&mut self, _event: &Event, _ctx: &AppContext) {}
    async fn on_mount(&mut self, _ctx: &AppContext) {}
}
