use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct SqliteLockManager {
    readonly_set: HashSet<String>,
    write_queue: VecDeque<String>,
    active_write: Option<String>,
}

pub type SharedLockManager = Arc<Mutex<SqliteLockManager>>;

impl SqliteLockManager {
    pub fn new() -> Self {
        Self {
            readonly_set: HashSet::new(),
            write_queue: VecDeque::new(),
            active_write: None,
        }
    }

    pub fn acquire_read(&mut self, path: &str) -> bool {
        if self.active_write.is_some() {
            return false;
        }
        self.readonly_set.insert(path.to_string());
        true
    }

    pub fn release_read(&mut self, path: &str) {
        self.readonly_set.remove(path);
    }

    pub fn enqueue_write(&mut self, path: &str) {
        self.write_queue.push_back(path.to_string());
    }

    pub fn acquire_write(&mut self) -> Option<String> {
        if self.active_write.is_none() && self.readonly_set.is_empty() {
            self.active_write = self.write_queue.pop_front();
        }
        self.active_write.clone()
    }

    pub fn release_write(&mut self) {
        self.active_write = None;
    }
}
