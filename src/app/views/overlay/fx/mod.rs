pub mod bandpass;
pub mod base;
pub mod chorus;
pub mod compressor;
pub mod dc_block;
pub mod delay;
pub mod eq;
pub mod highpass;
pub mod lowpass;
pub mod notch;
pub mod overdrive;
pub mod reverb;
mod util;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap},
};

use crate::app::actions::Action;
use crate::app::components::widgets::Slider;
use crate::app::keymap::Key;
use crate::app::views::overlay::fx::base::EffectMeta;
use crate::audio::fx::EffectHandle;
use crate::framework::theme::ThemeStyles;

use crate::app::views::overlay::fx::bandpass::BandpassRenderer;
use crate::app::views::overlay::fx::chorus::ChorusRenderer;
use crate::app::views::overlay::fx::compressor::CompressorRenderer;
use crate::app::views::overlay::fx::dc_block::DcBlockRenderer;
use crate::app::views::overlay::fx::delay::DelayRenderer;
use crate::app::views::overlay::fx::eq::EqRenderer;
use crate::app::views::overlay::fx::highpass::HighpassRenderer;
use crate::app::views::overlay::fx::lowpass::LowpassRenderer;
use crate::app::views::overlay::fx::notch::NotchRenderer;
use crate::app::views::overlay::fx::overdrive::OverdriveRenderer;
use crate::app::views::overlay::fx::reverb::ReverbRenderer;

const OVERLAY_WIDTH_PERCENT: u16 = 90;
const OVERLAY_HEIGHT_PERCENT: u16 = 90;
const SIDEBAR_MIN_WIDTH: u16 = 20;
const SIDEBAR_FALLBACK_WIDTH: u16 = 28;
const SIDEBAR_MAX_WIDTH: u16 = 35;
const HEADER_HEIGHT: u16 = 3;
const FOOTER_HEIGHT: u16 = 1;

const FILTER_PANEL_HEIGHT: u16 = 10;
const FX_PANEL_HEIGHT: u16 = 11;
const DELAY_PANEL_HEIGHT: u16 = 7;
const REVERB_PANEL_HEIGHT: u16 = 33;

const EQ_TOTAL_BANDS: usize = 15;

enum EffectRenderer {
    Eq(EqRenderer),
    Reverb(ReverbRenderer),
    Delay(DelayRenderer),
    Chorus(ChorusRenderer),
    Compressor(CompressorRenderer),
    Overdrive(OverdriveRenderer),
    DcBlock(DcBlockRenderer),
    Lowpass(LowpassRenderer),
    Highpass(HighpassRenderer),
    Bandpass(BandpassRenderer),
    Notch(NotchRenderer),
}

impl EffectRenderer {
    fn meta(&self) -> &EffectMeta {
        match self {
            Self::Eq(r) => r.meta(),
            Self::Reverb(r) => r.meta(),
            Self::Delay(r) => r.meta(),
            Self::Chorus(r) => r.meta(),
            Self::Compressor(r) => r.meta(),
            Self::Overdrive(r) => r.meta(),
            Self::DcBlock(r) => r.meta(),
            Self::Lowpass(r) => r.meta(),
            Self::Highpass(r) => r.meta(),
            Self::Bandpass(r) => r.meta(),
            Self::Notch(r) => r.meta(),
        }
    }
}

pub struct EffectsOverlay {
    renderers: Vec<EffectRenderer>,
    effect_handles: Arc<RwLock<HashMap<String, EffectHandle>>>,
    selected_effect: usize,
    display_order: Vec<usize>,
    settings_cursor: usize,
    eq_band_cursor: usize,
    param_values: HashMap<String, Vec<f32>>,
}

