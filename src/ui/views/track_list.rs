use async_trait::async_trait;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};

use crate::{
    ui::{
        context::AppContext,
        state::AppState,
        traits::{Action, View},
        util::get_active_track_icon,
    },
    util::colors,
};

pub struct TrackList {
    list_state: ListState,
}

impl Default for TrackList {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
        }
    }
}

#[async_trait]
impl View for TrackList {
    fn render(&mut self, f: &mut Frame, area: Rect, _state: &AppState, ctx: &AppContext) {
        let queue = ctx.audio_system.queue();
        if queue.is_empty() {
            let no_tracks = List::new(vec![ListItem::new("No tracks")]).highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );
            f.render_widget(no_tracks, area);
            return;
        }

        let current_index = if queue.is_empty() {
            None
        } else {
            Some(ctx.audio_system.current_track_index())
        };
        let is_playing = ctx.audio_system.is_playing();

        let items: Vec<ListItem> = queue
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let is_current = Some(i) == current_index;
                let prefix = if is_current {
                    format!("{} ", get_active_track_icon(is_playing))
                } else {
                    "  ".to_string()
                };

                let title = track.title.as_deref().unwrap_or("Unknown Title");
                let mut spans = Vec::with_capacity(4);
                spans.push(Span::raw(prefix));
                spans.push(Span::raw(title));
                spans.push(Span::raw(" - "));

                if let Some(first_artist) = track.artists.first() {
                    spans.push(Span::raw(
                        first_artist.name.as_deref().unwrap_or("Unknown Artist"),
                    ));
                    if track.artists.len() > 1 {
                        spans.push(Span::raw(", ..."));
                    }
                } else {
                    spans.push(Span::raw("Unknown Artist"));
                }

                let mut item = ListItem::new(Line::from(spans));

                if is_current {
                    item = item.style(
                        Style::default()
                            .fg(colors::SECONDARY)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                item
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        if !queue.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    async fn handle_input(
        &mut self,
        key: KeyEvent,
        _state: &AppState,
        ctx: &AppContext,
    ) -> Option<Action> {
        let queue_len = ctx.audio_system.queue().len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if queue_len > 0 {
                    let i = self
                        .list_state
                        .selected()
                        .map_or(0, |i| if i >= queue_len - 1 { i } else { i + 1 });
                    self.list_state.select(Some(i));
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if queue_len > 0 {
                    let i = self
                        .list_state
                        .selected()
                        .map_or(0, |i| if i == 0 { 0 } else { i - 1 });
                    self.list_state.select(Some(i));
                }
                None
            }
            KeyCode::Char('g') => {
                if queue_len > 0 {
                    self.list_state.select(Some(0));
                }
                None
            }
            KeyCode::Char('G') => {
                if queue_len > 0 {
                    self.list_state.select(Some(queue_len - 1));
                }
                None
            }
            KeyCode::Enter => {
                if let Some(i) = self.list_state.selected() {
                    let _ = ctx
                        .event_tx
                        .send(crate::event::events::Event::Play(i as i32));
                }
                None
            }
            _ => None,
        }
    }
}
