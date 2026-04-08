use image::DynamicImage;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols::{self, border},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratatui_image::{StatefulImage, protocol::StatefulProtocol};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    audio::enums::RepeatMode,
    cache::image::ImageCache,
    framework::{signals::Signal, theme::ThemeStyles},
    util::animation::Animation,
};

pub struct PlayerSignals {
    pub track_title: Signal<Option<String>>,
    pub track_artists: Signal<Option<String>>,
    pub is_playing: Signal<bool>,
    pub is_liked: Signal<bool>,
    pub is_disliked: Signal<bool>,
    pub position_ms: Signal<u64>,
    pub duration_ms: Signal<u64>,
    pub buffered_ratio: Signal<f32>,
    pub volume: Signal<u8>,
    pub is_muted: Signal<bool>,
    pub is_shuffled: Signal<bool>,
    pub repeat_mode: Signal<RepeatMode>,
    pub cover_url: Signal<Option<String>>,
}

impl PlayerSignals {
    pub fn new() -> Self {
        Self {
            track_title: Signal::new(None),
            track_artists: Signal::new(None),
            is_playing: Signal::new(false),
            is_liked: Signal::new(false),
            is_disliked: Signal::new(false),
            position_ms: Signal::new(0),
            duration_ms: Signal::new(0),
            buffered_ratio: Signal::new(0.0),
            volume: Signal::new(50),
            is_muted: Signal::new(false),
            is_shuffled: Signal::new(false),
            repeat_mode: Signal::new(RepeatMode::None),
            cover_url: Signal::new(None),
        }
    }
}

impl Default for PlayerSignals {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PlayerBar {
    signals: PlayerSignals,
    theme: Signal<ThemeStyles>,
    protocol: Option<StatefulProtocol>,
    last_art: Option<Arc<DynamicImage>>,
    last_volume: u8,
    last_muted: bool,
    last_volume_change_at: Option<Instant>,
    pub animation: PlayerBarAnimation,
}

pub struct PlayerBarAnimation {
    animated_played_ratio: f64,
    animated_buffered_ratio: f64,
    played_anim_from: f64,
    played_anim_to: f64,
    buffered_anim_from: f64,
    buffered_anim_to: f64,
    started_at: Option<Instant>,
    duration: Duration,
}

impl PlayerBarAnimation {
    pub fn new() -> Self {
        Self {
            animated_played_ratio: 0.0,
            animated_buffered_ratio: 0.0,
            played_anim_from: 0.0,
            played_anim_to: 0.0,
            buffered_anim_from: 0.0,
            buffered_anim_to: 0.0,
            started_at: None,
            duration: Duration::from_millis(180),
        }
    }

    fn update_state(&mut self, now: Instant) {
        let Some(started_at) = self.started_at else {
            return;
        };

        let duration = self.duration.as_secs_f64().max(f64::EPSILON);
        let elapsed = now.duration_since(started_at).as_secs_f64();
        let t = Animation::clamp01(elapsed / duration);
        let eased_t = Animation::ease_in_out_cubic(t);

        self.animated_played_ratio =
            Animation::lerp(self.played_anim_from, self.played_anim_to, eased_t);
        self.animated_buffered_ratio =
            Animation::lerp(self.buffered_anim_from, self.buffered_anim_to, eased_t);

        if t >= 1.0 {
            self.animated_played_ratio = self.played_anim_to;
            self.animated_buffered_ratio = self.buffered_anim_to;
            self.started_at = None;
        }
    }

