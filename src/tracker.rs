//! Build dependency tracker for incremental builds

use dashmap::DashMap;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use twox_hash::XxHash64;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileState {
    pub hash: u64,
    pub mtime_secs: u64,
    pub mtime_nanos: u32,
}

impl FileState {
    pub fn new(hash: u64, mtime: SystemTime) -> Self {
        let duration = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            hash,
            mtime_secs: duration.as_secs(),
            mtime_nanos: duration.subsec_nanos(),
        }
    }

    pub fn mtime(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::new(self.mtime_secs, self.mtime_nanos)
    }
}

thread_local! {
    static LOCAL_READS: RefCell<Vec<(PathBuf, FileState)>> = RefCell::default();
    static LOCAL_WRITES: RefCell<Vec<(PathBuf, FileState)>> = RefCell::default();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoKey {
    pub function: &'static str,
    pub input_hash: u64,
}

#[derive(Debug)]
pub struct BuildTracker {
    reads: Mutex<HashMap<PathBuf, FileState>>,
    writes: Mutex<HashMap<PathBuf, FileState>>,
    memo: DashMap<MemoKey, Vec<u8>>,
    enabled: bool,
}

impl Default for BuildTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildTracker {
    pub fn new() -> Self {
        Self {
            reads: Mutex::new(HashMap::new()),
            writes: Mutex::new(HashMap::new()),
            memo: DashMap::new(),
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            reads: Mutex::new(HashMap::new()),
            writes: Mutex::new(HashMap::new()),
            memo: DashMap::new(),
            enabled: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn record_read(&self, path: PathBuf, content: &[u8]) {
        if !self.enabled {
            return;
        }
        let hash = hash_content(content);
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        LOCAL_READS.with(|reads| {
            reads.borrow_mut().push((path, FileState::new(hash, mtime)));
        });
    }

    pub fn record_read_with_hash(&self, path: PathBuf, hash: u64, mtime: SystemTime) {
        if !self.enabled {
            return;
        }
        LOCAL_READS.with(|reads| {
            reads.borrow_mut().push((path, FileState::new(hash, mtime)));
        });
    }

    pub fn record_write(&self, path: PathBuf, content: &[u8]) {
        if !self.enabled {
            return;
        }
        let hash = hash_content(content);
        let mtime = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::now());
        LOCAL_WRITES.with(|writes| {
            writes
                .borrow_mut()
                .push((path, FileState::new(hash, mtime)));
        });
    }

    pub fn merge_thread_locals(&self) {
        if !self.enabled {
            return;
        }
        LOCAL_READS.with(|reads| {
            let mut local = reads.borrow_mut();
            if !local.is_empty() {
                let mut main = self.reads.lock();
                for (path, state) in local.drain(..) {
                    main.insert(path, state);
                }
            }
        });
        LOCAL_WRITES.with(|writes| {
            let mut local = writes.borrow_mut();
            if !local.is_empty() {
                let mut main = self.writes.lock();
                for (path, state) in local.drain(..) {
                    main.insert(path, state);
                }
            }
        });
    }

    pub fn merge_all_threads(&self) {
        if !self.enabled {
            return;
        }
        self.merge_thread_locals();
        rayon::broadcast(|_| {
            self.merge_thread_locals();
        });
    }

    pub fn memo_get(&self, function: &'static str, input_hash: u64) -> Option<Vec<u8>> {
        if !self.enabled {
            return None;
        }
        let key = MemoKey {
            function,
            input_hash,
        };
        self.memo.get(&key).map(|v| v.clone())
    }

    pub fn memo_set(&self, function: &'static str, input_hash: u64, output: Vec<u8>) {
        if !self.enabled {
            return;
        }
        let key = MemoKey {
            function,
            input_hash,
        };
        self.memo.insert(key, output);
    }

    pub fn get_reads(&self) -> HashMap<PathBuf, FileState> {
        self.merge_thread_locals();
        self.reads.lock().clone()
    }

    pub fn get_writes(&self) -> HashMap<PathBuf, FileState> {
        self.merge_thread_locals();
        self.writes.lock().clone()
    }

    pub fn clear(&self) {
        LOCAL_READS.with(|r| r.borrow_mut().clear());
        LOCAL_WRITES.with(|w| w.borrow_mut().clear());
        self.reads.lock().clear();
        self.writes.lock().clear();
        self.memo.clear();
    }

    pub fn get_changed_files(&self, cached: &CachedDeps) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        for (path, old_state) in &cached.reads {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    if mtime != old_state.mtime() {
                        if let Ok(content) = std::fs::read(path) {
                            if hash_content(&content) != old_state.hash {
                                changed.push(path.clone());
                            }
                        } else {
                            changed.push(path.clone());
                        }
                    }
                } else {
                    changed.push(path.clone());
                }
            } else {
                changed.push(path.clone());
            }
        }
        changed
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CachedDeps {
    pub reads: HashMap<PathBuf, FileState>,
    pub writes: HashMap<PathBuf, FileState>,
}

impl CachedDeps {
    pub fn from_tracker(tracker: &BuildTracker) -> Self {
        Self {
            reads: tracker.get_reads(),
            writes: tracker.get_writes(),
        }
    }

    pub fn load(path: &std::path::Path) -> Option<Self> {
        let content = std::fs::read(path).ok()?;
        postcard::from_bytes(&content).ok()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let encoded = postcard::to_allocvec(self).map_err(std::io::Error::other)?;
        std::fs::write(path, encoded)
    }
}

pub fn hash_content(content: &[u8]) -> u64 {
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(content);
    hasher.finish()
}

pub fn hash_str(s: &str) -> u64 {
    hash_content(s.as_bytes())
}

pub type SharedTracker = Arc<BuildTracker>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let content = b"hello world";
        let hash1 = hash_content(content);
        let hash2 = hash_content(content);
        assert_eq!(hash1, hash2);

        let different = b"hello world!";
        let hash3 = hash_content(different);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_tracker_read_write() {
        let tracker = BuildTracker::new();

        tracker.record_read(PathBuf::from("test.txt"), b"content");
        tracker.record_write(PathBuf::from("output.txt"), b"output");

        tracker.merge_thread_locals();

        let reads = tracker.get_reads();
        let writes = tracker.get_writes();

        assert_eq!(reads.len(), 1);
        assert_eq!(writes.len(), 1);
    }

    #[test]
    fn test_memo() {
        let tracker = BuildTracker::new();

        tracker.memo_set("render_markdown", 12345, b"cached".to_vec());

        let cached = tracker.memo_get("render_markdown", 12345);
        assert_eq!(cached, Some(b"cached".to_vec()));

        let miss = tracker.memo_get("render_markdown", 99999);
        assert_eq!(miss, None);
    }
}
