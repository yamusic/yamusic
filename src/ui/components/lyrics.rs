use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use crate::audio::progress::TrackProgress;
use crate::util::colors;
use unicode_width::UnicodeWidthStr;

pub struct LyricsWidget<'a> {
    lyrics_text: Option<&'a str>,
    progress: &'a TrackProgress,
}

impl<'a> LyricsWidget<'a> {
    pub fn new(lyrics_text: Option<&'a str>, progress: &'a TrackProgress) -> Self {
        Self {
            lyrics_text,
            progress,
        }
    }

    fn parse_lrc(text: &str) -> Vec<(u64, String)> {
        let mut out = Vec::new();
        let mut seq = 0usize;

        for line in text.lines() {
            let bytes = line.as_bytes();
            let mut i = 0;
            let mut timestamps = Vec::new();
            let mut last_end = 0;

            while i < bytes.len() {
                if bytes[i] == b'[' {
                    let start = i + 1;
                    if let Some(end) = line[start..].find(']').map(|e| start + e) {
                        let tag = &line[start..end];
                        if let Some(ts) = parse_timestamp(tag) {
                            timestamps.push(ts);
                            last_end = end + 1;
                        }
                        i = end + 1;
                        continue;
                    } else {
                        break;
                    }
                }
                i += 1;
            }

            if timestamps.is_empty() {
                continue;
            }

            let content_slice = line[last_end..].trim();
            let content = if content_slice.is_empty() {
                String::new()
            } else {
                content_slice.to_string()
            };

            for t in timestamps {
                out.push((t, seq, content.clone()));
                seq += 1;
            }
        }

        out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        out.into_iter().map(|(t, _, line)| (t, line)).collect()
    }
}

