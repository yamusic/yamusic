use std::ops::Range;

use im::Vector;

use crate::framework::signals::Signal;

#[derive(Debug, Clone, PartialEq)]
pub enum FetchState {
    Idle,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct DataChunk<T: Clone> {
    pub items: Vector<T>,
    pub start: usize,
    pub total: Option<usize>,
    pub has_more: bool,
}

impl<T: Clone> DataChunk<T> {
    pub fn empty() -> Self {
        Self {
            items: Vector::new(),
            start: 0,
            total: Some(0),
            has_more: false,
        }
    }

    pub fn from_complete(items: Vector<T>) -> Self {
        let len = items.len();
        Self {
            items,
            start: 0,
            total: Some(len),
            has_more: false,
        }
    }
}

pub trait DataSource<T>: Send + Sync {
    fn total(&self) -> Option<usize>;

    fn range(&self, range: Range<usize>) -> Vector<T>
    where
        T: Clone;

    fn is_loaded(&self, range: Range<usize>) -> bool;

    fn request_range(&self, range: Range<usize>);

    fn fetch_state(&self) -> FetchState;

    fn changed_signal(&self) -> Signal<u64>;

    fn refresh(&self);
}

pub trait DataSourceExt<T>: DataSource<T> {
    fn get(&self, index: usize) -> Option<T>
    where
        T: Clone,
    {
        self.range(index..index + 1).into_iter().next()
    }

    fn is_empty(&self) -> bool {
        self.total() == Some(0)
    }

    fn has_data(&self) -> bool {
        self.total().is_none_or(|t| t > 0)
    }
}

impl<T, D: DataSource<T>> DataSourceExt<T> for D {}

pub struct StaticDataSource<T> {
    items: std::sync::RwLock<Vector<T>>,
    changed: Signal<u64>,
}

impl<T: Clone> StaticDataSource<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: std::sync::RwLock::new(Vector::from(items)),
            changed: Signal::new(0),
        }
    }

    pub fn new_from_vector(items: Vector<T>) -> Self {
        Self {
            items: std::sync::RwLock::new(items),
            changed: Signal::new(0),
        }
    }

    pub fn set_items(&self, items: Vec<T>) {
        *self.items.write().unwrap() = Vector::from(items);
        self.changed.update(|v| *v += 1);
    }
}

impl<T: Clone + Send + Sync> DataSource<T> for StaticDataSource<T> {
    fn total(&self) -> Option<usize> {
        Some(self.items.read().unwrap().len())
    }

    fn range(&self, range: Range<usize>) -> Vector<T> {
        let items = self.items.read().unwrap();
        let start = range.start.min(items.len());
        let end = range.end.min(items.len());
        items.clone().slice(start..end)
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        true
    }

    fn request_range(&self, _range: Range<usize>) {}

    fn fetch_state(&self) -> FetchState {
        FetchState::Loaded
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {}
}

pub struct SignalDataSource<T> {
    items: Signal<Vector<T>>,
    changed: Signal<u64>,
}

impl<T: Clone + Send + Sync + 'static> SignalDataSource<T> {
    pub fn new(items: Signal<Vector<T>>) -> Self {
        Self {
            items,
            changed: Signal::new(0),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> DataSource<T> for SignalDataSource<T> {
    fn total(&self) -> Option<usize> {
        Some(self.items.with(|v| v.len()))
    }

    fn range(&self, range: Range<usize>) -> Vector<T> {
        self.items.with(|items| {
            let start = range.start.min(items.len());
            let end = range.end.min(items.len());
            items.clone().slice(start..end)
        })
    }

    fn is_loaded(&self, _range: Range<usize>) -> bool {
        true
    }

    fn request_range(&self, _range: Range<usize>) {}

    fn fetch_state(&self) -> FetchState {
        FetchState::Loaded
    }

    fn changed_signal(&self) -> Signal<u64> {
        self.changed.clone()
    }

    fn refresh(&self) {}
}
