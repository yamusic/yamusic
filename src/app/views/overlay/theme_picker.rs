use std::sync::Arc;

use opaline::{
    ThemeInfo, ThemeVariant, current, list_available_themes, names::tokens as otokens, set_theme,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{
    actions::Action,
    keymap::Key,
    theme::{self, theme},
};
use crate::framework::reactive::{Memo, Signal, memo, signal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePickerKeyResult {
    Redraw,
    Select,
    Cancel,
    Noop,
}

pub struct ThemePicker {
    themes: Arc<Vec<ThemeInfo>>,
    cache: Arc<Vec<opaline::Theme>>,
    filtered: Memo<Vec<usize>>,
    filter: Signal<String>,
    cursor: Signal<usize>,
    scroll: Signal<usize>,
    light_mode: Signal<bool>,
    last_dark_theme: Signal<Option<String>>,
    last_light_theme: Signal<Option<String>>,
    original_theme: Signal<Arc<opaline::Theme>>,
}

impl ThemePicker {
    pub fn new() -> Self {
        let mut themes = list_available_themes();
        for embedded_id in [theme::DEFAULT_THEME_DARK, theme::DEFAULT_THEME_LIGHT] {
            if themes.iter().all(|info| info.name != embedded_id)
                && let Some(info) = theme::embedded_theme_info(embedded_id)
            {
                themes.push(info);
            }
        }

        themes.sort_by(|a, b| {
            let variant_ord = match (&a.variant, &b.variant) {
                (ThemeVariant::Dark, ThemeVariant::Light) => std::cmp::Ordering::Less,
                (ThemeVariant::Light, ThemeVariant::Dark) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            variant_ord.then_with(|| a.display_name.cmp(&b.display_name))
        });

        let themes = Arc::new(themes);

        let cache = themes
            .iter()
            .map(|info| {
                theme::embedded_theme_by_id(&info.name)
                    .or_else(|| info.load())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        let cache = Arc::new(cache);

        let filter = signal(String::new());
        let show_light = signal(false);
        let builtin_only = signal(false);

        let filtered = memo({
            let themes = themes.clone();
            let filter = filter.clone();
            let show_light = show_light.clone();
            let builtin_only = builtin_only.clone();
            move |_| {
                let q = filter.get().to_lowercase();
                let want_light = show_light.get();
                let only_builtin = builtin_only.get();

                themes
                    .iter()
                    .enumerate()
                    .filter(|(_, info)| {
                        let variant_ok = if want_light {
                            matches!(info.variant, ThemeVariant::Light)
                        } else {
                            matches!(info.variant, ThemeVariant::Dark)
                        };

                        variant_ok
                            && (!only_builtin || info.builtin)
                            && (q.is_empty()
                                || info.display_name.to_lowercase().contains(&q)
                                || info.author.to_lowercase().contains(&q)
                                || info.name.to_lowercase().contains(&q))
                    })
                    .map(|(idx, _)| idx)
                    .collect::<Vec<_>>()
            }
        });

        Self {
            themes,
            cache,
            filtered,
            filter,
            cursor: signal(0usize),
            scroll: signal(0usize),
            light_mode: show_light,
            last_dark_theme: signal(None),
            last_light_theme: signal(None),
            original_theme: signal(current()),
        }
    }

    pub fn handle_key(&self, key: &Key) -> ThemePickerKeyResult {
        match key {
            Key::Esc => {
                set_theme((*self.original_theme.get()).clone());
                theme::refresh();
                ThemePickerKeyResult::Cancel
            }
            Key::Enter => ThemePickerKeyResult::Select,
            Key::Tab | Key::BackTab => {
                self.store_theme();
                self.light_mode.update(|v| *v = !*v);
                self.cursor.set(0);
                self.scroll.set(0);
                self.align_cursor_by_saved();
                self.apply_preview();
                ThemePickerKeyResult::Redraw
            }
            Key::Up => {
                let cursor = self.cursor.get();
                if cursor > 0 {
                    self.cursor.set(cursor - 1);
                    self.apply_preview();
                    ThemePickerKeyResult::Redraw
                } else {
                    ThemePickerKeyResult::Noop
                }
            }
            Key::Down => {
                let cursor = self.cursor.get();
                let filtered = self.filtered.get();
                if cursor + 1 < filtered.len() {
                    self.cursor.set(cursor + 1);
                    self.apply_preview();
                    ThemePickerKeyResult::Redraw
                } else {
                    ThemePickerKeyResult::Noop
                }
            }
            Key::PageUp => {
                self.cursor.update(|c| *c = c.saturating_sub(10));
                self.apply_preview();
                ThemePickerKeyResult::Redraw
            }
            Key::PageDown => {
                let filtered = self.filtered.get();
                if !filtered.is_empty() {
                    let next = (self.cursor.get() + 10).min(filtered.len() - 1);
                    self.cursor.set(next);
                    self.apply_preview();
                    ThemePickerKeyResult::Redraw
                } else {
                    ThemePickerKeyResult::Noop
                }
            }
            Key::Home => {
                self.cursor.set(0);
                self.apply_preview();
                ThemePickerKeyResult::Redraw
            }
            Key::End => {
                let filtered = self.filtered.get();
                if !filtered.is_empty() {
                    self.cursor.set(filtered.len() - 1);
                    self.apply_preview();
                    ThemePickerKeyResult::Redraw
                } else {
                    ThemePickerKeyResult::Noop
                }
            }
            Key::Backspace => {
                let mut changed = false;
                self.filter.update(|f| {
                    changed = f.pop().is_some();
                });
                if changed {
                    self.cursor.set(0);
                    self.scroll.set(0);
                    self.apply_preview();
                    ThemePickerKeyResult::Redraw
                } else {
                    ThemePickerKeyResult::Noop
                }
            }
            Key::Char(c)
                if c.is_ascii_alphanumeric()
                    || c.is_ascii_punctuation()
                    || c.is_ascii_whitespace() =>
            {
                self.filter.update(|f| f.push(*c));
                self.cursor.set(0);
                self.scroll.set(0);
                self.apply_preview();
                ThemePickerKeyResult::Redraw
            }
            _ => ThemePickerKeyResult::Noop,
        }
    }

    pub fn handle_key_action(&self, key: &Key) -> Option<Action> {
        match self.handle_key(key) {
            ThemePickerKeyResult::Select => {
                if let Some(id) = self.selected_theme_id() {
                    let _ = theme::load(&id);
                }
                Some(Action::DismissOverlay)
            }
            ThemePickerKeyResult::Cancel => Some(Action::DismissOverlay),
            ThemePickerKeyResult::Redraw => Some(Action::Redraw),
            ThemePickerKeyResult::Noop => None,
        }
    }

    pub fn view(&self, frame: &mut Frame, area: Rect) {
        let t = theme();
        let base = Style::default().fg(t.text.primary).bg(t.bg.base);

        frame.render_widget(Clear, area);
        frame.buffer_mut().set_style(area, base);

        let container = centered_rect(area, 36, 56);
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_style(t.focused_border)
            .border_set(border::ROUNDED)
            .style(base);
        let outer_inner = outer.inner(container);
        frame.render_widget(outer, container);

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(outer_inner);

        let search_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.unfocused_border)
            .border_set(border::ROUNDED)
            .style(Style::default().bg(t.bg.base));
        let search_inner = search_block.inner(sections[0]);
        frame.render_widget(search_block, sections[0]);

        let search_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(20)])
            .split(search_inner);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("", Style::default().fg(t.text.secondary)),
                Span::raw(" "),
                Span::styled(self.filter.get(), Style::default().fg(t.text.primary)),
                Span::raw("  "),
            ]))
            .style(Style::default().bg(t.bg.base)),
            search_cols[0],
        );

        frame.render_widget(
            Paragraph::new(self.light_mode.get().then(|| "").unwrap_or(""))
                .style(Style::default().fg(t.accent.primary).bg(t.bg.base))
                .alignment(Alignment::Right),
            Rect::new(
                search_cols[1].x,
                search_cols[1].y,
                search_cols[1].width.saturating_sub(1),
                search_cols[1].height,
            ),
        );

        render_theme_list(frame, sections[1], self);
    }

    pub fn selected_theme_id(&self) -> Option<String> {
        self.selected_theme_index()
            .map(|idx| self.themes[idx].name.clone())
    }

    fn apply_preview(&self) {
        if let Some(idx) = self.selected_theme_index()
            && let Some(t) = self.cache.get(idx)
        {
            self.store_theme_name(self.themes[idx].name.clone());
            set_theme(t.clone());
            theme::refresh();
        }
    }

    fn store_theme(&self) {
        if let Some(idx) = self.selected_theme_index() {
            self.store_theme_name(self.themes[idx].name.clone());
        }
    }

    fn store_theme_name(&self, name: String) {
        if self.light_mode.get() {
            self.last_light_theme.set(Some(name));
        } else {
            self.last_dark_theme.set(Some(name));
        }
    }

    fn align_cursor_by_saved(&self) {
        let saved_name = if self.light_mode.get() {
            self.last_light_theme.get()
        } else {
            self.last_dark_theme.get()
        };

        if let Some(saved_name) = saved_name
            && self.align_cursor_by_name(&saved_name)
        {
            return;
        }

        self.align_cursor();
    }

    fn selected_theme_index(&self) -> Option<usize> {
        let filtered = self.filtered.get();
        if filtered.is_empty() {
            return None;
        }

        let cursor = self.cursor.get().min(filtered.len() - 1);
        filtered.get(cursor).copied()
    }

    fn align_cursor(&self) {
        let current_name = self.original_theme.get().meta.name.clone();
        let _ = self.align_cursor_by_name(&current_name);
    }

    fn align_cursor_by_name(&self, theme_name: &str) -> bool {
        let filtered = self.filtered.get();

        if let Some((filtered_idx, _)) = filtered.iter().enumerate().find(|(_, idx)| {
            let info = &self.themes[**idx];
            info.display_name == theme_name || info.name == theme_name
        }) {
            self.cursor.set(filtered_idx);
            self.scroll.set(filtered_idx.saturating_sub(6));
            return true;
        }

        false
    }

    fn clamp_cursor(&self) {
        let filtered = self.filtered.get();
        if filtered.is_empty() {
            self.cursor.set(0);
            self.scroll.set(0);
            return;
        }

        let max = filtered.len() - 1;
        if self.cursor.get() > max {
            self.cursor.set(max);
        }
    }

    fn clamp_scroll(&self, viewport_rows: usize) {
        if viewport_rows == 0 {
            return;
        }

        self.clamp_cursor();

        let cursor = self.cursor.get();
        let mut scroll = self.scroll.get();

        if cursor < scroll {
            scroll = cursor;
        }
        if cursor >= scroll + viewport_rows {
            scroll = cursor + 1 - viewport_rows;
        }

        self.scroll.set(scroll);
    }
}