impl EffectsOverlay {
    pub fn new(effect_handles: Arc<RwLock<HashMap<String, EffectHandle>>>) -> Self {
        let mut renderers = vec![
            EffectRenderer::Eq(EqRenderer::new()),
            EffectRenderer::Reverb(ReverbRenderer::new()),
            EffectRenderer::Delay(DelayRenderer::new()),
            EffectRenderer::Chorus(ChorusRenderer::new()),
            EffectRenderer::Compressor(CompressorRenderer::new()),
            EffectRenderer::Overdrive(OverdriveRenderer::new()),
            EffectRenderer::DcBlock(DcBlockRenderer::new()),
            EffectRenderer::Lowpass(LowpassRenderer::new()),
            EffectRenderer::Highpass(HighpassRenderer::new()),
            EffectRenderer::Bandpass(BandpassRenderer::new()),
            EffectRenderer::Notch(NotchRenderer::new()),
        ];

        renderers.sort_by(|a, b| a.meta().name.cmp(b.meta().name));

        let mut param_values = HashMap::new();
        for renderer in &renderers {
            let meta = renderer.meta();
            let defaults: Vec<f32> = meta.params.iter().map(|p| p.default).collect();
            param_values.insert(meta.id.to_string(), defaults);
        }

        use std::collections::BTreeMap;
        let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, renderer) in renderers.iter().enumerate() {
            let category_key = renderer.meta().category.label().to_string();
            grouped.entry(category_key).or_default().push(idx);
        }
        let display_order: Vec<usize> = grouped.values().flatten().copied().collect();

