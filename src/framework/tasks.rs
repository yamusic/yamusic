use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupId(Arc<str>);

impl GroupId {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl<S: AsRef<str>> From<S> for GroupId {
    fn from(s: S) -> Self {
        Self::new(s.as_ref().to_owned())
    }
}

#[allow(dead_code)]
struct TaskInfo {
    id: TaskId,
    group: Option<GroupId>,
    description: String,
    started_at: Instant,
    handle: JoinHandle<()>,
}

impl TaskInfo {
    fn new(
        id: TaskId,
        group: Option<GroupId>,
        description: String,
        handle: JoinHandle<()>,
    ) -> Self {
        Self {
            id,
            group,
            description,
            started_at: Instant::now(),
            handle,
        }
    }

    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    pub spawned: u64,
    pub running: usize,
    pub completed: u64,
    pub aborted: u64,
    pub failed: u64,
}

pub struct TaskManager {
    tasks: RwLock<HashMap<TaskId, TaskInfo>>,
    groups: RwLock<HashMap<GroupId, TaskId>>,
    stats: RwLock<TaskStats>,
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            groups: RwLock::new(HashMap::new()),
            stats: RwLock::new(TaskStats::default()),
        }
    }

    pub fn spawn<F>(&self, description: impl Into<String>, future: F) -> TaskId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let id = TaskId::new();
        let handle = tokio::spawn(future);

        let info = TaskInfo::new(id, None, description.into(), handle);

        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(id, info);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.spawned += 1;
            stats.running += 1;
        }

        id
    }

    pub fn spawn_in_group<F>(
        &self,
        group: impl Into<GroupId>,
        description: impl Into<String>,
        future: F,
    ) -> TaskId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let group_id = group.into();
        let task_id = TaskId::new();

        self.abort_group_internal(&group_id);

        let handle = tokio::spawn(future);
        let info = TaskInfo::new(task_id, Some(group_id.clone()), description.into(), handle);

        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task_id, info);
        }

        {
            let mut groups = self.groups.write().unwrap();
            groups.insert(group_id, task_id);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.spawned += 1;
            stats.running += 1;
        }

        task_id
    }

    pub fn track(&self, key: impl Into<GroupId>, handle: JoinHandle<()>) -> TaskId {
        let group_id = key.into();
        let task_id = TaskId::new();

        self.abort_group_internal(&group_id);

        let info = TaskInfo::new(task_id, Some(group_id.clone()), String::new(), handle);

        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task_id, info);
        }

        {
            let mut groups = self.groups.write().unwrap();
            groups.insert(group_id, task_id);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.spawned += 1;
            stats.running += 1;
        }

        task_id
    }

    pub fn abort(&self, id: TaskId) -> bool {
        let info = {
            let mut tasks = self.tasks.write().unwrap();
            tasks.remove(&id)
        };

        if let Some(info) = info {
            info.handle.abort();

            if let Some(group) = &info.group {
                let mut groups = self.groups.write().unwrap();
                if groups.get(group) == Some(&id) {
                    groups.remove(group);
                }
            }

            let mut stats = self.stats.write().unwrap();
            stats.running = stats.running.saturating_sub(1);
            stats.aborted += 1;

            true
        } else {
            false
        }
    }

    pub fn abort_group(&self, group: impl Into<GroupId>) {
        let group_id = group.into();
        self.abort_group_internal(&group_id);
    }

    fn abort_group_internal(&self, group: &GroupId) {
        let task_id = {
            let groups = self.groups.read().unwrap();
            groups.get(group).copied()
        };

        if let Some(id) = task_id {
            self.abort(id);
        }
    }

    pub fn abort_all(&self) {
        let tasks: Vec<_> = {
            let mut tasks = self.tasks.write().unwrap();
            tasks.drain().collect()
        };

        let aborted = tasks.len();

        for (_, info) in tasks {
            info.handle.abort();
        }

        {
            let mut groups = self.groups.write().unwrap();
            groups.clear();
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.running = 0;
            stats.aborted += aborted as u64;
        }
    }

    pub fn is_group_active(&self, group: impl Into<GroupId>) -> bool {
        let group_id = group.into();
        let groups = self.groups.read().unwrap();
        groups.contains_key(&group_id)
    }

    pub fn is_running(&self, id: TaskId) -> bool {
        let tasks = self.tasks.read().unwrap();
        tasks.contains_key(&id)
    }

    pub fn running_count(&self) -> usize {
        let tasks = self.tasks.read().unwrap();
        tasks.len()
    }

    pub fn stats(&self) -> TaskStats {
        let stats = self.stats.read().unwrap();
        TaskStats {
            running: self.running_count(),
            ..stats.clone()
        }
    }

    pub fn running_tasks(&self) -> Vec<(TaskId, String, Duration)> {
        let tasks = self.tasks.read().unwrap();
        tasks
            .iter()
            .map(|(id, info)| (*id, info.description.clone(), info.elapsed()))
            .collect()
    }

    pub fn cleanup(&self) {
        let finished: Vec<TaskId> = {
            let tasks = self.tasks.read().unwrap();
            tasks
                .iter()
                .filter(|(_, info)| info.handle.is_finished())
                .map(|(id, _)| *id)
                .collect()
        };

        let mut stats_update = (0u64, 0u64);

        for id in finished {
            let info = {
                let mut tasks = self.tasks.write().unwrap();
                tasks.remove(&id)
            };

            if let Some(info) = info {
                if let Some(group) = &info.group {
                    let mut groups = self.groups.write().unwrap();
                    if groups.get(group) == Some(&id) {
                        groups.remove(group);
                    }
                }

                stats_update.0 += 1;
            }
        }

        if stats_update.0 > 0 || stats_update.1 > 0 {
            let mut stats = self.stats.write().unwrap();
            stats.running = stats.running.saturating_sub(stats_update.0 as usize);
            stats.completed += stats_update.0;
            stats.failed += stats_update.1;
        }
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        let tasks = self.tasks.get_mut().unwrap();
        for (_, info) in tasks.drain() {
            info.handle.abort();
        }
    }
}

