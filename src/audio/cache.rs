use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct UrlCache {
    cache: Arc<RwLock<HashMap<String, (String, String, u32)>>>,
}

impl UrlCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, track_id: &str) -> Option<(String, String, u32)> {
        self.cache.read().unwrap().get(track_id).cloned()
    }

    pub fn insert(&self, track_id: String, url: String, codec: String, bitrate: u32) {
        self.cache
            .write()
            .unwrap()
            .insert(track_id, (url, codec, bitrate));
    }
}
