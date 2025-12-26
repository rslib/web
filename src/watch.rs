use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crate::config::Config;

/// Types of changes that trigger different rebuild strategies
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChangeType {
    /// Config file changed - requires full rebuild
    Config,
    /// Content file changed (markdown or HTML in content dir)
    Content(PathBuf),
    /// Template file changed - re-render all posts
    Template,
    /// CSS/style file changed - only rebuild CSS
    Css,
    /// Static file changed (non-image) - copy that file
    StaticFile(PathBuf),
    /// Image file changed - optimize and copy that image
    Image(PathBuf),
    /// Home page changed
    Home,
}

/// Aggregated changes from a batch of file events
#[derive(Debug, Default)]
pub struct ChangeSet {
    pub full_rebuild: bool,
    pub rebuild_css: bool,
    pub reload_templates: bool,
    pub rebuild_home: bool,
    pub content_files: HashSet<PathBuf>,
    pub static_files: HashSet<PathBuf>,
    pub image_files: HashSet<PathBuf>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        !self.full_rebuild
            && !self.rebuild_css
            && !self.reload_templates
            && !self.rebuild_home
            && self.content_files.is_empty()
            && self.static_files.is_empty()
            && self.image_files.is_empty()
    }

    pub fn add(&mut self, change: ChangeType) {
        match change {
            ChangeType::Config => self.full_rebuild = true,
            ChangeType::Css => self.rebuild_css = true,
            ChangeType::Template => self.reload_templates = true,
            ChangeType::Home => self.rebuild_home = true,
            ChangeType::Content(path) => {
                self.content_files.insert(path);
            }
            ChangeType::StaticFile(path) => {
                self.static_files.insert(path);
            }
            ChangeType::Image(path) => {
                self.image_files.insert(path);
            }
        }
    }

    /// Optimize the change set - if full rebuild is needed, clear everything else
    pub fn optimize(&mut self) {
        if self.full_rebuild {
            self.rebuild_css = false;
            self.reload_templates = false;
            self.rebuild_home = false;
            self.content_files.clear();
            self.static_files.clear();
            self.image_files.clear();
        }
        // If templates changed, we need to re-render all content anyway
        if self.reload_templates {
            self.content_files.clear();
            self.rebuild_home = true;
        }
    }
}

/// File watcher for incremental builds
pub struct FileWatcher {
    project_dir: PathBuf,
    output_dir: PathBuf,
    config_path: PathBuf,
    content_dir: PathBuf,
    templates_dir: PathBuf,
    styles_dir: PathBuf,
    static_dir: PathBuf,
    home_path: PathBuf,
    rx: Receiver<Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>>,
    _watcher: notify_debouncer_mini::Debouncer<RecommendedWatcher>,
}

