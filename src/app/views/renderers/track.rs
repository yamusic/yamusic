use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::time::{SystemTime, UNIX_EPOCH};
use unicode_width::UnicodeWidthChar;
use yandex_music::model::track::Track;

use super::icons::{HEART_EMPTY, HEART_FILLED};
use crate::{
    app::{
        data::{ItemRenderer, ListItem, MatchHighlights, SearchScope},
        signals::LibrarySignals,
        views::icons::HEART_CROSSED,
    },
    framework::{signals::Signal, theme::ThemeStyles},
};

fn active_track_icon(is_playing: bool) -> &'static str {
    if is_playing {
        const FRAME_STEP_MS: u64 = 100;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let step = (now / FRAME_STEP_MS) as usize % 6;

        let level_idx = match step {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 2,
            4 => 1,
            _ => 0,
        };

        match level_idx {
            0 => "·",
            1 => "•",
            2 => "●",
            _ => "·",
        }
    } else {
        "•"
    }
}

pub struct TrackRenderer {
    library: LibrarySignals,
    playing_id: Signal<Option<String>>,
    playing_index: Option<Signal<usize>>,
    is_playing: Signal<bool>,
    show_album: bool,
    show_duration: bool,
    show_number: bool,
    theme: Signal<ThemeStyles>,
}

impl TrackRenderer {
    pub fn new(
        library: LibrarySignals,
        playing_id: Signal<Option<String>>,
        is_playing: Signal<bool>,
        theme: Signal<ThemeStyles>,
    ) -> Self {
        Self {
            library,
            playing_id,
            playing_index: None,
            is_playing,
            show_album: true,
            show_duration: true,
            show_number: false,
            theme,
        }
    }

    pub fn with_queue_index(mut self, index: Signal<usize>) -> Self {
        self.playing_index = Some(index);
        self
    }

    pub fn with_album(mut self, show: bool) -> Self {
        self.show_album = show;
        self
    }

    pub fn with_duration(mut self, show: bool) -> Self {
        self.show_duration = show;
        self
    }

    pub fn with_number(mut self, show: bool) -> Self {
        self.show_number = show;
        self
    }

    fn format_duration(duration: std::time::Duration) -> String {
        let secs = duration.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}:{:02}", mins, secs)
    }

    #[allow(dead_code)]
    fn truncate_or_pad(s: &str, width: usize) -> String {
        use unicode_width::UnicodeWidthStr;
        let display_width = s.width();
        if display_width > width {
            let mut result = String::new();
            let mut current_width = 0;
            for ch in s.chars() {
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
            format!("{}{}", s, " ".repeat(width - display_width))
        }
    }
}

