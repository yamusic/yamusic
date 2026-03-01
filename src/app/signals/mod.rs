mod library;
mod lyrics;
mod navigation;

pub use library::LibrarySignals;
pub use lyrics::LyricsSignals;
pub use navigation::NavigationSignals;

use crate::{audio::signals::AudioSignals, framework::theme::Theme};
use std::sync::Arc;

pub struct AppSignals {
    pub audio: AudioSignals,
    pub navigation: NavigationSignals,
    pub library: LibrarySignals,
    pub lyrics: LyricsSignals,
    pub theme: Arc<Theme>,
    pub is_focused: crate::framework::reactive::Signal<bool>,
}

impl AppSignals {
    pub fn new(api: Arc<crate::http::ApiService>) -> Self {
        let audio = AudioSignals::new();
        let library = LibrarySignals::new(api.clone());
        let lyrics = LyricsSignals::new(api.clone(), &audio);

        Self {
            audio,
            navigation: NavigationSignals::new(),
            library,
            lyrics,
            theme: Arc::new(Theme::default()),
            is_focused: crate::framework::reactive::signal(true),
        }
    }
}
