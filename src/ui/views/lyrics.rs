use async_trait::async_trait;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::event::events::Event;
use crate::ui::{
    components::lyrics::LyricsWidget,
    context::AppContext,
    state::AppState,
    traits::{Action, View},
};

#[derive(Default)]
pub struct Lyrics {
    lyrics: Option<String>,
}

#[async_trait]
impl View for Lyrics {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, ctx: &AppContext) {
        let track_progress = ctx.audio_system.track_progress();
        let widget = LyricsWidget::new(self.lyrics.as_deref(), track_progress);
        f.render_widget(widget, area);
    }

    async fn handle_input(
        &mut self,
        _key: KeyEvent,
        _state: &AppState,
        _ctx: &AppContext,
    ) -> Option<Action> {
        None
    }

    async fn on_event(&mut self, event: &Event, _ctx: &AppContext) {
        if let Event::LyricsFetched(lyrics) = event {
            self.lyrics = lyrics.clone();
        }
    }
}