impl<'a> Widget for LyricsWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if self.lyrics_text.is_none() {
            let hint = "No lyrics available";
            let x = inner.x + (inner.width.saturating_sub(hint.len() as u16)) / 2;
            let y = inner.y + inner.height / 2;
            buf.set_stringn(x, y, hint, inner.width as usize, Style::default());
            return;
        }

        let text = self.lyrics_text.unwrap();
        let parsed = Self::parse_lrc(text);

        if parsed.is_empty() {
            let lines: Vec<&str> = text.lines().collect();
            let block_h = lines.len() as u16;
            let start_row = inner.y + inner.height.saturating_sub(block_h) / 2;
            for (i, line) in lines.iter().enumerate() {
                let y = start_row + i as u16;
                if y < inner.y + inner.height {
                    draw_centered(buf, inner, y, line, Style::default());
                }
            }
            return;
        }

        let (pos_ms, _) = self.progress.get_progress();
        let mut idx = 0usize;
        for (i, (t, _)) in parsed.iter().enumerate() {
            if *t <= pos_ms {
                idx = i;
            } else {
                break;
            }
        }

        let len = parsed.len();
        let first_ts = parsed.first().map(|(t, _)| *t).unwrap_or(0);
        let before_first = pos_ms < first_ts;
        let intro_wait_active = before_first && first_ts >= 3000;

        let is_last_line = idx >= len.saturating_sub(1);

        let mut current_ts = parsed.get(idx).map(|(t, _)| *t).unwrap_or(0);
        let mut next_ts = if is_last_line {
            current_ts + 3000
        } else {
            parsed
                .get(idx + 1)
                .map(|(t, _)| *t)
                .unwrap_or(current_ts + 1000)
        };

        if intro_wait_active {
            current_ts = 0;
            next_ts = first_ts;
        }

        let denom = if next_ts > current_ts {
            next_ts - current_ts
        } else {
            1
        };
        let frac = ((pos_ms.saturating_sub(current_ts)) as f64) / (denom as f64);
        let frac = frac.clamp(0.0, 1.0);

        let scroll_frac = if frac < 0.8 {
            0.0
        } else {
            ((frac - 0.8) / 0.4).min(1.0)
        };
        let eased_scroll = ease_in_out_cubic(scroll_frac);

        let center_y_float = (inner.height / 2) as f64;
        let line_spacing = 2.0;

        let positions_to_render = [-2, -1, 0, 1, 2, 3];

        for &relative_pos in &positions_to_render {
            let line_idx = if intro_wait_active {
                if relative_pos == 0 {
                    0
                } else if relative_pos == 1 {
                    0
                } else if relative_pos == 2 {
                    1
                } else {
                    continue;
                }
            } else {
                (idx as i32 + relative_pos) as usize
            };

            if line_idx >= len {
                continue;
            }

            let (_, line_text) = &parsed[line_idx];

            let y_float = center_y_float + (relative_pos as f64 * line_spacing)
                - (eased_scroll * line_spacing);
            if y_float < -1.0 || y_float >= inner.height as f64 + 1.0 {
                continue;
            }

            let is_waiting = intro_wait_active && relative_pos == 0;

            let signed_distance = relative_pos as f64 - eased_scroll;
            let opacity = calculate_opacity(signed_distance);

            if opacity <= 0.0 {
                continue;
            }

            let base_color = if intro_wait_active {
                if relative_pos == 0 {
                    colors::ACCENT
                } else {
                    colors::NEUTRAL
                }
            } else {
                animate_line_color(line_idx, idx, eased_scroll)
            };

            let style = if intro_wait_active && relative_pos == 0 {
                Style::default()
                    .fg(blend_color_with_bg(base_color, opacity))
                    .add_modifier(Modifier::BOLD)
            } else if line_idx == idx && eased_scroll < 0.5 {
                Style::default()
                    .fg(blend_color_with_bg(base_color, opacity))
                    .add_modifier(Modifier::BOLD)
            } else if line_idx == idx + 1 && eased_scroll >= 0.5 {
                Style::default()
                    .fg(blend_color_with_bg(base_color, opacity))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(blend_color_with_bg(base_color, opacity))
            };

            let display_text = if is_waiting {
                waiting_frame(pos_ms, 0)
            } else {
                line_text.clone()
            };

            if !display_text.is_empty() || is_waiting {
                render_line_smooth(buf, inner, y_float, &display_text, style);
            }
        }

        let max_bar_w = inner.width.saturating_sub(8).min(30);
        if inner.height >= 5 && max_bar_w > 2 {
            let bar_x = inner.x + (inner.width.saturating_sub(max_bar_w)) / 2;
            let bar_y = inner.y + inner.height - 2;
            let pos = ((frac * (max_bar_w - 1) as f64).round() as u16).min(max_bar_w - 1);

            for i in 0..max_bar_w {
                let ch = if i == pos { '•' } else { '─' };
                let style = if i == pos {
                    Style::default().fg(colors::ACCENT)
                } else {
                    Style::default().fg(colors::NEUTRAL)
                };
                buf.set_stringn(
                    bar_x + i,
                    bar_y,
                    &ch.to_string(),
                    inner.width as usize,
                    style,
                );
            }
        }
    }
}

fn render_line_smooth(buf: &mut Buffer, area: Rect, y_float: f64, text: &str, style: Style) {
    let y_int = y_float.floor() as i32;
    let y_frac = y_float - y_float.floor();

    let y_top = (area.y as i32 + y_int) as u16;
    let y_bottom = (area.y as i32 + y_int + 1) as u16;

    if y_frac < 0.5 {
        if y_top >= area.y && y_top < area.y + area.height {
            draw_centered(buf, area, y_top, text, style);
        }
    } else {
        if y_bottom >= area.y && y_bottom < area.y + area.height {
            draw_centered(buf, area, y_bottom, text, style);
        }
    }
}

fn calculate_opacity(signed_distance: f64) -> f64 {
    const MIN_OPACITY: f64 = 0.3;

    let abs_distance = signed_distance.abs();

    if abs_distance <= 1.0 {
        1.0
    } else if abs_distance >= 3.0 {
        0.0
    } else if abs_distance >= 2.0 {
        let fade_progress = abs_distance - 2.0;
        MIN_OPACITY * (1.0 - fade_progress)
    } else {
        let fade_progress = abs_distance - 1.0;
        1.0 - (fade_progress * (1.0 - MIN_OPACITY))
    }
}