impl Default for ThemePicker {
    fn default() -> Self {
        Self::new()
    }
}

fn render_theme_list(frame: &mut Frame, area: Rect, state: &ThemePicker) {
    let t = theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(t.unfocused_border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = inner.height as usize;
    if rows == 0 {
        return;
    }
    let visible_tiles = (rows / 3).max(1);
    state.clamp_scroll(visible_tiles);

    let filtered = state.filtered.get();
    if filtered.is_empty() {
        frame.render_widget(Paragraph::new(" No themes match ").style(t.muted), inner);
        return;
    }

    let scroll = state.scroll.get();
    let cursor = state.cursor.get();

    for (tile_idx, filtered_idx) in filtered.iter().skip(scroll).take(visible_tiles).enumerate() {
        let info = &state.themes[*filtered_idx];
        let th = &state.cache[*filtered_idx];
        let is_selected = scroll + tile_idx == cursor;

        let fg = Color::from(th.color(otokens::BG_SELECTION));
        let bg = Color::from(th.color(otokens::BG_BASE));
        let panel = Color::from(th.color(otokens::BG_PANEL));
        let bg_selection = Color::from(th.color(otokens::BG_SELECTION));
        let accent_primary = Color::from(th.color(otokens::ACCENT_PRIMARY));
        let accent_secondary = Color::from(th.color(otokens::ACCENT_SECONDARY));
        let accent_tertiary = Color::from(th.color(otokens::ACCENT_TERTIARY));
        let accent_deep = Color::from(th.color(otokens::ACCENT_DEEP));
        let text_dim = Color::from(th.color(otokens::TEXT_DIM));

        let y = inner.y + (tile_idx as u16 * 3);
        if y >= inner.y + inner.height {
            break;
        }

        let tile = Rect::new(inner.x, y, inner.width, 3);
        let tile_style = Style::default().bg(bg).fg(fg);
        frame.buffer_mut().set_style(tile, tile_style);

        if is_selected {
            frame.render_widget(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(border::PLAIN)
                    .border_style(Style::default().fg(theme().accent.primary).bg(bg)),
                tile,
            );
        }

        let content_row = if is_selected {
            Rect::new(
                tile.x.saturating_add(1),
                tile.y + 1,
                tile.width.saturating_sub(2),
                1,
            )
        } else {
            Rect::new(tile.x, tile.y + 1, tile.width, 1)
        };

        if content_row.width == 0 {
            continue;
        }

        let chips_width = ((content_row.width as usize * 2) / 5).clamp(16, 49) as u16;

        let tile_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(chips_width)])
            .split(content_row);

        let mut name_style = Style::default().bg(bg).fg(theme().text.primary);
        if is_selected {
            name_style = name_style
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::ITALIC);
        }

        let spans = vec![
            Span::raw(if is_selected { " " } else { "  " }),
            Span::styled(info.display_name.clone(), name_style),
        ];

        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(tile_style),
            tile_cols[0],
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                color_icon(panel),
                Span::raw(" "),
                color_icon(bg_selection),
                Span::raw(" "),
                color_icon(Color::from(th.color(otokens::BORDER_UNFOCUSED))),
                Span::raw(" "),
                color_icon(text_dim),
                Span::raw(" "),
                color_icon(accent_deep),
                Span::raw(" "),
                color_icon(Color::from(th.color(otokens::CODE_KEYWORD))),
                Span::raw(" "),
                color_icon(accent_primary),
                Span::raw(" "),
                color_icon(accent_tertiary),
                Span::raw(" "),
                color_icon(accent_secondary),
            ]))
            .style(tile_style)
            .alignment(Alignment::Right),
            Rect::new(
                tile_cols[1].x,
                tile_cols[1].y,
                tile_cols[1]
                    .width
                    .saturating_sub(if is_selected { 1 } else { 2 }),
                tile_cols[1].height,
            ),
        );
    }
}

fn color_icon(color: Color) -> Span<'static> {
    Span::styled("●", Style::default().fg(color))
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