impl ItemRenderer<Track> for TrackRenderer {
    fn render(
        &self,
        track: &Track,
        index: usize,
        is_selected: bool,
        _is_playing: bool,
    ) -> ListItem<'static> {
        self.render_with_context(
            track,
            index,
            is_selected,
            _is_playing,
            120,
            &MatchHighlights::default(),
        )
    }

    fn render_with_context(
        &self,
        track: &Track,
        index: usize,
        is_selected: bool,
        _is_playing: bool,
        available_width: u16,
        highlights: &MatchHighlights,
    ) -> ListItem<'static> {
        let styles = self.theme.get();
        let title = track.title.clone().unwrap_or_else(|| "Unknown".to_string());
        let artists: String = track
            .artists
            .iter()
            .filter_map(|a| a.name.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let album_title = if self.show_album {
            track
                .albums
                .first()
                .and_then(|a| a.title.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        let duration_str = if self.show_duration && track.duration.is_some() {
            Self::format_duration(track.duration.unwrap())
        } else {
            String::new()
        };

        let is_liked = self.library.is_liked(&track.id);
        let is_disliked = self.library.is_disliked(&track.id);
        let is_current = if let Some(idx_signal) = &self.playing_index {
            idx_signal.get() == index
        } else {
            let current_id = self.playing_id.get();
            current_id.as_ref() == Some(&track.id)
        };
        let playing = self.is_playing.get();

        let icon_width = 2usize;
        let heart_width = 2usize;
        let duration_total: usize = if self.show_duration { 11 } else { 0 };
        let gap_width = 1usize;
        let num_gaps = if self.show_album { 3 } else { 2 };
        let fixed_width = icon_width + heart_width + duration_total + (gap_width * num_gaps);
        let remaining = (available_width as usize).saturating_sub(fixed_width);

        let (title_width, artist_width, album_width) = if self.show_album {
            let title_w = (remaining * 40) / 100;
            let artist_w = (remaining * 30) / 100;
            let album_w = remaining.saturating_sub(title_w + artist_w);
            (title_w, artist_w, album_w)
        } else {
            let title_w = (remaining * 55) / 100;
            let artist_w = remaining.saturating_sub(title_w);
            (title_w, artist_w, 0)
        };

        let mut spans = Vec::new();

        if is_current {
            let icon = active_track_icon(playing);
            spans.push(Span::styled(format!("{} ", icon), styles.accent));
        } else if self.show_number {
            spans.push(Span::styled(format!("{:2}", index + 1), styles.text_muted));
        } else {
            spans.push(Span::raw("  "));
        }

        let heart = if is_disliked {
            HEART_CROSSED
        } else if is_liked {
            HEART_FILLED
        } else {
            HEART_EMPTY
        };
        let heart_style = if is_selected {
            if is_liked {
                styles.accent.add_modifier(Modifier::BOLD)
            } else {
                styles.accent
            }
        } else if is_liked {
            styles.text_muted.add_modifier(Modifier::BOLD)
        } else {
            styles.text_muted
        };
        spans.push(Span::styled(format!("{}  ", heart), heart_style));

        let hl_style = if is_selected {
            Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            styles.accent.add_modifier(Modifier::BOLD)
        };

        let create_highlighted_spans = |text: &str,
                                        width: usize,
                                        base_style: Style,
                                        match_positions: &[usize],
                                        highlight_style: Style|
         -> Vec<Span<'static>> {
            let mut result = Vec::new();
            let mut current_segment = String::new();
            let mut current_width = 0usize;
            let match_set: std::collections::HashSet<usize> =
                match_positions.iter().copied().collect();

            for (i, ch) in text.chars().enumerate() {
                let ch_width = ch.width().unwrap_or(0);
                if current_width + ch_width + 1 > width {
                    if !current_segment.is_empty() {
                        result.push(Span::styled(current_segment.clone(), base_style));
                        current_segment.clear();
                    }
                    result.push(Span::styled("…".to_string(), base_style));
                    current_width += 1;
                    break;
                }

                if !match_positions.is_empty() && match_set.contains(&i) {
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

            if current_width < width {
                result.push(Span::styled(" ".repeat(width - current_width), base_style));
            }

            result
        };

        let base_text_style = if is_selected {
            Style::default()
        } else {
            Style::default().fg(styles.text.fg.unwrap_or(ratatui::style::Color::White))
        };

        let (title_base_style, artist_base_style, album_base_style) = match highlights.search_scope
        {
            Some(SearchScope::Full) => (base_text_style, base_text_style, base_text_style),
            Some(SearchScope::Title) => (base_text_style, styles.text_muted, styles.text_muted),
            Some(SearchScope::Artist) => (styles.text_muted, base_text_style, styles.text_muted),
            Some(SearchScope::Album) => (styles.text_muted, styles.text_muted, base_text_style),
            None => (base_text_style, styles.text_muted, styles.text_muted),
        };

        let title_style = if is_current {
            styles.accent.add_modifier(Modifier::BOLD)
        } else if is_disliked {
            styles.text_muted.add_modifier(Modifier::DIM)
        } else {
            title_base_style
        };
        spans.extend(create_highlighted_spans(
            &title,
            title_width,
            title_style,
            &highlights.title,
            hl_style,
        ));

        spans.push(Span::raw(" "));

        spans.extend(create_highlighted_spans(
            &artists,
            artist_width,
            artist_base_style,
            &highlights.artist,
            hl_style,
        ));

        if self.show_album && album_width > 0 {
            spans.push(Span::raw(" "));
            spans.extend(create_highlighted_spans(
                &album_title,
                album_width,
                album_base_style,
                &highlights.album,
                hl_style,
            ));
        }

        if self.show_duration && !duration_str.is_empty() {
            spans.push(Span::raw(" "));
            let duration_formatted = format!("{:>5}", duration_str);
            spans.push(Span::styled(duration_formatted, styles.text_muted));
            spans.push(Span::raw("   "));
        }

        let style = if is_selected {
            styles.selected
        } else if is_disliked {
            styles.text_muted.add_modifier(Modifier::DIM)
        } else {
            styles.text
        };

        ListItem::from_lines(vec![Line::from(spans)]).style(style)
    }
}
