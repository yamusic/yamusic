use std::{future::Future, sync::Arc};

use tokio::sync::Mutex;

use crate::framework::reactive::{SharedSignal, Signal, shared, signal};

pub use crate::framework::reactive::{Resource, ResourceState};

pub struct PaginatedResource<T, E = String>
where
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    pub items: SharedSignal<Vec<T>>,
    pub state: Signal<ResourceState<(), E>>,
    pub page: Signal<usize>,
    pub has_more: Signal<bool>,
    abort: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

impl<T, E> Clone for PaginatedResource<T, E>
where
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            state: self.state.clone(),
            page: self.page.clone(),
            has_more: self.has_more.clone(),
            abort: Arc::clone(&self.abort),
        }
    }
}

impl<T> PaginatedResource<T, String>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            items: shared(Vec::<T>::new()),
            state: signal(ResourceState::<(), String>::Idle),
            page: signal(0usize),
            has_more: signal(true),
            abort: Arc::new(Mutex::new(None)),
        }
    }

    pub fn load_next<F, Fut>(&self, fetcher: F)
    where
        F: FnOnce(usize) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(Vec<T>, bool), String>> + Send + 'static,
    {
        if !self.has_more.get() {
            return;
        }

        let items = self.items.clone();
        let state = self.state.clone();
        let page = self.page.clone();
        let has_more = self.has_more.clone();
        let cur_page = page.get();
        let abort = Arc::clone(&self.abort);

        state.set(ResourceState::Loading);

        let handle = tokio::spawn(async move {
            match fetcher(cur_page).await {
                Ok((new_items, more)) => {
                    items.update(|list| list.extend(new_items));
                    page.set(cur_page + 1);
                    has_more.set(more);
                    state.set(ResourceState::Ready(()));
                }
                Err(e) => {
                    state.set(ResourceState::Error(e));
                }
            }
        });

        tokio::spawn(async move {
            *abort.lock().await = Some(handle.abort_handle());
        });
    }

    pub fn reset(&self) {
        self.items.set(Vec::<T>::new());
        self.state.set(ResourceState::<(), String>::Idle);
        self.page.set(0usize);
        self.has_more.set(true);
    }

    pub fn items(&self) -> &SharedSignal<Vec<T>> {
        &self.items
    }
    pub fn state(&self) -> &Signal<ResourceState<(), String>> {
        &self.state
    }
    pub fn page(&self) -> usize {
        self.page.get()
    }
    pub fn has_more(&self) -> bool {
        self.has_more.get()
    }
}

impl<T> Default for PaginatedResource<T, String>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct ResourceBuilder<T>
where
    T: Clone + Send + Sync + 'static,
{
    initial: ResourceState<T, String>,
}

impl<T: Clone + Send + Sync + 'static> Default for ResourceBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static> ResourceBuilder<T> {
    pub fn new() -> Self {
        Self {
            initial: ResourceState::Idle,
        }
    }

    pub fn with_initial(mut self, value: T) -> Self {
        self.initial = ResourceState::Ready(value);
        self
    }

    pub fn build_idle(self) -> Resource<T, String> {
        let r = Resource::idle();
        if !matches!(self.initial, ResourceState::Idle) {
            r.state.set(self.initial);
        }
        r
    }

    pub fn build_with<F, Fut>(self, fetcher: F) -> Resource<T, String>
    where
        F: Fn() -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<T, String>> + Send + 'static,
    {
        Resource::new(fetcher)
    }
}
