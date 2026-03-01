use std::{
    fmt::{self, Debug, Display},
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone)]
pub struct ComponentId {
    inner: Arc<ComponentIdInner>,
}

struct ComponentIdInner {
    id: u64,
    name: Option<Arc<str>>,
}

impl ComponentId {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self {
            inner: Arc::new(ComponentIdInner {
                id: COUNTER.fetch_add(1, Ordering::Relaxed),
                name: Some(name.into()),
            }),
        }
    }

    pub fn anonymous() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        Self {
            inner: Arc::new(ComponentIdInner {
                id: COUNTER.fetch_add(1, Ordering::Relaxed),
                name: None,
            }),
        }
    }

    pub fn id(&self) -> u64 {
        self.inner.id
    }

    pub fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    pub fn is_anonymous(&self) -> bool {
        self.inner.name.is_none()
    }
}

impl Debug for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.name {
            Some(name) => write!(f, "ComponentId({}, \"{}\")", self.inner.id, name),
            None => write!(f, "ComponentId({})", self.inner.id),
        }
    }
}

impl Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.name {
            Some(name) => write!(f, "{}", name),
            None => write!(f, "component-{}", self.inner.id),
        }
    }
}

impl PartialEq for ComponentId {
    fn eq(&self, other: &Self) -> bool {
        self.inner.id == other.inner.id
    }
}

impl Eq for ComponentId {}

impl Hash for ComponentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.id.hash(state);
    }
}

impl<S: AsRef<str>> From<S> for ComponentId {
    fn from(s: S) -> Self {
        Self::new(s.as_ref().to_owned())
    }
}

pub struct ComponentIdBuilder {
    prefix: String,
}

impl ComponentIdBuilder {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    pub fn id(&self, name: &str) -> ComponentId {
        ComponentId::new(format!("{}:{}", self.prefix, name))
    }
}