    pub fn ratios(&mut self, played_target: f64, buffered_target: f64) -> (f64, f64) {
        let now = Instant::now();

        let played_target = Animation::clamp01(played_target);
        let buffered_target = Animation::clamp01(buffered_target).max(played_target);

        if self.started_at.is_none()
            && self.played_anim_to == 0.0
            && self.buffered_anim_to == 0.0
            && self.animated_played_ratio == 0.0
            && self.animated_buffered_ratio == 0.0
        {
            self.animated_played_ratio = played_target;
            self.animated_buffered_ratio = buffered_target;
            self.played_anim_from = played_target;
            self.played_anim_to = played_target;
            self.buffered_anim_from = buffered_target;
            self.buffered_anim_to = buffered_target;
            return (played_target, buffered_target);
        }

        self.update_state(now);

        let current_played = self.animated_played_ratio;
        let current_buffered = self.animated_buffered_ratio.max(current_played);

        let target_changed = (played_target - self.played_anim_to).abs() > 0.001
            || (buffered_target - self.buffered_anim_to).abs() > 0.001;

        if target_changed {
            let max_delta = (played_target - current_played)
                .abs()
                .max((buffered_target - current_buffered).abs());
            let duration_ms = if max_delta >= 0.45 {
                300
            } else if max_delta >= 0.2 {
                220
            } else {
                160
            };

            self.played_anim_from = current_played;
            self.played_anim_to = played_target;
            self.buffered_anim_from = current_buffered;
            self.buffered_anim_to = buffered_target;
            self.duration = Duration::from_millis(duration_ms);
            self.started_at = Some(now);

            self.update_state(now);
        }

        let played = Animation::clamp01(self.animated_played_ratio);
        let buffered = Animation::clamp01(self.animated_buffered_ratio.max(played));
        (played, buffered)
    }
}

impl PlayerBar {
    pub fn new(signals: PlayerSignals, theme: Signal<ThemeStyles>) -> Self {
        Self {
            signals,
            theme,
            protocol: None,
            last_art: None,
            last_volume: 0,
            last_muted: false,
            last_volume_change_at: None,
            animation: PlayerBarAnimation::new(),
        }
    }

    pub fn view(&mut self, frame: &mut Frame, area: Rect) {
        let styles = self.theme.get();

        let cache = ImageCache::global();
        let current_art = self
            .signals
            .cover_url
            .get()
            .and_then(|url| cache.get_or_fetch(&url));

        let art_changed = match (&self.last_art, &current_art) {
            (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };
        if art_changed {
            self.protocol = None;
            if let (Some(picker), Some(art)) = (ImageCache::global_picker(), &current_art) {
                self.protocol = Some(picker.new_resize_protocol((**art).clone()));
            }
            self.last_art = current_art;
        }

        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(styles.block_focused);
        let inner = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        if inner.height < 3 || inner.width < 12 {
            return;
        }

        let img_w = inner.height.saturating_mul(2).min(inner.width / 4);
        let text_x = inner.x + img_w + 1;
        let text_aw = inner.width.saturating_sub(img_w + 1);
        let left_w = (text_aw / 4).max(12).min(text_aw);

        let row0_y = inner.y;
        let row1_y = inner.y + 1;
        let row2_y = inner.y + inner.height - 1;
        let mut controls_center_x = inner.x + inner.width.saturating_sub(1) / 2;

        if img_w > 0 {
            if let Some(proto) = &mut self.protocol {
                frame.render_stateful_widget(
                    StatefulImage::new(),
                    Rect {
                        x: inner.x,
                        y: inner.y,
                        width: img_w,
                        height: inner.height,
                    },
                    proto,
                );
            }
        }

        {
            let title = self
                .signals
                .track_title
                .get()
                .unwrap_or_else(|| "No track".into());

            frame.render_widget(
                Paragraph::new(Span::styled(
                    title,
                    styles.text.add_modifier(Modifier::BOLD),
                )),
                Rect {
                    x: text_x,
                    y: row0_y,
                    width: left_w,
                    height: 1,
                },
            );
        }

        {
            let artist = self.signals.track_artists.get().unwrap_or_default();
            frame.render_widget(
                Paragraph::new(Span::styled(artist, styles.text_muted)),
                Rect {
                    x: text_x,
                    y: row1_y,
                    width: left_w,
                    height: 1,
                },
            );

            let vol = self.signals.volume.get();
            let is_muted = self.signals.is_muted.get();
            let now = Instant::now();
            if vol != self.last_volume || is_muted != self.last_muted {
                self.last_volume = vol;
                self.last_muted = is_muted;
                self.last_volume_change_at = Some(now);
            }

            let show_vol_popup = self
                .last_volume_change_at
                .is_some_and(|t| now.duration_since(t) < Duration::from_millis(1200));

            let vol_compact_w: u16 = 6;
            let show_volume = text_aw > left_w + vol_compact_w + 8;
            let vol_w = if show_volume { vol_compact_w } else { 0 };
            let gap_w: u16 = if show_volume { 2 } else { 0 };
            let vol_x = text_x + text_aw.saturating_sub(vol_w + 2);

            if show_volume {
                let vol_icon = if is_muted || vol == 0 {
                    "󰝟"
                } else if vol < 25 {
                    "󰕿"
                } else if vol < 50 {
                    "󰖀"
                } else {
                    "󰕾"
                };

                let popup_outer_w: u16 = 5;
                let popup_x = (vol_x.saturating_sub(2) + vol_w / 2).saturating_add(1);
                let icon_x = popup_x + popup_outer_w / 2;

                frame.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(vol_icon, styles.text_muted)])),
                    Rect {
                        x: icon_x,
                        y: row1_y,
                        width: vol_w,
                        height: 1,
                    },
                );

