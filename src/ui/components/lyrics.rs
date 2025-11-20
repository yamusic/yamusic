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
        let mut entries = Vec::new();
        for raw_line in text.lines() {
            let mut rest = raw_line;
            let mut timestamps: Vec<u64> = Vec::new();

            while let Some(open) = rest.find('[') {
                if let Some(close_rel) = rest[open..].find(']') {
                    let close = open + close_rel;
                    let tag = &rest[open + 1..close];
                    if let Some(colon) = tag.find(':') {
                        let (min_s, sec_s) = tag.split_at(colon);
                        let sec_s = &sec_s[1..];
                        if let Ok(min) = min_s.parse::<u64>() {
                            let mut ms = 0u64;
                            if let Some(dot) = sec_s.find('.') {
                                if let Ok(sec) = sec_s[..dot].parse::<u64>() {
                                    let frac = &sec_s[dot + 1..];
                                    let mut frac_ms = 0u64;
                                    if let Ok(f) = frac.parse::<u64>() {
                                        frac_ms = match frac.len() {
                                            3 => f,
                                            2 => f * 10,
                                            1 => f * 100,
                                            _ => {
                                                let mut s = frac.to_string();
                                                s.truncate(3);
                                                s.parse::<u64>().unwrap_or(0)
                                            }
                                        };
                                    }
                                    ms = (min * 60 + sec) * 1000 + frac_ms;
                                }
                            } else if let Ok(sec) = sec_s.parse::<u64>() {
                                ms = (min * 60 + sec) * 1000;
                            }
                            timestamps.push(ms);
                        }
                    }
                    let after = close + 1;
                    rest = &rest[after..];
                } else {
                    break;
                }
            }

            let content = rest.trim().to_string();
            if !timestamps.is_empty() && !content.is_empty() {
                for t in timestamps {
                    entries.push((t, content.clone()));
                }
            }
        }

        entries.sort_by_key(|e| e.0);
        entries
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
                let line_w = UnicodeWidthStr::width(*line) as u16;
                let x = inner.x + (inner.width.saturating_sub(line_w)) / 2;
                let y = start_row + i as u16;
                if y < inner.y + inner.height {
                    buf.set_stringn(x, y, line, inner.width as usize, Style::default());
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
        let current_ts = parsed.get(idx).map(|(t, _)| *t).unwrap_or(0);
        let next_ts = parsed
            .get(idx + 1)
            .map(|(t, _)| *t)
            .unwrap_or(current_ts + 1000);
        let denom = if next_ts > current_ts {
            next_ts - current_ts
        } else {
            1
        };
        let frac = ((pos_ms.saturating_sub(current_ts)) as f64) / (denom as f64);
        let center_row = inner.y + inner.height / 2;
        if idx > 0 {
            if let Some((_, prev_line)) = parsed.get(idx - 1) {
                let y = center_row.saturating_sub(1);
                if y >= inner.y && y < inner.y + inner.height {
                    let w = UnicodeWidthStr::width(prev_line.as_str()) as u16;
                    let x = inner.x + (inner.width.saturating_sub(w)) / 2;
                    buf.set_stringn(
                        x,
                        y,
                        prev_line,
                        inner.width as usize,
                        Style::default().fg(colors::NEUTRAL),
                    );
                }
            }
        }
        if let Some((_, cur_line)) = parsed.get(idx) {
            let y = center_row;
            if y >= inner.y && y < inner.y + inner.height {
                let w = UnicodeWidthStr::width(cur_line.as_str()) as u16;
                let x = inner.x + (inner.width.saturating_sub(w)) / 2;
                buf.set_stringn(
                    x,
                    y,
                    cur_line,
                    inner.width as usize,
                    Style::default()
                        .fg(colors::ACCENT)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }
        if let Some((_, next_line)) = parsed.get(idx + 1) {
            let y = center_row.saturating_add(1);
            if y >= inner.y && y < inner.y + inner.height {
                let w = UnicodeWidthStr::width(next_line.as_str()) as u16;
                let x = inner.x + (inner.width.saturating_sub(w)) / 2;
                buf.set_stringn(
                    x,
                    y,
                    next_line,
                    inner.width as usize,
                    Style::default().fg(colors::NEUTRAL),
                );
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
