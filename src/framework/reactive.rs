use std::{
    collections::{HashMap, HashSet},
    future::Future,
    hash::Hash,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use arc_swap::ArcSwap;
use tokio::task::AbortHandle;

static SIGNAL_COUNTER: AtomicU64 = AtomicU64::new(1);
static EFFECT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SignalId(u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EffectId(u64);

struct EffectEntry {
    run: Arc<dyn Fn() + Send + Sync>,
}

struct RuntimeInner {
    effects: HashMap<EffectId, EffectEntry>,
}

static RUNTIME: std::sync::OnceLock<Mutex<RuntimeInner>> = std::sync::OnceLock::new();

fn runtime() -> &'static Mutex<RuntimeInner> {
    RUNTIME.get_or_init(|| {
        Mutex::new(RuntimeInner {
            effects: HashMap::new(),
        })
    })
}

fn register_effect(id: EffectId, entry: EffectEntry) {
    runtime().lock().unwrap().effects.insert(id, entry);
}

fn effect_run(id: EffectId) -> Option<Arc<dyn Fn() + Send + Sync>> {
    runtime()
        .lock()
        .unwrap()
        .effects
        .get(&id)
        .map(|e| Arc::clone(&e.run))
}

thread_local! {
        static CURRENT_EFFECT: std::cell::Cell<Option<EffectId>> =
        const { std::cell::Cell::new(None) };
        static BATCH_QUEUE: std::cell::RefCell<Vec<EffectId>> =
        const { std::cell::RefCell::new(Vec::new()) };
        static BATCH_DEPTH: std::cell::Cell<u32> =
        const { std::cell::Cell::new(0) };
        static RUNNING: std::cell::RefCell<HashSet<EffectId>> =
        std::cell::RefCell::new(HashSet::new());
}

fn run_effect(id: EffectId) {
    let already = RUNNING.with(|r| !r.borrow_mut().insert(id));
    if already {
        return;
    }
    if let Some(f) = effect_run(id) {
        let prev = CURRENT_EFFECT.with(|c| c.replace(Some(id)));
        f();
        CURRENT_EFFECT.with(|c| c.set(prev));
    }
    RUNNING.with(|r| r.borrow_mut().remove(&id));
}

fn schedule_effect(id: EffectId) {
    if BATCH_DEPTH.with(|d| d.get() > 0) {
        BATCH_QUEUE.with(|q| q.borrow_mut().push(id));
    } else {
        run_effect(id);
    }
}

struct SignalInner<T> {
    value: ArcSwap<T>,
    subscribers: Mutex<Vec<EffectId>>,
    id: SignalId,
}

pub struct Signal<T>(Arc<SignalInner<T>>);

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> std::fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signal")
            .field("id", &self.0.id)
            .field("value", &**self.0.value.load())
            .finish()
    }
}

impl<T: Send + Sync + 'static> Signal<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(SignalInner {
            value: ArcSwap::new(Arc::new(value)),
            subscribers: Mutex::new(Vec::new()),
            id: SignalId(SIGNAL_COUNTER.fetch_add(1, Ordering::Relaxed)),
        }))
    }

    pub fn id(&self) -> SignalId {
        self.0.id
    }

    pub fn track(&self) {
        CURRENT_EFFECT.with(|c| {
            if let Some(eid) = c.get() {
                let mut subs = self.0.subscribers.lock().unwrap();
                if !subs.contains(&eid) {
                    subs.push(eid);
                }
            }
        });
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.track();
        (**self.0.value.load()).clone()
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.track();
        f(&**self.0.value.load())
    }

    pub fn set(&self, value: T) {
        self.0.value.store(Arc::new(value));
        self.notify_subscribers();
    }

    pub fn update(&self, f: impl FnOnce(&mut T))
    where
        T: Clone,
    {
        let current = self.0.value.load();
        let mut val = (**current).clone();
        f(&mut val);
        self.0.value.store(Arc::new(val));
        self.notify_subscribers();
    }

    pub fn set_neq(&self, value: T)
    where
        T: PartialEq,
    {
        if **self.0.value.load() != value {
            self.set(value);
        }
    }

    pub fn read_only(&self) -> ReadSignal<T> {
        ReadSignal(self.clone())
    }

    pub(crate) fn notify_subscribers(&self) {
        let subs = self.0.subscribers.lock().unwrap().clone();
        for id in subs {
            schedule_effect(id);
        }
    }
}

