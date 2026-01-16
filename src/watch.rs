//! File watcher for incremental rebuilds

use anyhow::{Context, Result};
use log::{debug, trace, warn};
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
    /// Content file changed - requires full rebuild (Lua controls content)
    Content(PathBuf),
    /// Template file changed - re-render affected pages
    Template(PathBuf),
    /// CSS/style file changed - triggers before_build hook
    Css,
}

/// Aggregated changes from a batch of file events
#[derive(Debug, Default)]
pub struct ChangeSet {
    pub full_rebuild: bool,
    pub rebuild_css: bool,
    pub rebuild_home: bool,
    pub content_files: HashSet<PathBuf>,
    pub template_files: HashSet<PathBuf>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        !self.full_rebuild
            && !self.rebuild_css
            && !self.rebuild_home
            && self.content_files.is_empty()
            && self.template_files.is_empty()
    }

    /// Check if any templates changed
    pub fn has_template_changes(&self) -> bool {
        !self.template_files.is_empty()
    }

    fn add(&mut self, change: ChangeType) {
        match change {
            ChangeType::Config => self.full_rebuild = true,
            ChangeType::Content(path) => {
                let canonical = path.canonicalize().unwrap_or(path);
                self.content_files.insert(canonical);
            }
            ChangeType::Template(path) => {
                let canonical = path.canonicalize().unwrap_or(path);
                self.template_files.insert(canonical);
            }
            ChangeType::Css => self.rebuild_css = true,
        }
    }

    fn optimize(&mut self) {
        // If full rebuild, clear incremental changes
        if self.full_rebuild {
            self.rebuild_css = false;
            self.rebuild_home = false;
            self.content_files.clear();
            self.template_files.clear();
        }
    }
}

/// File watcher for incremental builds
pub struct FileWatcher {
    project_dir: PathBuf,
    output_dir: PathBuf,
    config_path: PathBuf,
    templates_dir: PathBuf,
    rx: Receiver<Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>>,
    _watcher: notify_debouncer_mini::Debouncer<RecommendedWatcher>,
}

impl FileWatcher {
    pub fn new(project_dir: &Path, config: &Config, output_dir: &Path) -> Result<Self> {
        let project_dir = project_dir
            .canonicalize()
            .unwrap_or_else(|_| project_dir.to_path_buf());
        let output_dir = output_dir
            .canonicalize()
            .unwrap_or_else(|_| output_dir.to_path_buf());
        let config_path = project_dir.join("config.lua");
        let templates_dir = project_dir.join(&config.paths.templates);

        let templates_dir = templates_dir.canonicalize().unwrap_or(templates_dir);
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
            trace!("Watching config: {:?}", config_path);
            watcher
                .watch(&config_path, RecursiveMode::NonRecursive)
                .with_context(|| format!("Failed to watch config: {:?}", config_path))?;
        }

        // Watch project directory for content changes (Lua decides what's content)
        trace!("Watching project: {:?}", project_dir);
        watcher
            .watch(&project_dir, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch project: {:?}", project_dir))?;

        debug!("File watcher initialized");
        println!("Watching for changes...");
        println!("  Project:   {:?}", project_dir);
        println!("  Templates: {:?}", templates_dir);

        Ok(Self {
            project_dir,
            output_dir,
            config_path,
            templates_dir,
            rx,
            _watcher: debouncer,
        })
    }

    /// Wait for changes and return aggregated change set
    pub fn wait_for_changes(&self) -> Result<ChangeSet> {
        let mut changes = ChangeSet::default();
        trace!("Waiting for file changes...");

        // Block until we receive events
        match self.rx.recv() {
            Ok(Ok(events)) => {
                trace!("Received {} file events", events.len());
                for event in events {
                    if event.kind == DebouncedEventKind::Any
                        && let Some(change) = self.classify_change(&event.path)
                    {
                        trace!("Classified change: {:?} -> {:?}", event.path, change);
                        changes.add(change);
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Watch error: {:?}", e);
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
                    warn!("Watch error: {:?}", e);
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
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path = path.as_path();

        // Skip events from the output directory (prevents feedback loop)
        if path.starts_with(&self.output_dir) {
            trace!("Skipping output directory path: {:?}", path);
            return None;
        }

        // Skip hidden files and directories
        if path
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            trace!("Skipping hidden path: {:?}", path);
            return None;
        }

        // Config file
        if path == self.config_path {
            return Some(ChangeType::Config);
        }

        // Templates directory
        if path.starts_with(&self.templates_dir) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "html" || ext == "htm" {
                return Some(ChangeType::Template(path.to_path_buf()));
            }
            return None;
        }

        // Any file in project directory
        if path.starts_with(&self.project_dir) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // CSS files trigger before_build hook
            if ext == "css" {
                return Some(ChangeType::Css);
            }

            // Content files (md, html, json, lua) trigger rebuild
            if matches!(ext, "md" | "html" | "htm" | "json" | "lua")
                && let Ok(rel) = path.strip_prefix(&self.project_dir)
            {
                return Some(ChangeType::Content(rel.to_path_buf()));
            }
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

    if !changes.template_files.is_empty() {
        parts.push(format!("{} templates", changes.template_files.len()));
    }

    if changes.rebuild_css {
        parts.push("styles".to_string());
    }

    if changes.rebuild_home {
        parts.push("home".to_string());
    }

    if !changes.content_files.is_empty() {
        parts.push(format!("{} content files", changes.content_files.len()));
    }

    if parts.is_empty() {
        return "no actionable changes".to_string();
    }

    parts.join(", ")
}
