use yandex_music::model::{
    album::Album, artist::Artist, playlist::Playlist, search::Search as SearchModel, track::Track,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchTab {
    Tracks,
    Albums,
    Artists,
    Playlists,
}

impl SearchTab {
    pub fn all() -> &'static [SearchTab] {
        &[
            SearchTab::Tracks,
            SearchTab::Albums,
            SearchTab::Artists,
            SearchTab::Playlists,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            SearchTab::Tracks => "Tracks",
            SearchTab::Albums => "Albums",
            SearchTab::Artists => "Artists",
            SearchTab::Playlists => "Playlists",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            SearchTab::Tracks => 0,
            SearchTab::Albums => 1,
            SearchTab::Artists => 2,
            SearchTab::Playlists => 3,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => SearchTab::Tracks,
            1 => SearchTab::Albums,
            2 => SearchTab::Artists,
            3 => SearchTab::Playlists,
            _ => SearchTab::Tracks,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % Self::all().len())
    }

    pub fn prev(&self) -> Self {
        let tabs = Self::all();
        Self::from_index(if self.index() == 0 {
            tabs.len() - 1
        } else {
            self.index() - 1
        })
    }
}

pub struct SearchState {
    pub results: Option<SearchModel>,
    pub current_page: u32,
    pub is_loading: bool,
    pub is_loading_more: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            results: None,
            current_page: 0,
            is_loading: false,
            is_loading_more: false,
        }
    }

    pub fn begin_search(&mut self) {
        self.results = None;
        self.current_page = 0;
        self.is_loading = true;
        self.is_loading_more = false;
    }

    pub fn apply_results(&mut self, results: SearchModel) -> Option<SearchTab> {
        let tab = self.optimal_tab_for(&results);
        self.results = Some(results);
        self.is_loading = false;
        self.is_loading_more = false;
        self.current_page = 0;
        tab
    }

    pub fn merge_results(&mut self, additional: SearchModel, page: u32) {
        if let Some(existing) = &mut self.results {
            if let (Some(existing_tracks), Some(new_tracks)) =
                (&mut existing.tracks, additional.tracks)
            {
                existing_tracks.results.extend(new_tracks.results);
            }
            if let (Some(existing_albums), Some(new_albums)) =
                (&mut existing.albums, additional.albums)
            {
                existing_albums.results.extend(new_albums.results);
            }
            if let (Some(existing_artists), Some(new_artists)) =
                (&mut existing.artists, additional.artists)
            {
                existing_artists.results.extend(new_artists.results);
            }
            if let (Some(existing_playlists), Some(new_playlists)) =
                (&mut existing.playlists, additional.playlists)
            {
                existing_playlists.results.extend(new_playlists.results);
            }
        }
        self.current_page = page;
        self.is_loading_more = false;
    }

    pub fn optimal_tab_for(&self, results: &SearchModel) -> Option<SearchTab> {
        results.best.as_ref().map(|b| match b.item_type.as_str() {
            "track" => SearchTab::Tracks,
            "album" => SearchTab::Albums,
            "artist" => SearchTab::Artists,
            "playlist" => SearchTab::Playlists,
            _ => SearchTab::Tracks,
        })
    }

    pub fn has_more_for_tab(&self, tab: SearchTab, current_count: usize) -> bool {
        let results = match &self.results {
            Some(r) => r,
            None => return false,
        };
        match tab {
            SearchTab::Tracks => results
                .tracks
                .as_ref()
                .is_some_and(|t| current_count < t.total as usize),
            SearchTab::Albums => results
                .albums
                .as_ref()
                .is_some_and(|a| current_count < a.total as usize),
            SearchTab::Artists => results
                .artists
                .as_ref()
                .is_some_and(|a| current_count < a.total as usize),
            SearchTab::Playlists => results
                .playlists
                .as_ref()
                .is_some_and(|p| current_count < p.total as usize),
        }
    }

    pub fn should_load_more(
        &self,
        tab: SearchTab,
        selected_index: usize,
        loaded_count: usize,
    ) -> bool {
        if self.is_loading_more || !self.has_more_for_tab(tab, loaded_count) {
            return false;
        }
        loaded_count > 0 && selected_index >= loaded_count.saturating_sub(2)
    }

    pub fn has_results(&self) -> bool {
        self.results.is_some()
    }

    pub fn tracks(&self) -> Vec<Track> {
        self.results
            .as_ref()
            .and_then(|r| r.tracks.as_ref())
            .map(|t| t.results.clone())
            .unwrap_or_default()
    }

    pub fn albums(&self) -> Vec<Album> {
        self.results
            .as_ref()
            .and_then(|r| r.albums.as_ref())
            .map(|a| a.results.clone())
            .unwrap_or_default()
    }

    pub fn artists(&self) -> Vec<Artist> {
        self.results
            .as_ref()
            .and_then(|r| r.artists.as_ref())
            .map(|a| a.results.clone())
            .unwrap_or_default()
    }

    pub fn playlists(&self) -> Vec<Playlist> {
        self.results
            .as_ref()
            .and_then(|r| r.playlists.as_ref())
            .map(|p| p.results.clone())
            .unwrap_or_default()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