impl FileWatcher {
    pub fn new(project_dir: &Path, config: &Config, output_dir: &Path) -> Result<Self> {
        // Canonicalize project_dir to ensure consistent path matching
        let project_dir = project_dir
            .canonicalize()
            .unwrap_or_else(|_| project_dir.to_path_buf());

        // Canonicalize output_dir to skip it during watch
        let output_dir = output_dir
            .canonicalize()
            .unwrap_or_else(|_| output_dir.to_path_buf());

        // Resolve all watched paths (canonicalize for consistent matching)
        let config_path = project_dir.join("config.toml");
        let content_dir = project_dir.join(&config.paths.content);
        let templates_dir = project_dir.join(&config.paths.templates);
        let styles_dir = project_dir.join(&config.paths.styles);
        let static_dir = project_dir.join(&config.paths.static_files);
        let home_path = content_dir.join(&config.paths.home);

        // Canonicalize watched directories if they exist
        let content_dir = content_dir.canonicalize().unwrap_or(content_dir);
        let templates_dir = templates_dir.canonicalize().unwrap_or(templates_dir);
        let styles_dir = styles_dir.canonicalize().unwrap_or(styles_dir);
        let static_dir = static_dir.canonicalize().unwrap_or(static_dir);
        let home_path = home_path.canonicalize().unwrap_or(home_path);
        let config_path = config_path.canonicalize().unwrap_or(config_path);

        // Create channel for events
        let (tx, rx) = mpsc::channel();

        // Create debounced watcher (300ms debounce)
        let mut debouncer = new_debouncer(Duration::from_millis(300), tx)
            .context("Failed to create file watcher")?;

        // Watch all relevant directories
        let watcher = debouncer.watcher();

        // Watch config file
        if config_path.exists() {
            watcher
                .watch(&config_path, RecursiveMode::NonRecursive)
                .with_context(|| format!("Failed to watch config: {:?}", config_path))?;
        }

        // Watch content directory
        if content_dir.exists() {
            watcher
                .watch(&content_dir, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch content: {:?}", content_dir))?;
        }

        // Watch templates directory
        if templates_dir.exists() {
            watcher
                .watch(&templates_dir, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch templates: {:?}", templates_dir))?;
        }

        // Watch styles directory
        if styles_dir.exists() {
            watcher
                .watch(&styles_dir, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch styles: {:?}", styles_dir))?;
        }

        // Watch static directory
        if static_dir.exists() {
            watcher
                .watch(&static_dir, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch static: {:?}", static_dir))?;
        }

        println!("Watching for changes...");
        println!("  Content:   {:?}", content_dir);
        println!("  Templates: {:?}", templates_dir);
        println!("  Styles:    {:?}", styles_dir);
        println!("  Static:    {:?}", static_dir);

        Ok(Self {
            project_dir,
            output_dir,
            config_path,
            content_dir,
            templates_dir,
            styles_dir,
            static_dir,
            home_path,
            rx,
            _watcher: debouncer,
        })
    }

    /// Wait for changes and return aggregated change set
    pub fn wait_for_changes(&self) -> Result<ChangeSet> {
        let mut changes = ChangeSet::default();

        // Block until we receive events
        match self.rx.recv() {
            Ok(Ok(events)) => {
                for event in events {
                    if event.kind == DebouncedEventKind::Any
                        && let Some(change) = self.classify_change(&event.path)
                    {
                        changes.add(change);
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("Watch error: {:?}", e);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Watch channel closed: {:?}", e));
            }
        }

        // Drain any additional pending events (with short timeout)
        let drain_start = Instant::now();
        while drain_start.elapsed() < Duration::from_millis(50) {
            match self.rx.try_recv() {
                Ok(Ok(events)) => {
                    for event in events {
                        if event.kind == DebouncedEventKind::Any
                            && let Some(change) = self.classify_change(&event.path)
                        {
                            changes.add(change);
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Watch error: {:?}", e);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        changes.optimize();
        Ok(changes)
    }

    /// Classify a file path into a change type
    fn classify_change(&self, path: &Path) -> Option<ChangeType> {
        // Canonicalize the event path for consistent comparison
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path = path.as_path();

        // Skip events from the output directory (prevents feedback loop)
        if path.starts_with(&self.output_dir) {
            return None;
        }

        // Skip hidden files and directories
        if path
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            return None;
        }

        // Config file
        if path == self.config_path {
            return Some(ChangeType::Config);
        }

        // Home page
        if path == self.home_path {
            return Some(ChangeType::Home);
        }

        // Check specific directories FIRST (before content, which may be a parent)

        // Styles directory
        if path.starts_with(&self.styles_dir) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "css" {
                return Some(ChangeType::Css);
            }
            return None;
        }

        // Templates directory
        if path.starts_with(&self.templates_dir) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "html" || ext == "htm" {
                return Some(ChangeType::Template);
            }
            return None;
        }

        // Static directory
        if path.starts_with(&self.static_dir) {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if let Ok(rel) = path.strip_prefix(&self.static_dir) {
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    return Some(ChangeType::Image(rel.to_path_buf()));
                }
                return Some(ChangeType::StaticFile(rel.to_path_buf()));
            }
            return None;
        }

        // Content directory (check last as it may be a parent of other dirs)
        if path.starts_with(&self.content_dir) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if (ext == "md" || ext == "html" || ext == "htm")
                && let Ok(rel) = path.strip_prefix(&self.content_dir)
            {
                return Some(ChangeType::Content(rel.to_path_buf()));
            }
            return None;
        }

        None
    }

    /// Get the project directory
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }
}

/// Format a change set for display
pub fn format_changes(changes: &ChangeSet) -> String {
    let mut parts = Vec::new();

    if changes.full_rebuild {
        return "config changed (full rebuild)".to_string();
    }

    if changes.reload_templates {
        parts.push("templates".to_string());
    }

    if changes.rebuild_css {
        parts.push("css".to_string());
    }

    if changes.rebuild_home {
        parts.push("home".to_string());
    }

    if !changes.content_files.is_empty() {
        let count = changes.content_files.len();
        if count == 1 {
            let path = changes.content_files.iter().next().unwrap();
            parts.push(format!("content: {}", path.display()));
        } else {
            parts.push(format!("{} content files", count));
        }
    }

    if !changes.static_files.is_empty() {
        parts.push(format!("{} static files", changes.static_files.len()));
    }

    if !changes.image_files.is_empty() {
        parts.push(format!("{} images", changes.image_files.len()));
    }

    if parts.is_empty() {
        "no relevant changes".to_string()
    } else {
        parts.join(", ")
    }
}
