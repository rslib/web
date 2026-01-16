//! Git integration for commit info and file history

use git2::Repository;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use tera::{Function, Value};

struct GitInfo {
    hash: Option<String>,
    short_hash: Option<String>,
    branch: Option<String>,
    commit_date: Option<i64>,
    commit_message: Option<String>,
    is_dirty: bool,
}

static GIT_INFO: OnceLock<GitInfo> = OnceLock::new();

fn get_git_info() -> &'static GitInfo {
    GIT_INFO.get_or_init(|| {
        let repo = match Repository::discover(".") {
            Ok(r) => r,
            Err(_) => {
                return GitInfo {
                    hash: None,
                    short_hash: None,
                    branch: None,
                    commit_date: None,
                    commit_message: None,
                    is_dirty: false,
                };
            }
        };

        let head = repo.head().ok();
        let oid = head.as_ref().and_then(|h| h.target());
        let commit = oid.and_then(|o| repo.find_commit(o).ok());

        let hash = oid.map(|o| o.to_string());
        let short_hash = hash.as_ref().map(|h| h[..7].to_string());

        let branch = head.as_ref().and_then(|h| {
            if h.is_branch() {
                h.shorthand().map(|s| s.to_string())
            } else {
                None
            }
        });

        let commit_date = commit.as_ref().map(|c| c.time().seconds());
        let commit_message = commit
            .as_ref()
            .and_then(|c| c.message().map(|m| m.trim().to_string()));

        let is_dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);

        GitInfo {
            hash,
            short_hash,
            branch,
            commit_date,
            commit_message,
            is_dirty,
        }
    })
}

pub fn make_git_hash() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> {
        Ok(get_git_info()
            .hash
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null))
    }
}

pub fn make_git_short_hash() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> {
        Ok(get_git_info()
            .short_hash
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null))
    }
}

pub fn make_git_branch() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> {
        Ok(get_git_info()
            .branch
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null))
    }
}

pub fn make_git_commit_date() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> {
        Ok(get_git_info()
            .commit_date
            .map(|ts| Value::Number(ts.into()))
            .unwrap_or(Value::Null))
    }
}

pub fn make_git_commit_message() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> {
        Ok(get_git_info()
            .commit_message
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null))
    }
}

pub fn make_git_is_dirty() -> impl Function {
    |_: &HashMap<String, Value>| -> tera::Result<Value> { Ok(Value::Bool(get_git_info().is_dirty)) }
}

pub fn register_git_functions(tera: &mut tera::Tera) {
    tera.register_function("git_hash", make_git_hash());
    tera.register_function("git_short_hash", make_git_short_hash());
    tera.register_function("git_branch", make_git_branch());
    tera.register_function("git_commit_date", make_git_commit_date());
    tera.register_function("git_commit_message", make_git_commit_message());
    tera.register_function("git_is_dirty", make_git_is_dirty());
}

/// Git info for a specific file
#[derive(Debug, Clone, Default)]
pub struct FileGitInfo {
    pub hash: Option<String>,
    pub short_hash: Option<String>,
    pub commit_date: Option<i64>,
    pub author: Option<String>,
    pub is_dirty: bool,
}

/// Get git info for a specific file or directory (last commit that modified it)
/// For directories, finds the most recent commit that modified any file within
pub fn get_file_git_info(path: &Path) -> FileGitInfo {
    let repo = match Repository::discover(".") {
        Ok(r) => r,
        Err(_) => return FileGitInfo::default(),
    };

    // Get the relative path from repo root
    let workdir = match repo.workdir() {
        Some(w) => w,
        None => return FileGitInfo::default(),
    };

    let relative_path = match path.strip_prefix(workdir) {
        Ok(p) => p,
        Err(_) => path,
    };

    let is_directory = path.is_dir();

    // Check if file/directory has uncommitted changes
    let is_dirty = if is_directory {
        // For directories, check if any file within has changes
        repo.statuses(None)
            .map(|statuses| {
                statuses.iter().any(|s| {
                    s.path()
                        .map(|p| Path::new(p).starts_with(relative_path))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    } else {
        repo.status_file(relative_path)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    };

    // Use git log to find the last commit that modified this file/directory
    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return FileGitInfo::default(),
    };

    if revwalk.push_head().is_err() {
        return FileGitInfo::default();
    }

    for oid in revwalk.flatten() {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Check if this commit modified our file/directory
        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Get parent tree (if exists)
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let path_changed = if is_directory {
            // For directories, check if any file within the directory was changed
            diff.deltas().any(|delta| {
                delta
                    .new_file()
                    .path()
                    .map(|p| p.starts_with(relative_path))
                    .unwrap_or(false)
                    || delta
                        .old_file()
                        .path()
                        .map(|p| p.starts_with(relative_path))
                        .unwrap_or(false)
            })
        } else {
            // For files, exact match
            diff.deltas().any(|delta| {
                delta
                    .new_file()
                    .path()
                    .map(|p| p == relative_path)
                    .unwrap_or(false)
                    || delta
                        .old_file()
                        .path()
                        .map(|p| p == relative_path)
                        .unwrap_or(false)
            })
        };

        if path_changed {
            let hash = oid.to_string();
            let short_hash = hash[..7].to_string();
            let commit_date = commit.time().seconds();
            let author = commit.author().name().map(|s| s.to_string());

            return FileGitInfo {
                hash: Some(hash),
                short_hash: Some(short_hash),
                commit_date: Some(commit_date),
                author,
                is_dirty,
            };
        }
    }

    FileGitInfo {
        is_dirty,
        ..Default::default()
    }
}
