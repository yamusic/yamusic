use std::ops::Range;
use std::sync::{Arc, RwLock};

use super::super::{DataSource, FetchState};
use crate::framework::signals::Signal;
use im::Vector;

pub struct PaginatedDataSource<Id, Item> {
    all_ids: Arc<RwLock<Vec<Id>>>,
    items: Arc<RwLock<Vector<Item>>>,
    loaded_count: Arc<RwLock<usize>>,
    is_loading: Arc<RwLock<bool>>,
    state: Signal<FetchState>,
    changed: Signal<u64>,
    page_size: usize,
    fetch_batch: Arc<RwLock<Arc<dyn Fn(Vec<Id>, usize) + Send + Sync>>>,
}

impl<Id, Item> Clone for PaginatedDataSource<Id, Item> {
    fn clone(&self) -> Self {
        Self {
            all_ids: self.all_ids.clone(),
            items: self.items.clone(),
            loaded_count: self.loaded_count.clone(),
            is_loading: self.is_loading.clone(),
            state: self.state.clone(),
            changed: self.changed.clone(),
            page_size: self.page_size,
            fetch_batch: self.fetch_batch.clone(),
        }
    }
}

impl<Id, Item> PaginatedDataSource<Id, Item>
where
    Id: Clone + Send + Sync + 'static,
    Item: Clone + Send + Sync + 'static,
{
    pub fn new(
        page_size: usize,
        fetch_batch: impl Fn(Vec<Id>, usize) + Send + Sync + 'static,
    ) -> Self {
        Self {
            all_ids: Arc::new(RwLock::new(Vec::new())),
            items: Arc::new(RwLock::new(Vector::new())),
            loaded_count: Arc::new(RwLock::new(0)),
            is_loading: Arc::new(RwLock::new(false)),
            state: Signal::new(FetchState::Idle),
            changed: Signal::new(0),
            page_size,
            fetch_batch: Arc::new(RwLock::new(Arc::new(fetch_batch))),
        }
    }

    pub fn set_fetch_batch(&self, fetch_batch: impl Fn(Vec<Id>, usize) + Send + Sync + 'static) {
        *self.fetch_batch.write().unwrap() = Arc::new(fetch_batch);
    }

    pub fn set_ids(&self, ids: Vec<Id>) {
        *self.all_ids.write().unwrap() = ids;
        *self.items.write().unwrap() = Vector::new();
        *self.loaded_count.write().unwrap() = 0;
        *self.is_loading.write().unwrap() = false;
        self.state.set(FetchState::Loading);
        self.changed.update(|v| *v += 1);

        self.trigger_load_more();
    }

    pub fn set_items(&self, items: Vec<Item>, loaded_count: usize) {
        *self.items.write().unwrap() = Vector::from(items);
        *self.loaded_count.write().unwrap() = loaded_count;
        *self.is_loading.write().unwrap() = false;
        self.state.set(FetchState::Loaded);
        self.changed.update(|v| *v += 1);
    }

    pub fn set_item_ids(&self, ids: Vec<Id>, items: Vec<Item>) {
        let loaded_count = items.len();
        *self.all_ids.write().unwrap() = ids;
        *self.items.write().unwrap() = Vector::from(items);
        *self.loaded_count.write().unwrap() = loaded_count;
        *self.is_loading.write().unwrap() = false;
        self.state.set(FetchState::Loaded);
        self.changed.update(|v| *v += 1);
    }

    pub fn append_items(&self, new_items: Vec<Item>, new_loaded_count: usize) {
        {
            let mut items_lock = self.items.write().unwrap();
            items_lock.extend(new_items);
        }
        *self.loaded_count.write().unwrap() = new_loaded_count;
        *self.is_loading.write().unwrap() = false;
        self.state.set(FetchState::Loaded);
        self.changed.update(|v| *v += 1);
    }

    pub fn has_more(&self) -> bool {
        let loaded = *self.loaded_count.read().unwrap();
        let total = self.all_ids.read().unwrap().len();
        loaded < total
    }

    pub fn is_loading(&self) -> bool {
        *self.is_loading.read().unwrap()
    }

    pub fn trigger_load_more(&self) {
        if *self.is_loading.read().unwrap() || !self.has_more() {
            return;
        }

        let all_ids = self.all_ids.read().unwrap();
        let start = *self.loaded_count.read().unwrap();
        let end = (start + self.page_size).min(all_ids.len());
        let batch = all_ids[start..end].to_vec();
        drop(all_ids);

        if batch.is_empty() {
            return;
        }

        *self.is_loading.write().unwrap() = true;

        let fetch = self.fetch_batch.read().unwrap().clone();
        (fetch)(batch, end);
    }
}

impl<Id, Item> DataSource<Item> for PaginatedDataSource<Id, Item>
where
    Id: Sync + Send + Clone + 'static,
    Item: Sync + Send + Clone + 'static,
{
    fn total(&self) -> Option<usize> {
        let items_len = self.items.read().unwrap().len();
        if items_len == 0 {
            let has_ids = !self.all_ids.read().unwrap().is_empty();
            if has_ids {
                return None;
            }
        }
        Some(items_len)
    }

    fn range(&self, range: Range<usize>) -> Vector<Item> {
        let items = self.items.read().unwrap();
        let start = range.start.min(items.len());
        let end = range.end.min(items.len());
        items.clone().slice(start..end)
    }

    fn is_loaded(&self, range: Range<usize>) -> bool {
        let items = self.items.read().unwrap();
        range.end <= items.len()
    }

    fn request_range(&self, range: Range<usize>) {
        let items_len = self.items.read().unwrap().len();

        if items_len > 0 && range.end <= items_len.saturating_sub(2) {
            return;
        }

        self.trigger_load_more();
    }

    fn fetch_state(&self) -> FetchState {
        self.state.get()
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {
        *self.all_ids.write().unwrap() = Vec::new();
        *self.items.write().unwrap() = Vector::new();
        *self.loaded_count.write().unwrap() = 0;
        *self.is_loading.write().unwrap() = false;
        self.state.set(FetchState::Idle);
        self.changed.update(|v| *v += 1);
    }
}
