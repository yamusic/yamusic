use im::Vector;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    app::{
        actions::Action,
        components::{DropdownAction, FuzzyDropdown, Spinner},
        keymap::Key,
        state::wave::{StationCategory, StationItem},
    },
    framework::{signals::Signal, theme::ThemeStyles},
};
use std::collections::HashSet;

pub struct HomeView {
    waves: Signal<Vector<StationCategory>>,
    loading: Signal<bool>,
    selections: Vec<HashSet<usize>>,
    show_settings: Signal<bool>,
    focused_index: usize,
    dropdown: Option<FuzzyDropdown<StationItem>>,
    theme: Signal<ThemeStyles>,
}

impl HomeView {
    pub fn new(
        waves: Signal<Vector<StationCategory>>,
        loading: Signal<bool>,
        theme: Signal<ThemeStyles>,
    ) -> Self {
        Self {
            waves,
            loading,
            selections: Vec::new(),
            show_settings: Signal::new(false),
            focused_index: 0,
            dropdown: None,
            theme,
        }
    }

    pub fn is_popup_open(&self) -> bool {
        self.show_settings.get()
    }

    fn build_seeds(&self) -> Vec<String> {
        let waves = self.waves.with(|w| w.clone());
        let mut seeds = vec!["user:onyourwave".to_string()];
        for (i, indices) in self.selections.iter().enumerate() {
            if let Some(wave) = waves.get(i) {
                for &idx in indices {
                    if let Some(item) = wave.items.get(idx) {
                        seeds.push(item.seed.clone());
                    }
                }
            }
        }
        seeds
    }

    fn build_toast_msg(&self) -> Vec<Line<'static>> {
        let waves = self.waves.with(|w| w.clone());
        let mut lines = vec![Line::from(vec![Span::styled(
            "Starting a new wave",
            Style::default().add_modifier(Modifier::BOLD),
        )])];
        let mut any_selection = false;

        for (i, indices) in self.selections.iter().enumerate() {
            if indices.is_empty() {
                continue;
            } else if !any_selection {
                lines.push(Line::from(""));
                any_selection = true;
            }

            if let Some(wave) = waves.get(i) {
                let category_name = &wave.title;
                let selected_items: Vec<_> = indices
                    .iter()
                    .filter_map(|&idx| wave.items.get(idx).map(|item| item.label.clone()))
                    .collect();

                if selected_items.is_empty() {
                    continue;
                }

                let display_text = if selected_items.len() == 1 {
                    format!("{}: {}", category_name, selected_items[0])
                } else {
                    format!(
                        "{}: {} and {} more",
                        category_name,
                        selected_items[0],
                        selected_items.len() - 1
                    )
                };

                lines.push(Line::from(display_text));
            }
        }

