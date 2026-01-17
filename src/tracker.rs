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

/// Asset reference extracted from HTML (script src, link href, img src, etc.)
#[derive(Debug, Clone)]
pub struct AssetRef {
    /// The URL path as it appears in HTML (e.g., "/js/editor.js")
    pub url_path: String,
    /// The source file path if known (e.g., "static/js/editor.js")
    pub source_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct BuildTracker {
    reads: Mutex<HashMap<PathBuf, FileState>>,
    writes: Mutex<HashMap<PathBuf, FileState>>,
    /// Maps output page path -> asset references found in its HTML
    html_refs: Mutex<HashMap<PathBuf, Vec<AssetRef>>>,
    /// Maps source asset path -> output pages that reference it
    asset_to_pages: Mutex<HashMap<PathBuf, Vec<PathBuf>>>,
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
            html_refs: Mutex::new(HashMap::new()),
            asset_to_pages: Mutex::new(HashMap::new()),
            memo: DashMap::new(),
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            reads: Mutex::new(HashMap::new()),
            writes: Mutex::new(HashMap::new()),
            html_refs: Mutex::new(HashMap::new()),
            asset_to_pages: Mutex::new(HashMap::new()),
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
        self.html_refs.lock().clear();
        self.asset_to_pages.lock().clear();
        self.memo.clear();
    }

    /// Record HTML asset references for a rendered page
    pub fn record_html_refs(&self, page_path: PathBuf, refs: Vec<AssetRef>) {
        if !self.enabled || refs.is_empty() {
            return;
        }
        let mut html_refs = self.html_refs.lock();
        let mut asset_to_pages = self.asset_to_pages.lock();

        // Build reverse mapping
        for asset_ref in &refs {
            if let Some(ref source) = asset_ref.source_path {
                asset_to_pages
                    .entry(source.clone())
                    .or_default()
                    .push(page_path.clone());
            }
        }

        html_refs.insert(page_path, refs);
    }

    /// Get pages that reference a given asset (by source path)
    pub fn get_pages_for_asset(&self, asset_path: &PathBuf) -> Vec<PathBuf> {
        self.asset_to_pages
            .lock()
            .get(asset_path)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if an asset is referenced by any page
    pub fn is_asset_referenced(&self, asset_path: &PathBuf) -> bool {
        self.asset_to_pages.lock().contains_key(asset_path)
    }

    /// Get all HTML refs
    pub fn get_html_refs(&self) -> HashMap<PathBuf, Vec<AssetRef>> {
        self.html_refs.lock().clone()
    }

    /// Get reverse mapping of assets to pages
    pub fn get_asset_to_pages(&self) -> HashMap<PathBuf, Vec<PathBuf>> {
        self.asset_to_pages.lock().clone()
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
    /// Maps source asset path -> output pages that reference it
    #[serde(default)]
    pub asset_to_pages: HashMap<PathBuf, Vec<PathBuf>>,
}

impl CachedDeps {
    pub fn from_tracker(tracker: &BuildTracker) -> Self {
        Self {
            reads: tracker.get_reads(),
            writes: tracker.get_writes(),
            asset_to_pages: tracker.get_asset_to_pages(),
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

/// Extract asset references from HTML content.
/// Looks for: script src, link href, img src, video src, audio src, source src
pub fn extract_html_asset_refs(html: &str) -> Vec<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        vec![
            // <script src="...">
            Regex::new(r#"<script[^>]+src=["']([^"']+)["']"#).unwrap(),
            // <link href="..."> (for CSS)
            Regex::new(r#"<link[^>]+href=["']([^"']+)["']"#).unwrap(),
            // <img src="...">
            Regex::new(r#"<img[^>]+src=["']([^"']+)["']"#).unwrap(),
            // <video src="...">
            Regex::new(r#"<video[^>]+src=["']([^"']+)["']"#).unwrap(),
            // <audio src="...">
            Regex::new(r#"<audio[^>]+src=["']([^"']+)["']"#).unwrap(),
            // <source src="...">
            Regex::new(r#"<source[^>]+src=["']([^"']+)["']"#).unwrap(),
            // CSS url(...)
            Regex::new(r#"url\(["']?([^"')]+)["']?\)"#).unwrap(),
        ]
    });

    let mut refs = Vec::new();
    for pattern in PATTERNS.iter() {
        for cap in pattern.captures_iter(html) {
            if let Some(path) = cap.get(1) {
                let path = path.as_str();
                // Only include local paths (starting with / but not //)
                if path.starts_with('/') && !path.starts_with("//") {
                    refs.push(path.to_string());
                }
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

/// Extract image references from markdown content
/// Looks for: ![alt](url) and ![alt](url "title")
pub fn extract_markdown_asset_refs(markdown: &str) -> Vec<String> {
    use regex::Regex;
    use std::sync::LazyLock;

    static IMG_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        // ![alt](url) or ![alt](url "title")
        Regex::new(r#"!\[[^\]]*\]\(([^)"'\s]+)"#).unwrap()
    });

    let mut refs = Vec::new();
    for cap in IMG_PATTERN.captures_iter(markdown) {
        if let Some(path) = cap.get(1) {
            let path = path.as_str();
            // Only include local paths
            if path.starts_with('/') && !path.starts_with("//") {
                refs.push(path.to_string());
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

/// Map a URL path (e.g., "/js/editor.js") to a source path (e.g., "static/js/editor.js")
/// Uses the writes map to find what source file produced the output
pub fn resolve_url_to_source(
    url_path: &str,
    output_dir: &std::path::Path,
    writes: &HashMap<PathBuf, FileState>,
    project_dir: &std::path::Path,
) -> Option<PathBuf> {
    // Convert URL path to output file path
    // e.g., "/js/editor.js" -> "{output_dir}/js/editor.js"
    let url_path = url_path.trim_start_matches('/');
    let output_path = output_dir.join(url_path);
    let output_canonical = output_path.canonicalize().ok()?;

    // Check if this output was written during the build
    if writes.contains_key(&output_canonical) {
        // Try common source mappings:
        // 1. static/{path} -> {output_dir}/{path}
        // 2. {path} -> {output_dir}/{path}
        let candidates = [
            project_dir.join("static").join(url_path),
            project_dir.join(url_path),
        ];

        for candidate in candidates {
            if candidate.exists() {
                return candidate.canonicalize().ok();
            }
        }
    }

    None
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