pub struct ReadSignal<T>(Signal<T>);

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Send + Sync + 'static> ReadSignal<T> {
    pub fn from_signal(s: Signal<T>) -> Self {
        Self(s)
    }
    pub fn id(&self) -> SignalId {
        self.0.id()
    }
    pub fn track(&self) {
        self.0.track();
    }
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.0.get()
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.0.with(f)
    }
}

#[derive(Clone)]
pub struct SharedSignal<T: Send + Sync + 'static>(Signal<Arc<T>>);

impl<T: Send + Sync + 'static> SharedSignal<T> {
    pub fn new(value: T) -> Self {
        Self(Signal::new(Arc::new(value)))
    }
    pub fn get(&self) -> Arc<T> {
        self.0.get()
    }
    pub fn set(&self, value: T) {
        self.0.set(Arc::new(value));
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.0.with(|arc| f(arc.as_ref()))
    }
    pub fn update(&self, f: impl FnOnce(&mut T))
    where
        T: Clone,
    {
        let arc = self.0.get();
        let mut val = (*arc).clone();
        f(&mut val);
        self.0.set(Arc::new(val));
    }
}

pub fn shared<T: Send + Sync + 'static>(value: T) -> SharedSignal<T> {
    SharedSignal::new(value)
}

#[derive(Clone)]
pub struct Trigger(Signal<u64>);

impl Trigger {
    pub fn new() -> Self {
        Self(Signal::new(0))
    }
    pub fn notify(&self) {
        self.0.update(|v| *v += 1);
    }
    pub fn track(&self) {
        self.0.track();
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Memo<T>(pub Signal<T>);

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Memo<T> {
    pub fn get(&self) -> T {
        self.0.get()
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.0.with(f)
    }
    pub fn track(&self) {
        self.0.track();
    }
}

#[inline]
pub fn signal<T: Send + Sync + 'static>(value: T) -> Signal<T> {
    Signal::new(value)
}

#[inline]
pub fn trigger() -> Trigger {
    Trigger::new()
}

pub fn memo<T: Clone + PartialEq + Send + Sync + 'static>(
    f: impl Fn(Option<&T>) -> T + Send + Sync + 'static,
) -> Memo<T> {
    let f = Arc::new(f);

    let initial = {
        let prev = CURRENT_EFFECT.with(|c| c.replace(None));
        let v = f(None);
        CURRENT_EFFECT.with(|c| c.set(prev));
        v
    };

    let sig = Signal::new(initial);
    let memo_out = Memo(sig.clone());
    let id = EffectId(EFFECT_COUNTER.fetch_add(1, Ordering::Relaxed));
    let sig2 = sig.clone();
    let f2 = Arc::clone(&f);

    register_effect(
        id,
        EffectEntry {
            run: Arc::new(move || {
                let current = (**sig2.0.value.load()).clone();
                let new_val = f2(Some(&current));
                if new_val != current {
                    sig2.0.value.store(Arc::new(new_val));
                    sig2.notify_subscribers();
                }
            }),
        },
    );
    run_effect(id);

    memo_out
}

pub fn create_effect<T: Send + Sync + 'static>(f: impl Fn(Option<T>) -> T + Send + Sync + 'static) {
    let prev = Arc::new(Mutex::new(Option::<T>::None));
    let f = Arc::new(f);
    let id = EffectId(EFFECT_COUNTER.fetch_add(1, Ordering::Relaxed));
    let prev2 = Arc::clone(&prev);
    let f2 = Arc::clone(&f);

    register_effect(
        id,
        EffectEntry {
            run: Arc::new(move || {
                let mut p = prev2.lock().unwrap();
                let new_val = f2(p.take());
                *p = Some(new_val);
            }),
        },
    );
    run_effect(id);
}

pub fn effect(f: impl Fn() + Send + Sync + 'static) {
    create_effect(move |_: Option<()>| {
        f();
    });
}

