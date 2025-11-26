use async_trait::async_trait;
use ratatui::crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use yandex_music::model::info::lyrics::LyricsFormat;

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
    fn render(&mut self, f: &mut Frame, area: Rect, state: &AppState, ctx: &AppContext) {
        if self.lyrics.is_none() && state.data.lyrics.is_some() {
            self.lyrics = state.data.lyrics.clone();
        }

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
        match event {
            Event::TrackStarted(_, _) => {
                self.lyrics = None;
            }
            Event::LyricsFetched(lyrics) => {
                self.lyrics = lyrics.clone();
            }
            _ => {}
        }
    }

    async fn on_mount(&mut self, ctx: &AppContext) {
        if let Some(track) = ctx.audio_system.current_track() {
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
            let api = ctx.api.clone();
            let tx = ctx.event_tx.clone();

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
}
