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
        context::{AppContext, GlobalUiState},
        traits::{Action, Component},
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

impl Component for TrackList {
    fn render(&mut self, f: &mut Frame, area: Rect, ctx: &AppContext, state: &GlobalUiState) {
        if state.is_loading {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let symbol = spinner[state.spinner_index % spinner.len()];
            let text = format!("{} Loading...", symbol);
            let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
            let y = area.y + area.height / 2;

            f.buffer_mut()
                .set_string(x, y, text, Style::default().fg(colors::PRIMARY));
            return;
        }

        let queue = ctx.audio_system.queue();
        let current_index = if queue.is_empty() {
            None
        } else {
            Some(ctx.audio_system.current_track_index())
        };

        let items: Vec<ListItem> = queue
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let title = track.title.as_deref().unwrap_or("Unknown Title");
                let mut spans = Vec::with_capacity(3);
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

                if Some(i) == current_index {
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

    fn handle_input(
        &mut self,
        key: KeyEvent,
        ctx: &AppContext,
        _state: &GlobalUiState,
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