pub fn watch<T, F>(source: impl Fn() -> T + Send + Sync + 'static, callback: F)
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T, Option<&T>) + Send + Sync + 'static,
{
    let callback = Arc::new(callback);
    create_effect(move |prev: Option<T>| {
        let new_val = source();
        let saved = CURRENT_EFFECT.with(|c| c.replace(None));
        (callback)(&new_val, prev.as_ref());
        CURRENT_EFFECT.with(|c| c.set(saved));
        new_val
    });
}

pub fn watch_immediate<T, F>(source: impl Fn() -> T + Send + Sync + 'static, callback: F)
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T, Option<&T>) + Send + Sync + 'static,
{
    watch(source, callback);
}

pub fn batch<F: FnOnce() -> R, R>(f: F) -> R {
    BATCH_DEPTH.with(|d| d.set(d.get() + 1));
    let result = f();
    let depth = BATCH_DEPTH.with(|d| {
        let v = d.get();
        d.set(v - 1);
        v
    });
    if depth == 1 {
        let effects = BATCH_QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()));
        let mut seen = HashSet::new();
        for id in effects {
            if seen.insert(id) {
                run_effect(id);
            }
        }
    }
    result
}

pub fn init_reactive_runtime() {
    let _ = runtime();
}

pub struct Get;
impl Get {
    #[inline]
    pub fn get<T: Clone + Send + Sync + 'static>(s: &Signal<T>) -> T {
        s.get()
    }
}

pub struct Set;
impl Set {
    #[inline]
    pub fn set<T: Send + Sync + 'static>(s: &Signal<T>, v: T) {
        s.set(v);
    }
}

pub struct Update;
impl Update {
    #[inline]
    pub fn update<T: Clone + Send + Sync + 'static>(s: &Signal<T>, f: impl FnOnce(&mut T)) {
        s.update(f);
    }
}

pub struct With;
impl With {
    #[inline]
    pub fn with<T: Send + Sync + 'static, R>(s: &Signal<T>, f: impl FnOnce(&T) -> R) -> R {
        s.with(f)
    }
}

pub trait SignalSetNeq<T> {
    fn set_neq(&self, value: T)
    where
        T: PartialEq;
}

impl<T: Send + Sync + 'static> SignalSetNeq<T> for Signal<T> {
    fn set_neq(&self, value: T)
    where
        T: PartialEq,
    {
        Signal::set_neq(self, value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ResourceState<T, E = String> {
    Loading,
    Ready(T),
    Error(E),
    Stale(T),
    #[default]
    Idle,
}

impl<T, E> ResourceState<T, E> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Stale(_))
    }
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Ready(v) | Self::Stale(v) => Some(v),
            _ => None,
        }
    }
    pub fn error(&self) -> Option<&E> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ResourceState<U, E> {
        match self {
            Self::Loading => ResourceState::Loading,
            Self::Ready(v) => ResourceState::Ready(f(v)),
            Self::Error(e) => ResourceState::Error(e),
            Self::Stale(v) => ResourceState::Stale(f(v)),
            Self::Idle => ResourceState::Idle,
        }
    }

    pub fn map_err<F2>(self, f: impl FnOnce(E) -> F2) -> ResourceState<T, F2> {
        match self {
            Self::Loading => ResourceState::Loading,
            Self::Ready(v) => ResourceState::Ready(v),
            Self::Error(e) => ResourceState::Error(f(e)),
            Self::Stale(v) => ResourceState::Stale(v),
            Self::Idle => ResourceState::Idle,
        }
    }
}

impl<T: Clone, E: Clone> ResourceState<T, E> {
    pub fn unwrap_or(&self, default: T) -> T {
        self.value().cloned().unwrap_or(default)
    }
    pub fn unwrap_or_else(&self, f: impl FnOnce() -> T) -> T {
        self.value().cloned().unwrap_or_else(f)
    }
}

pub struct Resource<T, E = String>
where
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    pub state: Signal<ResourceState<T, E>>,
    trigger: Trigger,
    abort: Arc<Mutex<Option<AbortHandle>>>,
}

