//! Git module (rs.git)

use super::helpers::{is_path_within_root, resolve_path};
use crate::git::{get_file_git_info, get_git_info};
use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

pub fn create_module(lua: &Lua, project_root: &Path, sandbox: bool) -> Result<Table> {
    let git = lua.create_table()?;
    let root = project_root.to_path_buf();

    // info(path?) - Get git info for repo, file, or directory
    let root_clone = root.clone();
    let info_fn = lua.create_function(move |lua, path: Option<String>| {
        let result = lua.create_table()?;

        if let Some(ref p) = path {
            // File/directory-specific git info
            let resolved = resolve_path(p, &root_clone);
            if sandbox && !is_path_within_root(&resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot access '{}' outside project directory.",
                    p
                )));
            }

            let file_info = get_file_git_info(&resolved);

            if let Some(hash) = file_info.hash {
                result.set("hash", hash.clone())?;
                result.set(
                    "short_hash",
                    file_info
                        .short_hash
                        .unwrap_or_else(|| hash[..7.min(hash.len())].to_string()),
                )?;
            } else {
                return Ok(Value::Nil);
            }

            if let Some(author) = file_info.author {
                result.set("author", author)?;
            }

            if let Some(ts) = file_info.commit_timestamp {
                result.set("timestamp", ts)?;
            }

            result.set("dirty", file_info.is_dirty)?;
        } else {
            // Repository-level git info
            let repo_info = get_git_info();

            if let Some(ref hash) = repo_info.hash {
                result.set("hash", hash.as_str())?;
                result.set(
                    "short_hash",
                    repo_info
                        .short_hash
                        .as_deref()
                        .unwrap_or(&hash[..7.min(hash.len())]),
                )?;
            }

            if let Some(ref branch) = repo_info.branch {
                result.set("branch", branch.as_str())?;
            }

            if let Some(ts) = repo_info.commit_timestamp {
                result.set("timestamp", ts)?;
            }

            result.set("dirty", repo_info.is_dirty)?;
        }

        Ok(Value::Table(result))
    })?;
    git.set("info", info_fn)?;

    // is_ignored(path) - Check if path is ignored by .gitignore
    let root_clone = root.clone();
    let is_ignored_fn = lua.create_function(move |_, path: String| {
        let full_path = if Path::new(&path).is_absolute() {
            PathBuf::from(&path)
        } else {
            root_clone.join(&path)
        };

        let mut builder = ignore::gitignore::GitignoreBuilder::new(&root_clone);
        let gitignore_path = root_clone.join(".gitignore");
        if gitignore_path.exists() {
            let _ = builder.add(&gitignore_path);
        }

        match builder.build() {
            Ok(gitignore) => {
                let is_dir = full_path.is_dir();
                Ok(gitignore.matched(&full_path, is_dir).is_ignore())
            }
            Err(_) => Ok(false),
        }
    })?;
    git.set("is_ignored", is_ignored_fn)?;

    Ok(git)
}