        Self {
            renderers,
            effect_handles,
            selected_effect: 0,
            display_order,
            settings_cursor: 0,
            eq_band_cursor: 0,
            param_values,
        }
    }

    fn current_renderer(&self) -> Option<&EffectRenderer> {
        self.renderers.get(self.selected_effect)
    }

    fn current_meta(&self) -> Option<&EffectMeta> {
        self.current_renderer().map(|r| r.meta())
    }

    fn is_enabled(&self, id: &str) -> bool {
        if let Ok(guard) = self.effect_handles.read() {
            guard.get(id).map(|h| h.is_enabled()).unwrap_or(false)
        } else {
            false
        }
    }

    fn toggle(&self, id: &str) {
        if let Ok(guard) = self.effect_handles.read()
            && let Some(handle) = guard.get(id)
        {
            let was_enabled = handle.is_enabled();
            handle.set_enabled(!was_enabled);

            if !was_enabled {
                self.send_current_settings(id);
            }
        }
    }

    fn send_current_settings(&self, effect_id: &str) {
        let vals = match self.param_values.get(effect_id) {
            Some(v) => v,
            None => return,
        };

        if let Ok(guard) = self.effect_handles.read()
            && let Some(handle) = guard.get(effect_id)
        {
            for (i, val) in vals.iter().enumerate() {
                handle.set_param(i, *val);
            }
        }
    }

    fn select_next_effect(&mut self) {
        if self.display_order.is_empty() {
            return;
        }
        let current_pos = self
            .display_order
            .iter()
            .position(|&idx| idx == self.selected_effect)
            .unwrap_or(0);
        let next_pos = (current_pos + 1) % self.display_order.len();
        self.selected_effect = self.display_order[next_pos];
        self.settings_cursor = 0;
        self.eq_band_cursor = 0;
    }

    fn select_prev_effect(&mut self) {
        if self.display_order.is_empty() {
            return;
        }
        let current_pos = self
            .display_order
            .iter()
            .position(|&idx| idx == self.selected_effect)
            .unwrap_or(0);
        let prev_pos = if current_pos == 0 {
            self.display_order.len() - 1
        } else {
            current_pos - 1
        };
        self.selected_effect = self.display_order[prev_pos];
        self.settings_cursor = 0;
        self.eq_band_cursor = 0;
    }

    pub fn handle_key(&mut self, key: &Key) -> Action {
        let effect_id = match self.current_meta() {
            Some(e) => e.id,
            None => return Action::None,
        };

        if matches!(key, Key::Esc) {
            return Action::DismissOverlay;
        }
        if matches!(key, Key::Tab) {
            self.select_next_effect();
            return Action::Redraw;
        }
        if matches!(key, Key::BackTab) {
            self.select_prev_effect();
            return Action::Redraw;
        }
        if matches!(key, Key::Char(' ')) {
            self.toggle(effect_id);
            return Action::Redraw;
        }

        if effect_id == "eq" {
            return self.handle_eq_key(key);
        }

        let (total_items, step) = match self.current_meta() {
            Some(meta) => {
                let total = meta.params.len();
                let step = if self.settings_cursor < meta.params.len() {
                    meta.params[self.settings_cursor].step
                } else {
                    1.0
                };
                (total, step)
            }
            None => return Action::None,
        };

        match key {
            Key::Down | Key::Char('j') => {
                if total_items > 0 && self.settings_cursor + 1 < total_items {
                    self.settings_cursor += 1;
                }
                Action::Redraw
            }
            Key::Up | Key::Char('k') => {
                if self.settings_cursor > 0 {
                    self.settings_cursor -= 1;
                }
                Action::Redraw
            }
            Key::Right | Key::Char('l') => {
                self.adjust_param_by_cursor(effect_id, step);
                Action::Redraw
            }
            Key::Left | Key::Char('h') => {
                self.adjust_param_by_cursor(effect_id, -step);
                Action::Redraw
            }
            Key::Char('L') => {
                self.adjust_param_by_cursor(effect_id, step * 10.0);
                Action::Redraw
            }
            Key::Char('H') => {
                self.adjust_param_by_cursor(effect_id, -step * 10.0);
                Action::Redraw
            }
            Key::Char('r') => {
                self.reset_param(effect_id);
                Action::Redraw
            }
            Key::Char('R') => {
                self.reset_all_params(effect_id);
                Action::Redraw
            }
            _ => Action::None,
        }
    }

    fn reset_all_params(&mut self, effect_id: &str) {
        let meta = match self.renderers.iter().find(|r| r.meta().id == effect_id) {
            Some(r) => r.meta(),
            None => return,
        };

        let defaults: Vec<f32> = meta.params.iter().map(|p| p.default).collect();
        self.param_values.insert(effect_id.to_string(), defaults);
        self.send_current_settings(effect_id);
    }

    fn reset_param(&mut self, effect_id: &str) {
        let meta = match self.renderers.iter().find(|r| r.meta().id == effect_id) {
            Some(r) => r.meta(),
            None => return,
        };

        if self.settings_cursor >= meta.params.len() {
            return;
        }

        let default = meta.params[self.settings_cursor].default;
        if let Some(vals) = self.param_values.get_mut(effect_id) {
            if self.settings_cursor < vals.len() {
                vals[self.settings_cursor] = default;
                self.send_current_settings(effect_id);
            }
        }
    }

    fn adjust_param_by_cursor(&mut self, effect_id: &str, delta: f32) {
        self.adjust_param(effect_id, self.settings_cursor, delta);
    }

    fn adjust_param(&mut self, effect_id: &str, index: usize, delta: f32) {
        let (params_len, param_meta) = match self.current_meta() {
            Some(meta) => {
                if index >= meta.params.len() {
                    return;
                }
                (meta.params.len(), meta.params[index].clone())
            }
            None => return,
        };

        let vals = self.param_values.entry(effect_id.to_string()).or_default();
        if vals.len() < params_len {
            vals.resize(params_len, param_meta.default);
        }

        let new_val = vals[index] + delta;
        let snapped = param_meta.min
            + ((new_val - param_meta.min) / param_meta.step).round() * param_meta.step;
        vals[index] = snapped.clamp(param_meta.min, param_meta.max);
        self.send_current_settings(effect_id);
    }

    fn handle_eq_key(&mut self, key: &Key) -> Action {
        let effect_id = "eq";
        match key {
            Key::Left | Key::Char('h') => {
                self.eq_band_cursor = self.eq_band_cursor.saturating_sub(1);
                Action::Redraw
            }
            Key::Right | Key::Char('l') => {
                self.eq_band_cursor = (self.eq_band_cursor + 1).min(EQ_TOTAL_BANDS - 1);
                Action::Redraw
            }
            Key::Up | Key::Char('k') => {
                self.adjust_param(effect_id, self.eq_band_cursor, 1.0);
                Action::Redraw
            }
            Key::Down | Key::Char('j') => {
                self.adjust_param(effect_id, self.eq_band_cursor, -1.0);
                Action::Redraw
            }
            Key::Char('r') => {
                self.reset_param(effect_id);
                Action::Redraw
            }
            Key::Char('R') => {
                self.reset_all_params(effect_id);
                Action::Redraw
            }
            _ => Action::None,
        }
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect, styles: &ThemeStyles) {
        let overlay_area = centered_rect(area, OVERLAY_WIDTH_PERCENT, OVERLAY_HEIGHT_PERCENT);
        frame.render_widget(Clear, overlay_area);

        let accent = styles.accent.fg.unwrap_or(Color::Yellow);
        let muted = styles.text_muted.fg.unwrap_or(Color::DarkGray);
        let text = Color::White;
        let bg = styles.text.bg.unwrap_or(Color::Reset);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 󱕂 Effects ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(accent))
            .style(Style::default().fg(text).bg(bg))
            .padding(Padding::new(1, 1, 0, 0));

        let inner = block.inner(overlay_area);
        frame.render_widget(block, overlay_area);

        let sidebar_width = self
            .renderers
            .iter()
            .map(|r| (r.meta().name.len() + 6).max(SIDEBAR_MIN_WIDTH as usize) as u16)
            .max()
            .unwrap_or(SIDEBAR_FALLBACK_WIDTH)
            .min(SIDEBAR_MAX_WIDTH);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_width), Constraint::Min(0)])
            .split(inner);

        self.render_sidebar(frame, chunks[0], accent, muted, text);
        self.render_settings(frame, chunks[1], accent, muted, text);
    }

    fn render_sidebar(
        &self,
        frame: &mut Frame,
        area: Rect,
        accent: Color,
        muted: Color,
        text: Color,
    ) {
        let block = Block::default()
            .borders(Borders::RIGHT)
            // .title(" search (todo) ")
            // .title_alignment(Alignment::Center)
            // .title_style(Style::default().fg(accent))
            .border_style(Style::default().fg(muted))
            .padding(Padding::new(1, 1, 0, 0));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines = Vec::new();

        use std::collections::BTreeMap;
        let mut grouped: BTreeMap<String, Vec<(usize, &EffectRenderer)>> = BTreeMap::new();
        for (idx, renderer) in self.renderers.iter().enumerate() {
            let category_key = renderer.meta().category.label().to_string();
            grouped
                .entry(category_key)
                .or_default()
                .push((idx, renderer));
        }

        let selected_category = self.current_meta().map(|m| m.category.label().to_string());

        let mut first_category = true;
        for (category_name, effects) in grouped.iter() {
            if !first_category {
                lines.push(Line::from(""));
            }
            first_category = false;

            let is_active_category = selected_category.as_ref() == Some(category_name);

            let category_color = effects[0].1.meta().category.color();
            let category_icon = effects[0].1.meta().category.icon();
            let tree_indicator = if is_active_category { "" } else { "" };

            lines.push(Line::from(vec![
                Span::styled(
                    tree_indicator,
                    Style::default().fg(if is_active_category { accent } else { muted }),
                ),
                Span::raw(" "),
                Span::styled(
                    category_icon,
                    Style::default()
                        .fg(category_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    category_name,
                    Style::default()
                        .fg(category_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));

            let effects_len = effects.len();
            for (effect_idx, (idx, renderer)) in effects.iter().enumerate() {
                let effect = renderer.meta();
                let selected = *idx == self.selected_effect;
                let enabled = self.is_enabled(effect.id);
                let is_last = effect_idx == effects_len - 1;

                let branch = if is_last { "└" } else { "├" };
                let status = if enabled { "" } else { "" };

                let style = if selected {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else if enabled {
                    Style::default().fg(text)
                } else {
                    Style::default().fg(muted)
                };

                let status_style = if enabled {
                    Style::default().fg(category_color)
                } else {
                    Style::default().fg(muted)
                };

                let branch_style = if is_active_category {
                    Style::default().fg(accent)
                } else {
                    Style::default().fg(muted)
                };

                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(branch, branch_style),
                    Span::styled("─", branch_style),
                    Span::raw(" "),
                    Span::styled(status, status_style),
                    Span::raw("  "),
                    Span::styled(effect.name, style),
                ]));
            }
        }

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    fn render_settings(
        &self,
        frame: &mut Frame,
        area: Rect,
        accent: Color,
        muted: Color,
        text: Color,
    ) {
        let effect = match self.current_meta() {
            Some(e) => e,
            None => return,
        };

        let cat_color = effect.category.color();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(HEADER_HEIGHT),
                Constraint::Min(0),
                Constraint::Length(FOOTER_HEIGHT),
            ])
            .margin(1)
            .split(area);

        let enabled = self.is_enabled(effect.id);
        let status_text = if enabled { " ACTIVE " } else { " BYPASSED " };
        let status_style = if enabled {
            Style::default().fg(Color::Black).bg(cat_color)
        } else {
            Style::default().fg(muted).bg(Color::Reset)
        };

        let header = vec![
            Line::from(vec![
                Span::styled(
                    format!("{}", effect.category.icon()),
                    Style::default().fg(cat_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    effect.name,
                    Style::default().fg(text).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    format!(" {} ", effect.category.label()),
                    Style::default().fg(Color::Black).bg(cat_color),
                ),
                Span::raw("  "),
                Span::styled(status_text, status_style),
            ]),
            Line::from(Span::styled(effect.description, Style::default().fg(muted))),
        ];
        frame.render_widget(Paragraph::new(header), chunks[0]);

        let vals = self
            .param_values
            .get(effect.id)
            .map_or(&[][..], Vec::as_slice);

        match effect.id {
            "eq" => {
                if let Some(EffectRenderer::Eq(renderer)) = self.current_renderer() {
                    renderer.render(
                        frame,
                        chunks[1],
                        vals,
                        self.eq_band_cursor,
                        accent,
                        muted,
                        text,
                    );
                }
            }
            _ => self.render_standard_effect(
                frame, chunks[1], effect, vals, enabled, accent, muted, text,
            ),
        }

        let hint = if effect.id == "eq" {
            "tab/shift+tab: cycle  ←→: band  ↑↓: gain  r: reset band  R: reset all  space: toggle"
        } else {
            "tab/shift+tab: cycle  ↑↓: select  ←→: adjust  r: reset param  R: reset all  space: toggle"
        };

        frame.render_widget(
            Paragraph::new(hint)
                .alignment(Alignment::Center)
                .style(Style::default().fg(muted)),
            chunks[2],
        );
    }

    fn visual_height(effect_id: &str) -> Option<u16> {
        match effect_id {
            "reverb" => Some(REVERB_PANEL_HEIGHT),
            "delay" => Some(DELAY_PANEL_HEIGHT),
            "chorus" | "compressor" | "overdrive" | "dc_block" => Some(FX_PANEL_HEIGHT),
            "lowpass" | "highpass" | "bandpass" | "notch" => Some(FILTER_PANEL_HEIGHT),
            _ => None,
        }
    }

    fn split_visual_and_params(area: Rect, visual_height: u16) -> (Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(visual_height), Constraint::Min(0)])
            .split(area);
        (chunks[0], chunks[1])
    }

    #[allow(clippy::too_many_arguments)]
    fn render_standard_effect(
        &self,
        frame: &mut Frame,
        area: Rect,
        effect: &EffectMeta,
        vals: &[f32],
        enabled: bool,
        accent: Color,
        muted: Color,
        text: Color,
    ) {
        let Some(visual_height) = Self::visual_height(effect.id) else {
            self.render_params(frame, area, effect, accent, muted, text);
            return;
        };

        let (visual_area, params_area) = Self::split_visual_and_params(area, visual_height);

        if let Some(renderer) = self.current_renderer() {
            match renderer {
                EffectRenderer::Reverb(r) => r.render(frame, visual_area, vals, accent),
                EffectRenderer::Delay(r) => r.render(frame, visual_area, vals, accent, muted),
                EffectRenderer::Chorus(r) => r.render(frame, visual_area, vals, accent, muted),
                EffectRenderer::Compressor(r) => r.render(frame, visual_area, vals, accent, muted),
                EffectRenderer::Overdrive(r) => r.render(frame, visual_area, vals, accent, muted),
                EffectRenderer::DcBlock(r) => r.render(frame, visual_area, enabled, accent),
                EffectRenderer::Lowpass(r) => r.render(frame, visual_area, vals, accent),
                EffectRenderer::Highpass(r) => r.render(frame, visual_area, vals, accent),
                EffectRenderer::Bandpass(r) => r.render(frame, visual_area, vals, accent),
                EffectRenderer::Notch(r) => r.render(frame, visual_area, vals, accent),
                _ => {}
            }
        }

        self.render_params(frame, params_area, effect, accent, muted, text);
    }

    fn render_params(
        &self,
        frame: &mut Frame,
        area: Rect,
        effect: &EffectMeta,
        accent: Color,
        muted: Color,
        text: Color,
    ) {
        let vals = self
            .param_values
            .get(effect.id)
            .cloned()
            .unwrap_or_default();

        let mut constraints = Vec::new();
        for _ in 0..effect.params.len() {
            constraints.push(Constraint::Length(2));
        }
        constraints.push(Constraint::Min(0));

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        for (i, param) in effect.params.iter().enumerate() {
            Slider::new(param.name, vals.get(i).copied().unwrap_or(param.default))
                .range(param.min, param.max)
                .suffix(param.suffix)
                .focused(self.settings_cursor == i)
                .colors(accent, muted, text)
                .render(
                    frame,
                    Rect {
                        x: rows[i].x + 2,
                        y: rows[i].y,
                        width: rows[i].width.saturating_sub(4),
                        height: 1,
                    },
                );
        }
    }
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
