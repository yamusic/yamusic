use crate::ui::context::{AppContext, GlobalUiState};
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub trait Component: Send {
    fn render(&mut self, f: &mut Frame, area: Rect, ctx: &AppContext, state: &GlobalUiState);
    fn handle_input(
        &mut self,
        key: KeyEvent,
        ctx: &AppContext,
        state: &GlobalUiState,
    ) -> Option<Action>;
    fn on_event(&mut self, _event: &crate::event::events::Event) {}
}
