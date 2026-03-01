use im::Vector;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};
use std::borrow::Cow;
use unicode_width::UnicodeWidthChar;

use crate::{
    app::components::fuzzy::fuzzy_match_positioned,
    app::keymap::Key,
    framework::{signals::Signal, theme::ThemeStyles},
};

pub trait FuzzyItem {
    fn label(&self) -> Cow<'_, str>;
}

pub enum DropdownAction {
    Selected(Option<usize>),
    MultiUpdated(std::collections::HashSet<usize>),
    Handled,
    Ignored,
}

pub struct FuzzyDropdown<T: FuzzyItem + Clone + Send + Sync + 'static> {
    pub items: Signal<Vector<T>>,
    pub query: Signal<String>,
    pub filtered_selection_index: Signal<usize>,
    pub is_open: Signal<bool>,
    pub multi_select: bool,
    pub selected_indices: std::collections::HashSet<usize>,
    state: ListState,
}

impl<T: FuzzyItem + Clone + Send + Sync + 'static> FuzzyDropdown<T> {
    pub fn new(items: Signal<Vector<T>>) -> Self {
        Self {
            items,
            query: Signal::new(String::new()),
            filtered_selection_index: Signal::new(0),
            is_open: Signal::new(false),
            multi_select: false,
            selected_indices: std::collections::HashSet::new(),
            state: ListState::default(),
        }
    }

    fn truncate(text: &str, width: usize) -> String {
        use unicode_width::UnicodeWidthStr;
        let display_width = text.width();
        if display_width > width {
            let mut result = String::new();
            let mut current_width = 0;
            for ch in text.chars() {
                let ch_width = ch.width().unwrap_or(0);
                if current_width + ch_width + 1 > width {
                    result.push('…');
                    break;
                }
                result.push(ch);
                current_width += ch_width;
            }
            result
        } else {
            text.to_string()
        }
    }

    fn create_highlighted_spans(
        text: &str,
        width: usize,
        base_style: Style,
        match_positions: &[usize],
        highlight_style: Style,
    ) -> Vec<Span<'static>> {
        let mut result = Vec::new();
        let mut current_segment = String::new();
        let mut current_width = 0usize;
        let match_set: std::collections::HashSet<usize> = match_positions.iter().copied().collect();

        for (i, ch) in text.chars().enumerate() {
            let ch_width = ch.width().unwrap_or(0);

            if current_width + ch_width + 1 > width {
                if !current_segment.is_empty() {
                    result.push(Span::styled(current_segment.clone(), base_style));
                    current_segment.clear();
                }
                result.push(Span::styled("…".to_string(), base_style));
                #[allow(unused_assignments)]
                {
                    current_width += 1;
                }
                break;
            }

            if match_set.contains(&i) {
                if !current_segment.is_empty() {
                    result.push(Span::styled(current_segment.clone(), base_style));
                    current_segment.clear();
                }
                result.push(Span::styled(ch.to_string(), highlight_style));
            } else {
                current_segment.push(ch);
            }
            current_width += ch_width;
        }

        if !current_segment.is_empty() {
            result.push(Span::styled(current_segment, base_style));
        }

        result
    }

    pub fn with_multi_select(mut self, selected: std::collections::HashSet<usize>) -> Self {
        self.multi_select = true;
        self.selected_indices = selected;
        self
    }

    pub fn filtered_items(&self) -> Vec<(usize, T, Vec<usize>)> {
        let q = self.query.get();
        let items = self.items.get();
        if q.is_empty() {
            items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| (idx, item, Vec::new()))
                .collect()
        } else {
            let indexed: Vec<(usize, T)> = items.into_iter().enumerate().collect();
            let ranked_with_positions = fuzzy_match_positioned(
                &q,
                indexed
                    .iter()
                    .map(|(idx, item)| (*idx, item.label().into_owned())),
            );

            let item_by_index = indexed
                .into_iter()
                .collect::<std::collections::HashMap<usize, T>>();

            ranked_with_positions
                .into_iter()
                .filter_map(|(idx, positions)| {
                    item_by_index
                        .get(&idx)
                        .cloned()
                        .map(|item| (idx, item, positions))
                })
                .collect()
        }
    }

    pub fn open(&self) {
        self.is_open.set(true);
        self.query.set(String::new());
        self.filtered_selection_index.set(0);
    }

    pub fn close(&self) {
        self.is_open.set(false);
        self.query.set(String::new());
    }

    pub fn handle_key(&mut self, key: &Key) -> DropdownAction {
        if !self.is_open.get() {
            return DropdownAction::Ignored;
        }

        let filtered = self.filtered_items();
        let display_none = self.query.get().is_empty() || filtered.is_empty();
        let total = filtered.len() + if display_none { 1 } else { 0 };
        let current_sel = self.filtered_selection_index.get();

        match key {
            Key::Esc => {
                self.close();
                DropdownAction::Ignored
            }
            Key::Enter => {
                if display_none && current_sel == 0 {
                    if self.multi_select {
                        self.selected_indices.clear();
                        DropdownAction::MultiUpdated(self.selected_indices.clone())
                    } else {
                        self.close();
                        DropdownAction::Selected(None)
                    }
                } else {
                    let adjusted_sel = if display_none {
                        current_sel.saturating_sub(1)
                    } else {
                        current_sel
                    };
                    if adjusted_sel < filtered.len() {
                        let (abs_index, _, _) = filtered[adjusted_sel];
                        if self.multi_select {
                            if self.selected_indices.contains(&abs_index) {
                                self.selected_indices.remove(&abs_index);
                            } else {
                                self.selected_indices.insert(abs_index);
                            }
                            DropdownAction::MultiUpdated(self.selected_indices.clone())
                        } else {
                            self.close();
                            DropdownAction::Selected(Some(abs_index))
                        }
                    } else if filtered.is_empty() {
                        if self.multi_select {
                            self.selected_indices.clear();
                            DropdownAction::MultiUpdated(self.selected_indices.clone())
                        } else {
                            self.close();
                            DropdownAction::Selected(None)
                        }
                    } else {
                        DropdownAction::Handled
                    }
                }
            }
            Key::Backspace => {
                let mut q = self.query.get();
                if !q.is_empty() {
                    q.pop();
                    self.query.set(q);
                    self.filtered_selection_index.set(0);
                }
                DropdownAction::Handled
            }
            Key::Char(c)
                if c.is_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace() =>
            {
                let mut q = self.query.get();
                q.push(*c);
                self.query.set(q);
                self.filtered_selection_index.set(0);
                DropdownAction::Handled
            }
            Key::Up | Key::Char('A') => {
                if current_sel > 0 {
                    self.filtered_selection_index.set(current_sel - 1);
                }
                DropdownAction::Handled
            }
            Key::Down | Key::Char('B') => {
                if current_sel + 1 < total {
                    self.filtered_selection_index.set(current_sel + 1);
                }
                DropdownAction::Handled
            }
            _ => DropdownAction::Handled,
        }
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect, styles: &ThemeStyles, max_height: u16) {
        if !self.is_open.get() {
            return;
        }

        let filtered = self.filtered_items();
        let display_none = self.query.get().is_empty() || filtered.is_empty();
        let total = filtered.len() + if display_none { 1 } else { 0 };
        let height = total.min(max_height as usize);

        let mut render_area = area;
        render_area.height = (height as u16 + 2).min(max_height);

        frame.render_widget(Clear, render_area);
        frame.render_widget(Block::default().style(styles.text), render_area);

        let q = self.query.get();
        let title = if q.is_empty() {
            " Search... ".to_string()
        } else {
            format!(" {} ", q)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(styles.accent)
            .title(Span::styled(title, styles.accent))
            .style(styles.text);

        let mut list_items = Vec::new();
        if display_none {
            list_items.push(ListItem::new("None").style(styles.text));
        }

        let available_width = render_area.width.saturating_sub(4);

        list_items.extend(filtered.iter().map(|(idx, item, positions)| {
            let label = item.label();
            let prefix = if self.multi_select {
                if self.selected_indices.contains(idx) {
                    "󰡖 "
                } else {
                    "󰄱 "
                }
            } else {
                ""
            };

            let prefix_width = prefix
                .chars()
                .map(|c| c.width().unwrap_or(0))
                .sum::<usize>();
            let label_width = (available_width as usize).saturating_sub(prefix_width);

            use ratatui::text::Line;
            let mut spans = Vec::new();

            if !prefix.is_empty() {
                spans.push(Span::styled(prefix, styles.text));
            }

            if positions.is_empty() {
                let truncated = Self::truncate(&label, label_width);
                spans.push(Span::styled(truncated, styles.text));
            } else {
                let highlight_style = styles.accent.add_modifier(Modifier::BOLD);

                spans.extend(Self::create_highlighted_spans(
                    &label,
                    label_width,
                    styles.text,
                    positions,
                    highlight_style,
                ));
            }

            ListItem::new(Line::from(spans)).style(Style::default())
        }));

        let highlight_style = styles.selected.add_modifier(Modifier::BOLD);
        let list = List::new(list_items)
            .block(block)
            .highlight_style(highlight_style)
            .style(styles.text);

        self.state.select(Some(self.filtered_selection_index.get()));
        frame.render_stateful_widget(list, render_area, &mut self.state);
    }
}