fn animate_line_color(line_idx: usize, current_idx: usize, scroll_progress: f64) -> Color {
    let is_old_current = line_idx == current_idx;
    let is_new_current = line_idx == current_idx + 1;

    if is_old_current {
        blend_colors(colors::ACCENT, colors::NEUTRAL, scroll_progress)
    } else if is_new_current {
        blend_colors(colors::NEUTRAL, colors::ACCENT, scroll_progress)
    } else {
        colors::NEUTRAL
    }
}

fn blend_colors(from: Color, to: Color, progress: f64) -> Color {
    let (r1, g1, b1) = match from {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return from,
    };

    let (r2, g2, b2) = match to {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return to,
    };

    let r = (r1 as f64 + (r2 as f64 - r1 as f64) * progress) as u8;
    let g = (g1 as f64 + (g2 as f64 - g1 as f64) * progress) as u8;
    let b = (b1 as f64 + (b2 as f64 - b1 as f64) * progress) as u8;

    Color::Rgb(r, g, b)
}

fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn blend_color_with_bg(color: Color, opacity: f64) -> Color {
    if opacity >= 0.95 {
        return color;
    }

    let (r, g, b) = match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return color,
    };

    let bg_r = 13u8;
    let bg_g = 13u8;
    let bg_b = 13u8;

    let blended_r = (r as f64 * opacity + bg_r as f64 * (1.0 - opacity)) as u8;
    let blended_g = (g as f64 * opacity + bg_g as f64 * (1.0 - opacity)) as u8;
    let blended_b = (b as f64 * opacity + bg_b as f64 * (1.0 - opacity)) as u8;

    Color::Rgb(blended_r, blended_g, blended_b)
}

#[inline]
fn parse_timestamp(tag: &str) -> Option<u64> {
    let mut parts = tag.split(':');
    let min = parts.next()?.parse::<u64>().ok()?;
    let rest = parts.next()?;

    let (sec, ms) = if let Some(dot) = rest.find('.') {
        let sec = rest[..dot].parse::<u64>().ok()?;
        if sec > 59 {
            return None;
        }

        let mut frac_value = 0u64;
        let mut digits = 0u32;
        for b in rest[dot + 1..].bytes() {
            if !b.is_ascii_digit() {
                break;
            }
            if digits == 3 {
                break;
            }
            frac_value = frac_value * 10 + (b - b'0') as u64;
            digits += 1;
        }

        if digits == 0 {
            return None;
        }

        let ms = match digits {
            1 => frac_value * 100,
            2 => frac_value * 10,
            _ => frac_value,
        };
        (sec, ms)
    } else {
        let sec = rest.parse().ok()?;
        if sec > 59 {
            return None;
        }
        (sec, 0)
    };

    Some((min * 60 + sec) * 1000 + ms)
}

fn waiting_frame(pos_ms: u64, phase_ms: u64) -> String {
    const FRAME_STEP_MS: u64 = 150;
    const CHAR_LEVELS: [char; 3] = ['·', '•', '●'];
    const STATES: [[usize; 3]; 9] = [
        [0, 0, 0],
        [1, 0, 0],
        [2, 0, 0],
        [1, 1, 0],
        [0, 2, 0],
        [0, 1, 1],
        [0, 0, 2],
        [0, 0, 1],
        [0, 0, 0],
    ];

    let step = ((pos_ms + phase_ms) / FRAME_STEP_MS) as usize % STATES.len();
    let current_state = STATES[step];

    let mut out = String::with_capacity(12);
    for (i, &level_idx) in current_state.iter().enumerate() {
        out.push(CHAR_LEVELS[level_idx]);
        if i < 2 {
            out.push(' ');
        }
    }

    out
}

fn draw_centered(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    if y < area.y || y >= area.y + area.height {
        return;
    }
    let width = UnicodeWidthStr::width(text) as u16;
    let x = area.x + area.width.saturating_sub(width) / 2;
    buf.set_stringn(x, y, text, area.width as usize, style);
}