                if show_vol_popup {
                    let popup_h: u16 = 6;
                    let popup_label_h: u16 = 1;
                    let border_pad: u16 = 1;

                    let total_outer_h = popup_label_h + popup_h + border_pad * 2;
                    let popup_bottom = area.y;
                    let popup_y = popup_bottom.saturating_sub(total_outer_h + 1);

                    let popup_block = Block::default()
                        .borders(Borders::ALL)
                        .border_set(border::ROUNDED)
                        .border_style(styles.text_muted);
                    let outer_rect = Rect {
                        x: popup_x,
                        y: popup_y,
                        width: popup_outer_w,
                        height: total_outer_h,
                    };
                    let inner_rect = popup_block.inner(outer_rect);
                    frame.render_widget(popup_block, outer_rect);

                    let inner_x = inner_rect.x;
                    let inner_y = inner_rect.y;
                    let inner_w = inner_rect.width;

                    let pct_str = if is_muted {
                        " 󰝟 ".to_string()
                    } else {
                        format!("{:>2}%", vol)
                    };
                    frame.render_widget(
                        Paragraph::new(Span::styled(pct_str, styles.text_muted)),
                        Rect {
                            x: inner_x,
                            y: inner_y,
                            width: inner_w,
                            height: 1,
                        },
                    );

                    let ratio = if is_muted {
                        0.0_f64
                    } else {
                        (vol as f64 / 100.0).clamp(0.0, 1.0)
                    };

                    let total_eighths = (ratio * popup_h as f64 * 8.0).round() as u32;
                    let full_rows = (total_eighths / 8) as u16;
                    let partial_eighths = (total_eighths % 8) as u8;

                    let partial_sym: Option<&str> = match partial_eighths {
                        0 => None,
                        1 => Some("▁"),
                        2 => Some("▂"),
                        3 => Some("▃"),
                        4 => Some("▄"),
                        5 => Some("▅"),
                        6 => Some("▆"),
                        7 => Some("▇"),
                        _ => Some("█"),
                    };

                    let has_partial = partial_sym.is_some();
                    let empty_rows = popup_h
                        .saturating_sub(full_rows)
                        .saturating_sub(if has_partial { 1 } else { 0 });

                    let empty_bg = styles.text_muted.bg.unwrap_or(Color::Reset);

                    let side_style = Style::default();
                    let empty_center_style = Style::default().bg(empty_bg);
                    let partial_style = Style::default()
                        .fg(styles.accent.fg.unwrap_or(Color::Reset))
                        .bg(empty_bg);

                    for row in 0..popup_h {
                        let pip_y = inner_y + popup_label_h + row;

                        if row < empty_rows {
                            frame.render_widget(
                                Paragraph::new(Line::from(vec![
                                    Span::styled(" ", side_style),
                                    Span::styled(" ", empty_center_style),
                                    Span::styled(" ", side_style),
                                ])),
                                Rect {
                                    x: inner_x,
                                    y: pip_y,
                                    width: inner_w,
                                    height: 1,
                                },
                            );
                        } else if row == empty_rows && has_partial {
                            let sub_char_line = Line::from(vec![
                                Span::styled(" ", side_style),
                                Span::styled(partial_sym.unwrap(), partial_style),
                                Span::styled(" ", side_style),
                            ]);

                            frame.render_widget(
                                Paragraph::new(sub_char_line),
                                Rect {
                                    x: inner_x,
                                    y: pip_y,
                                    width: inner_w,
                                    height: 1,
                                },
                            );
                        } else {
                            frame.render_widget(
                                Paragraph::new(Span::styled(" █ ", styles.accent)),
                                Rect {
                                    x: inner_x,
                                    y: pip_y,
                                    width: inner_w,
                                    height: 1,
                                },
                            );
                        }
                    }
                }
            }