impl<T, E> Clone for Resource<T, E>
where
    T: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            trigger: self.trigger.clone(),
            abort: Arc::clone(&self.abort),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Resource<T, String> {
    pub fn new<F, Fut>(fetcher: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<T, String>> + Send + 'static,
    {
        let resource = Self::idle();
        let state = resource.state.clone();
        let trigger = resource.trigger.clone();
        let abort = Arc::clone(&resource.abort);

        create_effect(move |_| {
            trigger.track();

            let state2 = state.clone();
            let abort2 = Arc::clone(&abort);

            if let Some(h) = abort.lock().unwrap().take() {
                h.abort();
            }

            state.update(|s| {
                *s = match std::mem::replace(s, ResourceState::Loading) {
                    ResourceState::Ready(v) | ResourceState::Stale(v) => ResourceState::Stale(v),
                    _ => ResourceState::Loading,
                };
            });

            let fut = fetcher();
            let handle = tokio::spawn(async move {
                match fut.await {
                    Ok(v) => state2.set(ResourceState::Ready(v)),
                    Err(e) => state2.set(ResourceState::Error(e)),
                }
            });
            *abort2.lock().unwrap() = Some(handle.abort_handle());
        });

        resource
    }

    pub fn idle() -> Self {
        Self {
            state: Signal::new(ResourceState::Idle),
            trigger: Trigger::new(),
            abort: Arc::new(Mutex::new(None)),
        }
    }

    pub fn ready(value: T) -> Self {
        Self {
            state: Signal::new(ResourceState::Ready(value)),
            trigger: Trigger::new(),
            abort: Arc::new(Mutex::new(None)),
        }
    }

    pub fn refetch(&self) {
        self.trigger.notify();
    }

    pub fn get(&self) -> ResourceState<T, String> {
        self.state.get()
    }
    pub fn with<R>(&self, f: impl FnOnce(&ResourceState<T, String>) -> R) -> R {
        self.state.with(f)
    }
    pub fn is_loading(&self) -> bool {
        self.state.with(|s| s.is_loading())
    }
    pub fn is_ready(&self) -> bool {
        self.state.with(|s| s.is_ready())
    }
    pub fn is_error(&self) -> bool {
        self.state.with(|s| s.is_error())
    }
    pub fn value(&self) -> Option<T> {
        self.state.with(|s| s.value().cloned())
    }

    pub fn set(&self, value: T) {
        self.state.set(ResourceState::Ready(value));
    }
    pub fn set_error(&self, error: String) {
        self.state.set(ResourceState::Error(error));
    }
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        self.state.update(|s| {
            if let ResourceState::Ready(v) = s {
                f(v);
            }
        });
    }
}

pub fn debounced<T: Clone + PartialEq + Send + Sync + 'static>(
    source: Signal<T>,
    delay_ms: u64,
) -> Memo<T> {
    let ds = signal(source.get());
    create_effect({
        let source = source.clone();
        let ds2 = ds.clone();
        move |prev_handle: Option<Option<tokio::task::JoinHandle<()>>>| {
            let value = source.get();
            if let Some(Some(h)) = prev_handle {
                h.abort();
            }
            let ds3 = ds2.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                ds3.set(value);
            });
            Some(handle)
        }
    });
    memo(move |_| ds.get())
}

pub fn throttled<T: Clone + PartialEq + Send + Sync + 'static>(
    source: Signal<T>,
    interval_ms: u64,
) -> Memo<T> {
    let ts = signal(source.get());
    let last = signal(std::time::Instant::now());
    create_effect({
        let source = source.clone();
        let ts2 = ts.clone();
        let last2 = last.clone();
        move |_| {
            let value = source.get();
            let now = std::time::Instant::now();
            let prev = last2.get();
            if now.duration_since(prev).as_millis() >= interval_ms as u128 {
                ts2.set(value);
                last2.set(now);
            }
        }
    });
    memo(move |_| ts.get())
}

pub struct Selector<T: Clone + PartialEq + Send + Sync + 'static> {
    current: Memo<T>,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Selector<T> {
    pub fn new(source: impl Fn() -> T + Send + Sync + 'static) -> Self {
        Self {
            current: memo(move |_| source()),
        }
    }
    pub fn selected(&self, id: &T) -> bool {
        self.current.with(|s| s == id)
    }
}

pub fn create_selector<T>(source: impl Fn() -> T + Send + Sync + Clone + 'static) -> Selector<T>
where
    T: PartialEq + Eq + Clone + Hash + Send + Sync + 'static,
{
    Selector::new(source)
}
