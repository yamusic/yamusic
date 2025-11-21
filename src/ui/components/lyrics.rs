use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
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
        let pos_ms = pos_ms;
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
        let mut current_ts = parsed.get(idx).map(|(t, _)| *t).unwrap_or(0);
        let mut next_ts = parsed
            .get(idx + 1)
            .map(|(t, _)| *t)
            .unwrap_or(current_ts + 1000);

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
        let center_row = inner.y + inner.height / 2;
        if idx > 0 && !intro_wait_active {
            let prev_idx = idx - 1;
            if let Some((_, prev_line)) = parsed.get(prev_idx) {
                let y = center_row.saturating_sub(1);
                if y >= inner.y && y < inner.y + inner.height {
                    if !prev_line.is_empty() {
                        draw_centered(
                            buf,
                            inner,
                            y,
                            prev_line,
                            Style::default().fg(colors::NEUTRAL),
                        );
                    }
                }
            }
        }

        let current_line = parsed.get(idx);
        let has_next = idx + 1 < len;
        let current_waiting = if intro_wait_active {
            true
        } else {
            current_line
                .map(|(_, line)| line.is_empty() && has_next)
                .unwrap_or(false)
        };

        let y = center_row;
        if y >= inner.y && y < inner.y + inner.height {
            if current_waiting {
                let frame = waiting_frame(pos_ms, 0);
                draw_centered(
                    buf,
                    inner,
                    y,
                    &frame,
                    Style::default()
                        .fg(colors::ACCENT)
                        .add_modifier(Modifier::BOLD),
                );
            } else if let Some((_, line)) = current_line {
                if !line.is_empty() {
                    draw_centered(
                        buf,
                        inner,
                        y,
                        line,
                        Style::default()
                            .fg(colors::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }

        let next_idx = if intro_wait_active { 0 } else { idx + 1 };
        if next_idx < len {
            if let Some((_, next_line)) = parsed.get(next_idx) {
                let y = center_row.saturating_add(1);
                if y >= inner.y && y < inner.y + inner.height {
                    if !next_line.is_empty() {
                        draw_centered(
                            buf,
                            inner,
                            y,
                            next_line,
                            Style::default().fg(colors::NEUTRAL),
                        );
                    }
                }
            }
        }

        let max_bar_w = inner.width.saturating_sub(8).min(30);
        if inner.height >= 3 && max_bar_w > 2 {
            let bar_x = inner.x + (inner.width.saturating_sub(max_bar_w)) / 2;
            let bar_y = center_row.saturating_add(2);
            let pos =
                ((frac.clamp(0.0, 1.0) * (max_bar_w - 1) as f64).round() as u16).min(max_bar_w - 1);
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
