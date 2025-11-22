use std::collections::HashMap;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct TaskManager {
    tasks: HashMap<String, JoinHandle<()>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn spawn(&mut self, key: &str, task: JoinHandle<()>) {
        if let Some(handle) = self.tasks.insert(key.to_string(), task) {
            handle.abort();
        }
    }

    pub fn abort(&mut self, key: &str) {
        if let Some(handle) = self.tasks.remove(key) {
            handle.abort();
        }
    }

    pub fn abort_all(&mut self) {
        for handle in self.tasks.values() {
            handle.abort();
        }
        self.tasks.clear();
    }
}