            let is_playing = self.signals.is_playing.get();
            let is_liked = self.signals.is_liked.get();
            let is_disliked = self.signals.is_disliked.get();
            let shuffle = self.signals.is_shuffled.get();
            let repeat = self.signals.repeat_mode.get();
            let accent = styles.accent;
            let muted_sty = styles.text_muted;
            let normal_sty = styles.text;

            let sep = || Span::raw("  ");

            let like_span = if is_liked {
                Span::styled("󰋑", accent)
            } else {
                Span::styled("󰋕", muted_sty)
            };
            let dislike_span = if is_disliked {
                Span::styled("󰝙", accent)
            } else {
                Span::styled("󱐴", muted_sty)
            };
            let shuffle_span = if shuffle {
                Span::styled("󰒟", accent)
            } else {
                Span::styled("󰒞", muted_sty)
            };
            let prev_span = Span::styled("󰒮", normal_sty);
            let play_span = if is_playing {
                Span::styled("󰏤", normal_sty.add_modifier(Modifier::BOLD))
            } else {
                Span::styled("󰐊", normal_sty.add_modifier(Modifier::BOLD))
            };
            let next_span = Span::styled("󰒭", normal_sty);
            let repeat_span = match repeat {
                RepeatMode::None => Span::styled("󰑗", muted_sty),
                RepeatMode::Single => Span::styled("󰑘", accent),
                RepeatMode::All => Span::styled("󰑖", accent),
            };

            let controls = Line::from(vec![
                like_span,
                sep(),
                shuffle_span,
                sep(),
                prev_span,
                sep(),
                play_span,
                sep(),
                next_span,
                sep(),
                repeat_span,
                sep(),
                dislike_span,
            ]);
            let play_prefix_w: u16 = controls
                .spans
                .iter()
                .take(6)
                .map(|s| s.width() as u16)
                .sum();
            let play_icon_w = controls.spans.get(6).map(|s| s.width() as u16).unwrap_or(1);

            let controls_w = controls.width() as u16;
            if controls_w > 0 {
                let ideal_x = inner.x + inner.width.saturating_sub(controls_w) / 2;
                let min_x = text_x + left_w.saturating_add(1);
                let right_limit = if show_volume {
                    vol_x.saturating_sub(gap_w)
                } else {
                    text_x + text_aw
                };
                let max_x = right_limit.saturating_sub(controls_w);
                let controls_x = if max_x >= min_x {
                    ideal_x.clamp(min_x, max_x)
                } else {
                    min_x
                };
                controls_center_x = controls_x + play_prefix_w + (play_icon_w / 2);

                frame.render_widget(
                    Paragraph::new(controls),
                    Rect {
                        x: controls_x,
                        y: row1_y,
                        width: controls_w,
                        height: 1,
                    },
                );
            }
        }

        {
            let current = self.signals.position_ms.get();
            let total = self.signals.duration_ms.get();
            let played_target = if total > 0 {
                (current as f64 / total as f64).min(1.0)
            } else {
                0.0
            };
            let buffered_target = (self.signals.buffered_ratio.get() as f64).min(1.0);
            let (played, buffered) = self.animation.ratios(played_target, buffered_target);

            let current_label = format_duration(current);
            let total_label = format_duration(total);
            let current_w = current_label.len() as u16;
            let total_w = total_label.len() as u16;
            let gap_w: u16 = 1;
            let label_gap: u16 = 1;

            let target_gauge_w = (text_aw / 3).max(8);
            let side_need = current_w.max(total_w).saturating_add(label_gap);
            let max_centered_gauge_w = text_aw.saturating_sub(side_need.saturating_mul(2));
            let mut gauge_w = target_gauge_w.min(max_centered_gauge_w).max(1);
            if gauge_w % 2 == 0 && gauge_w < max_centered_gauge_w {
                gauge_w += 1;
            }
            if gauge_w % 2 == 0 && gauge_w > 1 {
                gauge_w -= 1;
            }

            let ideal_gauge_x = controls_center_x.saturating_sub(gauge_w / 2);

            let min_gauge_x = text_x.saturating_add(current_w + label_gap);
            let max_gauge_x = text_x
                .saturating_add(text_aw)
                .saturating_sub(gauge_w + total_w + label_gap);

            let mut gauge_x = if max_gauge_x >= min_gauge_x {
                ideal_gauge_x.clamp(min_gauge_x, max_gauge_x)
            } else {
                text_x + text_aw.saturating_sub(gauge_w) / 2
            };

            if text_aw > current_w + total_w + (gap_w * 2) {
                let current_x = gauge_x.saturating_sub(current_w + label_gap);
                let total_x = gauge_x + gauge_w + label_gap;

                if current_x < text_x || total_x + total_w > text_x + text_aw {
                    gauge_x = text_x + text_aw.saturating_sub(gauge_w) / 2;
                }

                frame.render_widget(
                    Paragraph::new(Span::styled(current_label, styles.text_muted)),
                    Rect {
                        x: gauge_x.saturating_sub(current_w + label_gap),
                        y: row2_y,
                        width: current_w,
                        height: 1,
                    },
                );

                frame.render_widget(
                    Paragraph::new(Span::styled(total_label, styles.text_muted)),
                    Rect {
                        x: gauge_x + gauge_w + label_gap,
                        y: row2_y,
                        width: total_w,
                        height: 1,
                    },
                );
            }

            frame.render_widget(
                CustomGauge::default()
                    .ratios(played, buffered)
                    .played_style(styles.progress_fg)
                    .buffered_style(styles.progress_bg)
                    .remaining_style(
                        Style::default()
                            .fg(styles.text.bg.unwrap_or_default())
                            .bg(styles.text.bg.unwrap_or_default()),
                    )
                    .use_unicode(true),
                Rect {
                    x: gauge_x,
                    y: row2_y,
                    width: gauge_w,
                    height: 1,
                },
            );
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct CustomGauge<'a> {
    block: Option<Block<'a>>,
    played_ratio: f64,
    buffered_ratio: f64,
    use_unicode: bool,
    style: Style,
    played_style: Style,
    buffered_style: Style,
    remaining_style: Style,
}

