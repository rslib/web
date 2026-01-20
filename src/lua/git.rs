//! Git module (rs.git)

use super::helpers::{is_path_within_root, resolve_path};
use mlua::{Lua, Result, Table, Value};
use std::path::{Path, PathBuf};

pub fn create_module(lua: &Lua, project_root: &Path, sandbox: bool) -> Result<Table> {
    let git = lua.create_table()?;
    let root = project_root.to_path_buf();

    // info(path?) - Get git info for repo, file, or directory
    let root_clone = root.clone();
    let info_fn = lua.create_function(move |lua, path: Option<String>| {
        use git2::Repository;

        let repo = match Repository::discover(&root_clone) {
            Ok(r) => r,
            Err(_) => return Ok(Value::Nil),
        };

        let result = lua.create_table()?;

        if let Some(ref p) = path {
            let resolved = resolve_path(p, &root_clone);
            if sandbox && !is_path_within_root(&resolved, &root_clone) {
                return Err(mlua::Error::RuntimeError(format!(
                    "Sandbox: cannot access '{}' outside project directory.",
                    p
                )));
            }

            let repo_root = repo.workdir().unwrap_or(root_clone.as_path());
            let rel_path = resolved.strip_prefix(repo_root).unwrap_or(&resolved);

            let mut revwalk = match repo.revwalk() {
                Ok(r) => r,
                Err(_) => return Ok(Value::Nil),
            };
            revwalk.push_head().ok();
            revwalk.set_sorting(git2::Sort::TIME).ok();

            for oid in revwalk.flatten() {
                if let Ok(commit) = repo.find_commit(oid) {
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

            if let Some(name) = head.shorthand() {
                result.set("branch", name.to_string())?;
            }
        }

        let statuses = repo.statuses(None).ok();
        let dirty = statuses.map(|s| !s.is_empty()).unwrap_or(false);
        result.set("dirty", dirty)?;

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
