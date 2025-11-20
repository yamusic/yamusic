use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};

use crate::{
    ui::{
        context::{AppContext, GlobalUiState},
        traits::{Action, Component},
    },
    util::colors,
};

#[derive(Default)]
pub struct Playlists {
    pub list_state: ListState,
}

impl Component for Playlists {
    fn render(&mut self, f: &mut Frame, area: Rect, _ctx: &AppContext, state: &GlobalUiState) {
        if state.is_loading && state.playlists.is_empty() {
            let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let symbol = spinner[state.spinner_index % spinner.len()];
            let text = format!("{} Loading playlists...", symbol);
            let x = area.x + (area.width.saturating_sub(text.len() as u16)) / 2;
            let y = area.y + area.height / 2;

            f.buffer_mut()
                .set_string(x, y, text, Style::default().fg(colors::PRIMARY));
            return;
        }

        let items: Vec<ListItem> = state
            .playlists
            .iter()
            .map(|playlist| {
                let title = &playlist.title;
                let count = playlist.track_count;
                let content = format!("{} ({} tracks)", title, count);
                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        if !state.playlists.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        ctx: &AppContext,
        state: &GlobalUiState,
    ) -> Option<Action> {
        let len = state.playlists.len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let i = self
                        .list_state
                        .selected()
                        .map_or(0, |i| if i >= len - 1 { i } else { i + 1 });
                    self.list_state.select(Some(i));
                }
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
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
                    if let Some(playlist) = state.playlists.get(i) {
                        let _ = ctx
                            .event_tx
                            .send(crate::event::events::Event::PlaylistSelected(
                                playlist.clone(),
                            ));
                    }
                }
                None
            }
            _ => None,
        }
    }
}
