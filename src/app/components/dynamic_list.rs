use std::sync::Arc;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{
        Block, Borders, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use crate::{
    app::theme::theme,
    app::{
        actions::Action,
        components::fuzzy::fuzzy_match_positioned,
        data::{DataSource, ItemRenderer, ListItem, MatchHighlights, SearchScope},
        keymap::Key,
    },
    cache::image::ImageCache,
    framework::signals::Signal,
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct FuzzyFields {
    pub full: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DynamicListConfig {
    pub prefetch_distance: usize,
    pub show_scrollbar: bool,
    pub wrap_selection: bool,
    pub highlight_style: Style,
    pub highlight_symbol: String,
}

impl Default for DynamicListConfig {
    fn default() -> Self {
        Self {
            prefetch_distance: 5,
            show_scrollbar: true,
            wrap_selection: false,
            highlight_style: Style::default().add_modifier(Modifier::BOLD),
            highlight_symbol: "> ".to_string(),
        }
    }
}

pub struct DynamicList<T> {
    source: Arc<dyn DataSource<T>>,
    renderer: Arc<dyn ItemRenderer<T>>,
    selection: Signal<usize>,
    playing_index: Signal<Option<usize>>,
    list_state: ListState,
    config: DynamicListConfig,
    visible_range: (usize, usize),
    title: Option<String>,
    search_query: Signal<String>,
    search_mode: Signal<bool>,
    search_scope: Signal<SearchScope>,
    fuzzy_labeler: Option<Arc<dyn Fn(&T) -> FuzzyFields + Send + Sync>>,
    image_protocols: HashMap<String, StatefulProtocol>,
}

impl<T: Clone + Send + Sync + 'static> DynamicList<T> {
    pub fn new(source: Arc<dyn DataSource<T>>, renderer: Arc<dyn ItemRenderer<T>>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let mut config = DynamicListConfig::default();
        config.highlight_style = theme().selected.add_modifier(Modifier::BOLD);

        Self {
            source,
            renderer,
            selection: Signal::new(0),
            playing_index: Signal::new(None),
            list_state,
            config,
            visible_range: (0, 0),
            title: None,
            search_query: Signal::new(String::new()),
            search_mode: Signal::new(false),
            search_scope: Signal::new(SearchScope::Full),
            fuzzy_labeler: None,
            image_protocols: HashMap::new(),
        }
    }

    pub fn with_fuzzy<F>(mut self, labeler: F) -> Self
    where
        F: Fn(&T) -> FuzzyFields + Send + Sync + 'static,
    {
        self.fuzzy_labeler = Some(Arc::new(labeler));
        self
    }

    pub fn with_config(mut self, config: DynamicListConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_playing_index(mut self, playing_index: Signal<Option<usize>>) -> Self {
        self.playing_index = playing_index;
        self
    }

    pub fn selection_signal(&self) -> Signal<usize> {
        self.selection.clone()
    }

    pub fn selected(&self) -> usize {
        self.selection.get()
    }

    pub fn selected_item(&self) -> Option<T> {
        let idx = self.selection.get();
        self.source.range(idx..idx + 1).into_iter().next()
    }

    pub fn select(&mut self, index: usize) {
        let total = self.source.total().unwrap_or(0);
        let clamped = index.min(total.saturating_sub(1));
        self.selection.set(clamped);
        self.list_state.select(Some(clamped));
        self.maybe_load_more();
    }

    pub fn select_prev(&mut self) {
        if let Some(active) = self.active_indices() {
            if active.is_empty() {
                self.selection.set(0);
                self.list_state.select(Some(0));
                return;
            }

            self.ensure_selection(&active);

            let current = self.selection.get();
            let current_pos = active
                .iter()
                .position(|(idx, _)| *idx == current)
                .unwrap_or(0);
            let next_pos = if current_pos == 0 {
                if self.config.wrap_selection {
                    active.len() - 1
                } else {
                    0
                }
            } else {
                current_pos - 1
            };
            let (next_abs, _) = active[next_pos];

            self.selection.set(next_abs);
            self.list_state.select(Some(next_pos));
            return;
        }

        let current = self.selection.get();
        let total = self.source.total().unwrap_or(0);

        let new_index = if current == 0 {
            if self.config.wrap_selection && total > 0 {
                total - 1
            } else {
                0
            }
        } else {
            current - 1
        };

        self.selection.set(new_index);
        self.list_state.select(Some(new_index));
    }

    pub fn select_next(&mut self) {
        if let Some(active) = self.active_indices() {
            if active.is_empty() {
                self.selection.set(0);
                self.list_state.select(Some(0));
                return;
            }

            self.ensure_selection(&active);

            let current = self.selection.get();
            let current_pos = active
                .iter()
                .position(|(idx, _)| *idx == current)
                .unwrap_or(0);
            let next_pos = if current_pos + 1 >= active.len() {
                if self.config.wrap_selection {
                    0
                } else {
                    active.len() - 1
                }
            } else {
                current_pos + 1
            };
            let (next_abs, _) = active[next_pos];

            self.selection.set(next_abs);
            self.list_state.select(Some(next_pos));
            self.maybe_load_more();
            return;
        }

        let current = self.selection.get();
        let total = self.source.total().unwrap_or(0);

        let new_index = if total == 0 {
            0
        } else if current >= total - 1 {
            if self.config.wrap_selection {
                0
            } else {
                total - 1
            }
        } else {
            current + 1
        };

        self.selection.set(new_index);
        self.list_state.select(Some(new_index));
        self.maybe_load_more();
    }

    pub fn select_first(&mut self) {
        if let Some(active) = self.active_indices()
            && let Some((first, _)) = active.first()
        {
            self.selection.set(*first);
            self.list_state.select(Some(0));
            return;
        }

        self.selection.set(0);
        self.list_state.select(Some(0));
    }

    pub fn select_last(&mut self) {
        if let Some(active) = self.active_indices() {
            if let Some((last, _)) = active.last() {
                self.selection.set(*last);
                self.list_state.select(Some(active.len().saturating_sub(1)));
                self.maybe_load_more();
            }
            return;
        }

        let total = self.source.total().unwrap_or(0);
        if total > 0 {
            let last = total - 1;
            self.selection.set(last);
            self.list_state.select(Some(last));
            self.maybe_load_more();
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        if let Some(active) = self.active_indices() {
            if active.is_empty() {
                self.selection.set(0);
                self.list_state.select(Some(0));
                return;
            }

            self.ensure_selection(&active);

            let current = self.selection.get();
            let current_pos = active
                .iter()
                .position(|(idx, _)| *idx == current)
                .unwrap_or(0);
            let new_pos = current_pos.saturating_sub(page_size);
            let (new_abs, _) = active[new_pos];

            self.selection.set(new_abs);
            self.list_state.select(Some(new_pos));
            return;
        }

        let current = self.selection.get();
        let new_index = current.saturating_sub(page_size);
        self.selection.set(new_index);
        self.list_state.select(Some(new_index));
    }

    pub fn page_down(&mut self, page_size: usize) {
        if let Some(active) = self.active_indices() {
            if active.is_empty() {
                self.selection.set(0);
                self.list_state.select(Some(0));
                return;
            }

            self.ensure_selection(&active);

            let current = self.selection.get();
            let current_pos = active
                .iter()
                .position(|(idx, _)| *idx == current)
                .unwrap_or(0);
            let new_pos = (current_pos + page_size).min(active.len().saturating_sub(1));
            let (new_abs, _) = active[new_pos];

            self.selection.set(new_abs);
            self.list_state.select(Some(new_pos));
            self.maybe_load_more();
            return;
        }

        let current = self.selection.get();
        let total = self.source.total().unwrap_or(0);
        let new_index = (current + page_size).min(total.saturating_sub(1));
        self.selection.set(new_index);
        self.list_state.select(Some(new_index));
        self.maybe_load_more();
    }

    fn maybe_load_more(&self) {
        let selected = self.selection.get();
        let loaded = self.source.range(0..usize::MAX).len();
        let total = self.source.total();

        if selected + self.config.prefetch_distance >= loaded && total.is_none_or(|t| loaded < t) {
            let range_start = loaded;
            let range_end = loaded + self.config.prefetch_distance * 2;
            self.source.request_range(range_start..range_end);
        }
    }

    fn calculate_visible_range(
        &self,
        area_height: u16,
        total: usize,
        selected_pos: usize,
        item_stride: usize,
    ) -> (usize, usize) {
        let stride = item_stride.max(1);
        let visible_count = ((area_height as usize) / stride).max(1);

        if total == 0 {
            return (0, 0);
        }

        let half = visible_count / 2;
        let start = selected_pos.saturating_sub(half);
        let end = (start + visible_count).min(total);

        let start = if end == total && total > visible_count {
            total - visible_count
        } else {
            start
        };
        (start, end)
    }

    fn estimate_item_stride(&self, list_width: u16, selected_abs: usize) -> usize {
        let sample = self
            .source
            .range(selected_abs..selected_abs.saturating_add(1))
            .into_iter()
            .next()
            .or_else(|| self.source.range(0..1).into_iter().next());

        if let Some(item) = sample {
            let rendered = self.renderer.render_with_context(
                &item,
                selected_abs,
                false,
                self.playing_index.get() == Some(selected_abs),
                list_width.max(1),
                &MatchHighlights::default(),
            );
            return rendered.height as usize + 1;
        }

        2
    }

    fn active_indices(&self) -> Option<Vec<(usize, MatchHighlights)>> {
        let query = self.search_query.get();
        let labeler = self.fuzzy_labeler.as_ref()?;

        if !self.search_mode.get() {
            return None;
        }

        let scope = self.search_scope.get();

        if query.is_empty() {
            let all_items = self.source.range(0..usize::MAX);
            return Some(
                all_items
                    .iter()
                    .enumerate()
                    .map(|(idx, _)| {
                        (
                            idx,
                            MatchHighlights {
                                search_scope: Some(scope),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
            );
        }

        let all_items = self.source.range(0..usize::MAX);

        let fields_per_item: Vec<(usize, FuzzyFields)> = all_items
            .iter()
            .enumerate()
            .map(|(idx, item)| (idx, labeler(item)))
            .collect();

        let ranked = fuzzy_match_positioned(
            &query,
            fields_per_item.iter().map(|(idx, fields)| {
                let searchable = match scope {
                    SearchScope::Full => fields.full.clone(),
                    SearchScope::Title => fields.title.clone().unwrap_or_default(),
                    SearchScope::Artist => fields.artist.clone().unwrap_or_default(),
                    SearchScope::Album => fields.album.clone().unwrap_or_default(),
                };
                (*idx, searchable)
            }),
        );

        Some(
            ranked
                .into_iter()
                .map(|(idx, positions)| {
                    let fields = &fields_per_item.iter().find(|(i, _)| *i == idx).unwrap().1;
                    let highlights = match scope {
                        SearchScope::Title => MatchHighlights {
                            title: positions,
                            search_scope: Some(SearchScope::Title),
                            ..Default::default()
                        },
                        SearchScope::Artist => MatchHighlights {
                            artist: positions,
                            search_scope: Some(SearchScope::Artist),
                            ..Default::default()
                        },
                        SearchScope::Album => MatchHighlights {
                            album: positions,
                            search_scope: Some(SearchScope::Album),
                            ..Default::default()
                        },
                        SearchScope::Full => {
                            let title_len = fields.title.as_ref().map_or(0, |s| s.chars().count());
                            let artist_len =
                                fields.artist.as_ref().map_or(0, |s| s.chars().count());
                            let artist_offset = title_len + 1;
                            let album_offset = artist_offset + artist_len + 1;
                            let mut hl = MatchHighlights::default();
                            for pos in positions {
                                if pos < title_len {
                                    hl.title.push(pos);
                                } else if pos >= artist_offset && pos < artist_offset + artist_len {
                                    hl.artist.push(pos - artist_offset);
                                } else if pos >= album_offset {
                                    hl.album.push(pos - album_offset);
                                }
                            }
                            hl.search_scope = Some(SearchScope::Full);
                            hl
                        }
                    };
                    (idx, highlights)
                })
                .collect(),
        )
    }

    fn ensure_selection(&mut self, active: &[(usize, MatchHighlights)]) {
        let selected = self.selection.get();
        if !active.iter().any(|(idx, _)| *idx == selected)
            && let Some((first, _)) = active.first()
        {
            self.selection.set(*first);
        }
    }

    fn can_start_search(&self) -> bool {
        self.fuzzy_labeler.is_some()
    }

    fn clear_search(&self) {
        self.search_mode.set(false);
        self.search_query.set(String::new());
    }

    fn render_item(
        &self,
        item: &T,
        actual_index: usize,
        is_selected: bool,
        is_playing: bool,
        list_width: u16,
        highlights: &MatchHighlights,
    ) -> ListItem<'static> {
        let mut rendered = self.renderer.render_with_context(
            item,
            actual_index,
            is_selected,
            is_playing,
            list_width,
            highlights,
        );

        let highlight_w = self.config.highlight_symbol.chars().count() as u16;
        let reserve_highlight = rendered.prefix_lines.is_none();
        let prefix_w = rendered
            .prefix_lines
            .as_ref()
            .map(|lines| lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16)
            .unwrap_or(0);

        let text_after_prefix =
            list_width.saturating_sub(if reserve_highlight { highlight_w } else { 0 } + prefix_w);
        let cover_reserved = if rendered.cover_url.is_some() {
            let img_w = (rendered.height * 2).min(text_after_prefix / 4).max(2);
            img_w.saturating_add(1)
        } else {
            0
        };

        let effective_text_width = list_width
            .saturating_sub(if reserve_highlight { highlight_w } else { 0 })
            .saturating_sub(prefix_w)
            .saturating_sub(cover_reserved)
            .max(1);

        if effective_text_width < list_width {
            rendered = self.renderer.render_with_context(
                item,
                actual_index,
                is_selected,
                is_playing,
                effective_text_width,
                highlights,
            );
        }

        rendered
    }

    pub fn handle_key(&mut self, key: &Key, prefix: Option<char>) -> Action {
        if self.search_mode.get() {
            match key {
                Key::Esc => {
                    self.clear_search();
                    self.select_first();
                    return Action::Redraw;
                }
                Key::Enter => {
                    let selected = self.selection.get();
                    self.search_mode.set(false);
                    self.search_query.set(String::new());
                    self.search_scope.set(SearchScope::Full);
                    self.selection.set(selected);
                    self.list_state.select(Some(selected));
                    return Action::Redraw;
                }
                Key::Tab => {
                    self.search_scope.update(|s| *s = s.next());
                    self.select_first();
                    return Action::Redraw;
                }
                Key::BackTab => {
                    self.search_scope.update(|s| *s = s.prev());
                    self.select_first();
                    return Action::Redraw;
                }
                Key::Backspace => {
                    self.search_query.update(|q| {
                        q.pop();
                    });
                    self.select_first();
                    return Action::Redraw;
                }
                Key::Char(c)
                    if c.is_alphanumeric()
                        || c.is_ascii_punctuation()
                        || c.is_ascii_whitespace() =>
                {
                    self.search_query.update(|q| q.push(*c));
                    self.select_first();
                    return Action::Redraw;
                }
                _ => {}
            }
        }

        if self.can_start_search() && prefix.is_none() && *key == Key::Char('/') {
            self.search_mode.set(true);
            self.search_query.set(String::new());
            self.select_first();
            return Action::Redraw;
        }

        if let Some(p) = prefix {
            match (p, key) {
                ('g', Key::Char('g')) => {
                    self.select_first();
                    return Action::Redraw;
                }
                _ => return Action::None,
            }
        }

        match key {
            Key::Up | Key::Char('k') => {
                self.select_prev();
                Action::Redraw
            }
            Key::Down | Key::Char('j') => {
                self.select_next();
                Action::Redraw
            }
            Key::Home => {
                self.select_first();
                Action::Redraw
            }
            Key::End | Key::Char('G') => {
                self.select_last();
                Action::Redraw
            }
            Key::PageUp => {
                self.page_up(10);
                Action::Redraw
            }
            Key::PageDown => {
                self.page_down(10);
                Action::Redraw
            }
            _ => Action::None,
        }
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        let _ = ImageCache::global().version().track();

        let colors = theme();
        let text_style = Style::default().fg(colors.text.primary);
        let selected_style = colors.selected;
        let unfocused_border = colors.unfocused_border;
        let accent_style = Style::default().fg(colors.accent.primary);
        let playing = self.playing_index.get();
        let (list_area, search_active) = if self.search_mode.get() {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area);
            (chunks[1], true)
        } else {
            (area, false)
        };
        let has_border = self.title.is_some();
        let inner_height = if has_border {
            list_area.height.saturating_sub(2)
        } else {
            list_area.height
        };
        let selected = self.selection.get();
        let item_stride = self.estimate_item_stride(list_area.width, selected);
        let active = self.active_indices();

        let (total, selected_pos, list_items): (usize, usize, Vec<ListItem<'static>>) =
            if let Some(active_indices) = active {
                let all_items = self.source.range(0..usize::MAX);
                if active_indices.is_empty() {
                    (0, 0, Vec::new())
                } else {
                    self.ensure_selection(&active_indices);
                    let selected_abs = self.selection.get();
                    let selected_pos = active_indices
                        .iter()
                        .position(|(idx, _)| *idx == selected_abs)
                        .unwrap_or(0);
                    let (start, end) = self.calculate_visible_range(
                        inner_height,
                        active_indices.len(),
                        selected_pos,
                        item_stride,
                    );
                    self.visible_range = (start, end);

                    let items = active_indices[start..end]
                        .iter()
                        .filter_map(|(abs_idx, highlights)| {
                            all_items
                                .get(*abs_idx)
                                .cloned()
                                .map(|item| (*abs_idx, item, highlights.clone()))
                        })
                        .collect::<Vec<_>>();

                    let list_items = items
                        .iter()
                        .map(|(actual_index, item, highlights)| {
                            let is_selected = *actual_index == selected_abs;
                            let is_playing = playing == Some(*actual_index);
                            self.render_item(
                                item,
                                *actual_index,
                                is_selected,
                                is_playing,
                                list_area.width,
                                highlights,
                            )
                        })
                        .collect();

                    (active_indices.len(), selected_pos, list_items)
                }
            } else {
                let total = self.source.total().unwrap_or(0);
                let (start, end) =
                    self.calculate_visible_range(inner_height, total, selected, item_stride);
                self.visible_range = (start, end);

                if !self
                    .source
                    .is_loaded(start..end + self.config.prefetch_distance)
                {
                    self.source
                        .request_range(start..end + self.config.prefetch_distance);
                }

                let items = self.source.range(start..end);

                let list_items = items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| {
                        let actual_index = start + i;
                        let is_selected = actual_index == selected;
                        let is_playing = playing == Some(actual_index);
                        self.render_item(
                            item,
                            actual_index,
                            is_selected,
                            is_playing,
                            list_area.width,
                            &MatchHighlights::default(),
                        )
                    })
                    .collect();

                (total, selected, list_items)
            };

        let mut block = Block::default().borders(Borders::NONE);
        if let Some(title) = &self.title {
            let query = self.search_query.get();
            let search_marker = if self.search_mode.get() { " /" } else { "" };
            let title = if query.is_empty() {
                format!("{}{}", title, search_marker)
            } else {
                format!("{} /{}{}", title, query, search_marker)
            };
            block = block
                .title(title)
                .borders(Borders::ALL)
                .border_style(unfocused_border);
        }

        let inner_area = block.inner(list_area);

        if search_active {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(area);
            let search_area = chunks[0];
            let q = self.search_query.get();
            let scope = self.search_scope.get();
            let prompt = format!(" [{}] {}", scope.label(), q);
            let search_block = Block::default()
                .borders(Borders::ALL)
                .border_style(accent_style);
            let paragraph = Paragraph::new(prompt).block(search_block).style(text_style);
            frame.render_widget(paragraph, search_area);

            frame.render_widget(block, list_area);
        } else {
            frame.render_widget(block, area);
        }

        let visible_urls: std::collections::HashSet<String> = list_items
            .iter()
            .filter_map(|i| i.cover_url.clone())
            .collect();
        self.image_protocols
            .retain(|url, _| visible_urls.contains(url));

        let mut current_y = inner_area.y;

        let list_items_len = list_items.len();
        for (i, item) in list_items.into_iter().enumerate() {
            if current_y >= inner_area.y + inner_area.height {
                break;
            }

            let item_height = item.height;
            let is_selected = i == selected_pos.saturating_sub(self.visible_range.0);

            let remaining_height = inner_area.y + inner_area.height - current_y;
            if remaining_height < item_height {
                break;
            }

            let item_area = Rect {
                x: inner_area.x,
                y: current_y,
                width: inner_area.width,
                height: item_height,
            };

            if is_selected {
                frame.render_widget(
                    ratatui::widgets::Block::default().style(
                        Style::default().bg(selected_style.bg.unwrap_or(colors.bg.selection)),
                    ),
                    item_area,
                );
            }

            let mut text_area = item_area;
            let mut prefix_area = None;
            let reserve_highlight = item.prefix_lines.is_none();

            if reserve_highlight && is_selected {
                let prefix_width = self.config.highlight_symbol.chars().count() as u16;
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(prefix_width), Constraint::Min(0)])
                    .split(text_area);
                prefix_area = Some(chunks[0]);
                text_area = chunks[1];
            } else if reserve_highlight {
                let prefix_width = self.config.highlight_symbol.chars().count() as u16;
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Length(prefix_width), Constraint::Min(0)])
                    .split(text_area);
                text_area = chunks[1];
            }

            if let Some(prefix) = prefix_area {
                let p = ratatui::widgets::Paragraph::new(self.config.highlight_symbol.clone())
                    .style(self.config.highlight_style);
                frame.render_widget(p, prefix);
            }

            let final_style = if is_selected {
                item.style.patch(self.config.highlight_style)
            } else {
                item.style
            };

            if let Some(prefix_lines) = &item.prefix_lines {
                let max_width = prefix_lines.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
                if max_width > 0 {
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Length(max_width), Constraint::Min(0)])
                        .split(text_area);

                    let p_area = chunks[0];
                    text_area = chunks[1];

                    let paragraph = Paragraph::new(prefix_lines.clone()).style(final_style);
                    frame.render_widget(paragraph, p_area);
                }
            }

            if let Some(cover_url) = &item.cover_url {
                if let Some(picker) = ImageCache::global_picker() {
                    if let Some(img) = ImageCache::global().get_or_fetch(cover_url) {
                        let mut img_w = item_height * 2;
                        img_w = img_w.min(text_area.width / 4).max(2);

                        let img_rect = Rect {
                            x: text_area.x,
                            y: text_area.y,
                            width: img_w,
                            height: item_height,
                        };

                        let proto = self
                            .image_protocols
                            .entry(cover_url.clone())
                            .or_insert_with(|| picker.new_resize_protocol((*img).clone()));

                        frame.render_stateful_widget(StatefulImage::new(), img_rect, proto);

                        let text_x = text_area.x + img_w + 1;
                        let text_w = text_area.width.saturating_sub(img_w + 1);
                        text_area = Rect {
                            x: text_x,
                            y: text_area.y,
                            width: text_w,
                            height: item_height,
                        };
                    }
                }
            }

            let paragraph = Paragraph::new(item.content).style(final_style);
            frame.render_widget(paragraph, text_area);

            current_y += item_height;
            if i + 1 < list_items_len {
                current_y += 1;
            }
        }

        if self.config.show_scrollbar && total > inner_height as usize {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(None)
                .thumb_symbol("┃")
                .thumb_style(Style::default().fg(colors.accent.primary));
            let mut scrollbar_state = ScrollbarState::new(total).position(selected);

            let scrollbar_area = Rect {
                x: list_area.x + list_area.width - 1,
                y: list_area.y,
                width: 1,
                height: list_area.height,
            };

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }
}
