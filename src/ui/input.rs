use crate::ui::message::{AppMessage, ViewRoute};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct InputHandler;

impl InputHandler {
    pub fn handle_key(key: KeyEvent) -> Option<AppMessage> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) => Some(AppMessage::ToggleQueue),
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(AppMessage::Quit),
            (KeyCode::Char(' '), _) => Some(AppMessage::TogglePlayPause),
            (KeyCode::Char('n'), _) => Some(AppMessage::NextTrack),
            (KeyCode::Char('p'), _) => Some(AppMessage::PreviousTrack),
            (KeyCode::Char('+'), _) => Some(AppMessage::VolumeUp),
            (KeyCode::Char('-'), _) => Some(AppMessage::VolumeDown),
            (KeyCode::Char('='), _) => Some(AppMessage::VolumeUp),
            (KeyCode::Char('H'), _) => Some(AppMessage::SeekBackward),
            (KeyCode::Char('L'), _) => Some(AppMessage::SeekForward),
            (KeyCode::Char('r'), _) => Some(AppMessage::ToggleRepeat),
            (KeyCode::Char('s'), _) => Some(AppMessage::ToggleShuffle),
            (KeyCode::Char('m'), _) => Some(AppMessage::ToggleMute),
            (KeyCode::Char('y'), _) => Some(AppMessage::NavigateTo(ViewRoute::Lyrics)),
            (KeyCode::Esc, _) => Some(AppMessage::GoBack),
            (KeyCode::Tab, _) => Some(AppMessage::NextSidebarItem),
            (KeyCode::BackTab, _) => Some(AppMessage::PreviousSidebarItem),
            (KeyCode::Char('1'), _) => Some(AppMessage::SetSidebarIndex(0)),
            (KeyCode::Char('2'), _) => Some(AppMessage::SetSidebarIndex(1)),
            (KeyCode::Char('3'), _) => Some(AppMessage::SetSidebarIndex(2)),
            (KeyCode::Char('4'), _) => Some(AppMessage::SetSidebarIndex(3)),
            _ => None,
        }
    }
}
