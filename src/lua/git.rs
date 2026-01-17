//! Git functions - git_info

use super::helpers::{is_path_within_root, resolve_path};
use mlua::{Lua, Result, Table, Value};
use std::path::Path;

/// Register git-related Lua functions on the module table
pub fn register(lua: &Lua, module: &Table, project_root: &Path, sandbox: bool) -> Result<()> {
    let root = project_root.to_path_buf();

    // git_info(path?) - Get git info for repo, file, or directory
    let root_clone = root.clone();
    let git_info_fn = lua.create_function(move |lua, path: Option<String>| {
        use git2::Repository;

        let repo = match Repository::discover(&root_clone) {
            Ok(r) => r,
            Err(_) => return Ok(Value::Nil),
        };

        let result = lua.create_table()?;

        // If path is provided, get info for specific file/directory
        if let Some(ref p) = path {
            let resolved = resolve_path(p, &root_clone);
            if sandbox && !is_path_within_root(&resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot access '{}' outside project directory.",
                    p
                )));
            }

            // Get the relative path from repo root
            let repo_root = repo.workdir().unwrap_or(root_clone.as_path());
            let rel_path = resolved.strip_prefix(repo_root).unwrap_or(&resolved);

            // Find last commit that touched this path
            let mut revwalk = match repo.revwalk() {
                Ok(r) => r,
                Err(_) => return Ok(Value::Nil),
            };
            revwalk.push_head().ok();
            revwalk.set_sorting(git2::Sort::TIME).ok();

            for oid in revwalk.flatten() {
                if let Ok(commit) = repo.find_commit(oid) {
                    // Check if this commit touched the path
                    let dominated = if let Ok(parent) = commit.parent(0) {
                        let tree = commit.tree().ok();
                        let parent_tree = parent.tree().ok();
                        if let (Some(t), Some(pt)) = (tree, parent_tree) {
                            let diff = repo.diff_tree_to_tree(Some(&pt), Some(&t), None).ok();
                            diff.map(|d| {
                                d.deltas().any(|delta| {
                                    delta
                                        .new_file()
                                        .path()
                                        .map(|dp| dp.starts_with(rel_path))
                                        .unwrap_or(false)
                                        || delta
                                            .old_file()
                                            .path()
                                            .map(|dp| dp.starts_with(rel_path))
                                            .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false)
                        } else {
                            false
                        }
                    } else {
                        // First commit - check if path exists in tree
                        commit
                            .tree()
                            .ok()
                            .map(|t| t.get_path(rel_path).is_ok())
                            .unwrap_or(false)
                    };

                    if dominated {
                        let hash = commit.id().to_string();
                        result.set("hash", hash.clone())?;
                        result.set("short_hash", &hash[..7.min(hash.len())])?;
                        result.set(
                            "author",
                            commit.author().name().unwrap_or("Unknown").to_string(),
                        )?;
                        if let Some(time) =
                            chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                        {
                            result.set("date", time.format("%Y-%m-%d").to_string())?;
                        }
                        return Ok(Value::Table(result));
                    }
                }
            }
            return Ok(Value::Nil);
        }

        // No path - return repo-level info
        if let Ok(head) = repo.head() {
            if let Some(oid) = head.target() {
                let hash = oid.to_string();
                result.set("hash", hash.clone())?;
                result.set("short_hash", &hash[..7.min(hash.len())])?;

                if let Ok(commit) = repo.find_commit(oid)
                    && let Some(time) = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                {
                    result.set("date", time.format("%Y-%m-%d").to_string())?;
                }
            }

            // Branch name
            if let Some(name) = head.shorthand() {
                result.set("branch", name.to_string())?;
            }
        }

        // Check if repo is dirty
        let statuses = repo.statuses(None).ok();
        let dirty = statuses.map(|s| !s.is_empty()).unwrap_or(false);
        result.set("dirty", dirty)?;

        Ok(Value::Table(result))
    })?;
    module.set("git_info", git_info_fn)?;

    Ok(())
}
