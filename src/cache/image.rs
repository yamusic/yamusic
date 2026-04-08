use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use image::DynamicImage;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use crate::framework::reactive::Signal;

#[derive(Clone)]
enum CacheEntry {
    Loading,
    Ready(Arc<DynamicImage>),
    Failed,
}

pub struct ImageCache {
    entries: DashMap<String, CacheEntry>,
    version: Signal<u64>,
}

static GLOBAL: OnceLock<Arc<ImageCache>> = OnceLock::new();
static GLOBAL_PICKER: OnceLock<Picker> = OnceLock::new();

impl ImageCache {
    pub fn global() -> Arc<ImageCache> {
        Arc::clone(GLOBAL.get_or_init(|| {
            Arc::new(ImageCache {
                entries: DashMap::new(),
                version: Signal::new(0),
            })
        }))
    }

    pub fn set_global_picker(picker: Picker) {
        let _ = GLOBAL_PICKER.set(picker);
    }

    pub fn global_picker() -> Option<Picker> {
        GLOBAL_PICKER.get().cloned()
    }

    pub fn version(&self) -> &Signal<u64> {
        &self.version
    }

    pub fn get_or_fetch(&self, url: &str) -> Option<Arc<DynamicImage>> {
        if let Some(entry) = self.entries.get(url) {
            return match entry.value() {
                CacheEntry::Ready(img) => Some(Arc::clone(img)),
                CacheEntry::Loading | CacheEntry::Failed => None,
            };
        }

        self.entries.insert(url.to_owned(), CacheEntry::Loading);

        let cache = ImageCache::global();
        let url_owned = url.to_owned();

        tokio::spawn(async move {
            let result = fetch_image(&url_owned).await;
            match result {
                Some(img) => {
                    let img = Arc::new(img);
                    cache.entries.insert(url_owned, CacheEntry::Ready(img));
                    cache.version.update(|v| *v += 1);
                }
                None => {
                    cache.entries.insert(url_owned, CacheEntry::Failed);
                }
            }
        });

        None
    }

    pub fn get_protocol(&self, url: &str, picker: &mut Picker) -> Option<StatefulProtocol> {
        self.get_or_fetch(url)
            .map(|img| picker.new_resize_protocol((*img).clone()))
    }

    pub fn resolve_cover_uri(uri: &str, size: &str) -> String {
        let uri = uri.replace("%%", size);
        if uri.starts_with("http") {
            uri
        } else {
            format!("https://{uri}")
        }
    }

    pub fn get_cover(&self, uri: &str, size: &str) -> Option<Arc<DynamicImage>> {
        let url = Self::resolve_cover_uri(uri, size);
        self.get_or_fetch(&url)
    }
}

async fn fetch_image(url: &str) -> Option<DynamicImage> {
    let response = reqwest::get(url).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    image::load_from_memory(&bytes).ok()
}