        lines
    }

    pub fn handle_key(&mut self, key: &Key, prefix: Option<char>) -> Action {
        if let Some('g') = prefix
            && *key == Key::Char('g')
        {
            self.focused_index = 0;
            return Action::Redraw;
        }
        if prefix.is_some() {
            return Action::None;
        }

        let waves_len = self.waves.with(|w| w.len());
        if self.selections.len() != waves_len {
            self.selections = vec![HashSet::new(); waves_len];
        }

        if *key == Key::Char('w') && self.dropdown.is_none() {
            if self.show_settings.get() {
                let seeds = self.build_seeds();
                let toast_message = self.build_toast_msg();
                self.show_settings.set(false);
                return Action::StartWave {
                    seeds,
                    title: None,
                    toast_message: Some(toast_message),
                };
            } else {
                self.show_settings.set(true);
                return Action::Redraw;
            }
        }

        if self.show_settings.get() {
            return self.handle_settings_key(key);
        }

        match key {
            Key::Char('r') => Action::RefreshWaves,
            _ => Action::None,
        }
    }

    fn handle_settings_key(&mut self, key: &Key) -> Action {
        let waves = self.waves.with(|w| w.clone());

        if let Some(dropdown) = &mut self.dropdown {
            match dropdown.handle_key(key) {
                DropdownAction::Selected(idx) => {
                    self.selections[self.focused_index].clear();
                    if let Some(idx) = idx {
                        self.selections[self.focused_index].insert(idx);
                    }
                    self.dropdown = None;
                }
                DropdownAction::MultiUpdated(indices) => {
                    self.selections[self.focused_index] = indices;
                }
                DropdownAction::Handled => {}
                DropdownAction::Ignored => {
                    if *key == Key::Esc {
                        self.dropdown = None;
                    }
                }
            }
            return Action::Redraw;
        }

        match key {
            Key::Char('R') => {
                for sel in self.selections.iter_mut() {
                    sel.clear();
                }
                return Action::Redraw;
            }
            Key::Char('r') => {
                if self.focused_index < self.selections.len() {
                    self.selections[self.focused_index].clear();
                }
                return Action::Redraw;
            }
            Key::Up | Key::Char('k') => {
                if self.focused_index > 0 {
                    self.focused_index -= 1;
                }
            }
            Key::Down | Key::Char('j') => {
                if self.focused_index < waves.len().saturating_sub(1) {
                    self.focused_index += 1;
                }
            }
            Key::Enter => {
                if self.focused_index < waves.len() {
                    let items = waves[self.focused_index].items.clone();
                    let current_indices = self.selections[self.focused_index].clone();
                    let dropdown =
                        FuzzyDropdown::new(Signal::new(items)).with_multi_select(current_indices);
                    dropdown.open();

                    if let Some(&first_idx) = self.selections[self.focused_index].iter().next() {
                        let total_filtered = dropdown.filtered_items().len();
                        dropdown
                            .filtered_selection_index
                            .set(first_idx.min(total_filtered) + 1);
                    }
                    self.dropdown = Some(dropdown);
                }
            }
            Key::Esc => {
                self.show_settings.set(false);
            }
            Key::Char('w') => {
                let seeds = self.build_seeds();
                let toast_message = self.build_toast_msg();
                self.show_settings.set(false);
                return Action::StartWave {
                    seeds,
                    title: None,
                    toast_message: Some(toast_message),
                };
            }
            _ => {}
        }

        Action::Redraw
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        if self.show_settings.get() {
            self.render_settings(frame, area);
        }
    }

    fn render_settings(&mut self, frame: &mut Frame, area: Rect) {
        let styles = self.theme.get();
        let waves = self.waves.get();

        if self.selections.len() != waves.len() {
            self.selections = vec![HashSet::new(); waves.len()];
        }

        let overlay_area = centered_rect(area, 60, 80);
        f_render_block(frame, overlay_area, " My Wave Settings ", &styles);

        if self.loading.get() {
            let spinner_area = Rect {
                x: overlay_area.x + 1,
                y: overlay_area.y + overlay_area.height / 2,
                width: overlay_area.width.saturating_sub(2),
                height: 1,
            };
            Spinner::new()
                .with_label("Loading waves...")
                .view(frame, spinner_area);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .margin(1)
            .split(overlay_area);

        let content_area = chunks[0];

        let mut constraints: Vec<Constraint> = Vec::new();
        for _ in 0..waves.len() {
            constraints.push(Constraint::Length(3));
        }
        constraints.push(Constraint::Min(0));

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .margin(1)
            .split(content_area);

        for (i, wave) in waves.iter().enumerate() {
            let is_focused = i == self.focused_index;
            let indices = &self.selections[i];

            let selected_text = if indices.is_empty() {
                "None".to_string()
            } else if indices.len() == 1 {
                let idx = *indices.iter().next().unwrap();
                wave.items
                    .get(idx)
                    .map(|item| item.label.clone())
                    .unwrap_or_else(|| "None".to_string())
            } else {
                format!("{} selected", indices.len())
            };

            let (border_style, title_style, text_style) = if is_focused {
                (styles.accent, styles.accent, styles.accent)
            } else {
                (styles.block, styles.text, styles.text)
            };

            let paragraph = Paragraph::new(Span::styled(selected_text, text_style)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(wave.title.clone(), title_style))
                    .border_style(border_style),
            );

            if i < rows.len() {
                frame.render_widget(paragraph, rows[i]);
            }
        }

        if let Some(dropdown) = &mut self.dropdown
            && self.focused_index < rows.len()
        {
            let anchor = rows[self.focused_index];
            let dropdown_area = Rect {
                x: anchor.x + anchor.width / 2,
                y: anchor.y + 1,
                width: anchor.width / 2,
                height: overlay_area.bottom().saturating_sub(anchor.y + 1),
            };
            dropdown.view(frame, dropdown_area, &styles, 10);
        }

        let instructions = Paragraph::new(
            "↑↓: Navigate | Enter: Select | Esc: Close Settings | w: Start Wave | r/R: Reset selection",
        )
        .alignment(Alignment::Center)
        .style(styles.text_muted);
        frame.render_widget(instructions, chunks[1]);
    }
}

fn f_render_block(frame: &mut Frame, area: Rect, title: &str, styles: &ThemeStyles) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(styles.block_focused)
        .style(styles.text);
    frame.render_widget(block, area);
}

fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl Default for HomeView {
    fn default() -> Self {
        panic!("HomeView requires wave/loading signals - use new() instead");
    }
}
