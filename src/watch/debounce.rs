use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct DebounceBatcher {
    pending: HashMap<PathBuf, Instant>,
    debounce_duration: Duration,
}

impl DebounceBatcher {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            pending: HashMap::new(),
            debounce_duration: Duration::from_millis(debounce_ms),
        }
    }

    pub fn add_event(&mut self, path: &Path) {
        self.pending.insert(path.to_path_buf(), Instant::now());
    }

    pub fn drain_ready(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let ready: Vec<PathBuf> = self.pending.iter()
            .filter(|(_, &last_event)| now.duration_since(last_event) >= self.debounce_duration)
            .map(|(path, _)| path.clone())
            .collect();

        for path in &ready {
            self.pending.remove(path);
        }

        ready
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}