pub struct TaskScope {
    manager: Arc<TaskManager>,
    group_prefix: String,
}

impl TaskScope {
    pub fn new(manager: Arc<TaskManager>, prefix: impl Into<String>) -> Self {
        Self {
            manager,
            group_prefix: prefix.into(),
        }
    }

    pub fn spawn<F>(&self, name: &str, future: F) -> TaskId
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let group = format!("{}:{}", self.group_prefix, name);
        self.manager.spawn_in_group(group, name, future)
    }

    pub fn abort_all(&self) {
        tracing::warn!("TaskScope::abort_all is not fully implemented");
    }
}

pub struct DebouncedTask {
    manager: Arc<TaskManager>,
    group: GroupId,
    delay: Duration,
}

impl DebouncedTask {
    pub fn new(manager: Arc<TaskManager>, group: impl Into<GroupId>, delay: Duration) -> Self {
        Self {
            manager,
            group: group.into(),
            delay,
        }
    }

    pub fn schedule<F, Fut>(&self, task: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let delay = self.delay;

        self.manager.spawn_in_group(
            self.group.clone(),
            format!("debounced:{}", self.group.name()),
            async move {
                tokio::time::sleep(delay).await;
                task().await;
            },
        );
    }

    pub fn cancel(&self) {
        self.manager.abort_group(self.group.clone());
    }
}

pub struct ThrottledTask {
    manager: Arc<TaskManager>,
    group: GroupId,
    interval: Duration,
    last_run: Arc<RwLock<Option<Instant>>>,
}

impl ThrottledTask {
    pub fn new(manager: Arc<TaskManager>, group: impl Into<GroupId>, interval: Duration) -> Self {
        Self {
            manager,
            group: group.into(),
            interval,
            last_run: Arc::new(RwLock::new(None)),
        }
    }

    pub fn try_run<F>(&self, description: &str, future: F) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let now = Instant::now();

        {
            let last = self.last_run.read().unwrap();
            if let Some(last_time) = *last
                && now.duration_since(last_time) < self.interval
            {
                return false;
            }
        }

        {
            let mut last = self.last_run.write().unwrap();
            *last = Some(now);
        }

        self.manager
            .spawn_in_group(self.group.clone(), description, future);
        true
    }
}