impl<'a> CustomGauge<'a> {
    fn ratios(mut self, played: f64, buffered: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&played),
            "Played ratio must be between 0 and 1"
        );
        assert!(
            (0.0..=1.0).contains(&buffered),
            "Buffered ratio must be between 0 and 1"
        );
        self.played_ratio = played;
        self.buffered_ratio = buffered;
        self
    }

    const fn use_unicode(mut self, use_unicode: bool) -> Self {
        self.use_unicode = use_unicode;
        self
    }

    fn played_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.played_style = style.into();
        self
    }

    fn buffered_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.buffered_style = style.into();
        self
    }

    fn remaining_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.remaining_style = style.into();
        self
    }

    fn render_gauge(&self, gauge_area: Rect, buf: &mut Buffer) {
        if gauge_area.is_empty() {
            return;
        }

        let width = gauge_area.width as f64;
        let played_pos = width * self.played_ratio;
        let buffered_pos = width * self.buffered_ratio;

        for y in gauge_area.top()..gauge_area.bottom() {
            for x in gauge_area.left()..gauge_area.right() {
                let pos = (x - gauge_area.left()) as f64;

                let mut symbol = symbols::block::FULL;
                let mut style = self.remaining_style;

                if pos < played_pos {
                    style = self.played_style;
                    if self.use_unicode && pos + 1.0 > played_pos {
                        symbol = unicode_block(played_pos - pos);
                    }
                } else if pos < buffered_pos {
                    style = self.buffered_style;
                    if self.use_unicode && pos + 1.0 > buffered_pos {
                        symbol = unicode_block(buffered_pos - pos);
                    }
                } else if self.use_unicode {
                    symbol = " ";
                }

                buf[(x, y)]
                    .set_symbol(symbol)
                    .set_fg(style.fg.unwrap_or_default())
                    .set_bg(style.bg.unwrap_or_default());
            }
        }
    }
}

impl Widget for CustomGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        if let Some(ref block) = self.block {
            block.render(area, buf);
        }
        let inner = self.block.as_ref().map_or(area, |b| b.inner(area));
        if !inner.is_empty() {
            self.render_gauge(inner, buf);
        }
    }
}

fn unicode_block(frac: f64) -> &'static str {
    match (frac * 8.0).round() as u16 {
        0 => " ",
        1 => symbols::block::ONE_EIGHTH,
        2 => symbols::block::ONE_QUARTER,
        3 => symbols::block::THREE_EIGHTHS,
        4 => symbols::block::HALF,
        5 => symbols::block::FIVE_EIGHTHS,
        6 => symbols::block::THREE_QUARTERS,
        7 => symbols::block::SEVEN_EIGHTHS,
        _ => symbols::block::FULL,
    }
}

fn format_duration(duration: u64) -> String {
    let total_seconds = duration / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}
