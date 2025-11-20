use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::ui::{
    components::lyrics::LyricsWidget,
    context::{AppContext, GlobalUiState},
    traits::{Action, Component},
};

#[derive(Default)]
pub struct Lyrics;

impl Component for Lyrics {
    fn render(&mut self, f: &mut Frame, area: Rect, ctx: &AppContext, state: &GlobalUiState) {
        let track_progress = ctx.audio_system.track_progress();
        let widget = LyricsWidget::new(state.lyrics.as_deref(), track_progress);
        f.render_widget(widget, area);
    }

    fn handle_input(
        &mut self,
        _key: KeyEvent,
        _ctx: &AppContext,
        _state: &GlobalUiState,
    ) -> Option<Action> {
        None
    }
}
