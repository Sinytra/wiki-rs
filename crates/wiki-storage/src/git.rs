use std::collections::HashMap;
use std::path::Path;

use git2::build::RepoBuilder;
use git2::{BranchType, FetchOptions, RemoteCallbacks, Repository};
use serde::{Deserialize, Serialize};
use wiki_domain::error::ProjectError;
use wiki_domain::response::GitRevision;
use crate::error::{StorageError, StorageResult};

const MAX_REPO_SIZE_BYTES: u64 = 500 * 1024 * 1024;

pub struct GitProvider {
    pub file_path: &'static str,
    pub commit_path: &'static str,
}

pub fn get_git_provider(url: &str) -> Option<&'static GitProvider> {
    static GITHUB: GitProvider = GitProvider {
        file_path: "blob/{branch}/{base}/{path}",
        commit_path: "commit/{hash}",
    };

    if url.contains("github.com") {
        Some(&GITHUB)
    } else {
        None
    }
}

pub fn format_commit_url(repo_url: &str, hash: &str) -> Option<String> {
    let provider = get_git_provider(repo_url)?;
    let base = repo_url.trim_end_matches('/');
    let path = provider.commit_path.replace("{hash}", hash);
    Some(format!("{base}/{path}"))
}

pub fn format_edit_url(
    repo_url: &str,
    branch: &str,
    base_path: &str,
    file_path: &str,
) -> Option<String> {
    let provider = get_git_provider(repo_url)?;
    let base = repo_url.trim_end_matches('/');
    let cleaned_base = base_path.trim_start_matches('/');
    let cleaned_file = file_path.trim_end_matches('/');
    let path = provider
        .file_path
        .replace("{branch}", branch)
        .replace("{base}", cleaned_base)
        .replace("{path}", cleaned_file);
    Some(format!("{base}/{path}"))
}

fn is_local_url(url: &str) -> bool {
    url.starts_with("file://") || url.starts_with('/')
}

pub async fn clone_repository(
    url: &str,
    dest: &Path,
    branch: &str,
) -> StorageResult<Repository> {
    let url = url.to_owned();
    let dest = dest.to_owned();
    let branch = branch.to_owned();

    tokio::task::spawn_blocking(move || clone_repository_sync(&url, &dest, &branch))
        .await
        .map_err(|e| StorageError::Internal(format!("clone task panicked: {e}")))?
}

fn clone_repository_sync(url: &str, dest: &Path, branch: &str) -> StorageResult<Repository> {
    tracing::info!(url = %url, "Cloning git repository");

    let shallow = !is_local_url(url);

    let mut callbacks = RemoteCallbacks::new();
    callbacks.transfer_progress(|progress| {
        let received = progress.received_bytes() as u64;
        if received > MAX_REPO_SIZE_BYTES {
            tracing::error!("Repository exceeded size limit ({received} bytes)");
            return false;
        }
        true
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks);
    if shallow {
        fo.depth(1);
    }

    let repo = RepoBuilder::new()
        .branch(branch)
        .fetch_options(fo)
        .clone(url, dest)
        .map_err(|e| classify_clone_error(e))?;

    tracing::info!("Git clone successful");
    Ok(repo)
}

fn classify_clone_error(err: git2::Error) -> StorageError {
    let msg = err.message().to_lowercase();

    if msg.contains("not found") || msg.contains("does not exist") {
        return StorageError::project(ProjectError::NoRepository, "Repository not found.");
    }

    if msg.contains("authentication")
        || msg.contains("401")
        || msg.contains("403")
        || msg.contains("credentials")
    {
        return StorageError::project(
            ProjectError::RequiresAuth,
            "Authentication required.",
        );
    }

    if msg.contains("remote branch")
        || msg.contains("not found in upstream")
        || msg.contains("reference")
    {
        return StorageError::project(
            ProjectError::NoBranch,
            "Requested branch not found.",
        );
    }

    if msg.contains("pack exceeds") || msg.contains("early eof") || msg.contains("out of memory") {
        return StorageError::project(
            ProjectError::RepoTooLarge,
            "Repository clone failed (network/size).",
        );
    }

    StorageError::Git(err)
}

pub fn get_latest_revision(repo: &Repository) -> StorageResult<GitRevision> {
    let head = repo.head().map_err(|e| {
        StorageError::Internal(format!("failed to get HEAD: {e}"))
    })?;

    let commit = head.peel_to_commit().map_err(|e| {
        StorageError::Internal(format!("failed to peel HEAD to commit: {e}"))
    })?;

    let oid = commit.id();
    let full_hash = oid.to_string();
    let hash = full_hash[..7.min(full_hash.len())].to_owned();
    let message = commit.summary().unwrap_or("").to_owned();
    let author = commit.author();
    let author_name = author.name().unwrap_or("").to_owned();
    let author_email = author.email().unwrap_or("").to_owned();

    let time = commit.time();
    let date = format_iso_time(time.seconds());

    Ok(GitRevision {
        hash,
        full_hash,
        message,
        author_name,
        author_email,
        date,
    })
}

pub fn list_branches(repo: &Repository) -> StorageResult<HashMap<String, String>> {
    let mut branches = HashMap::new();

    let branch_iter = repo.branches(Some(BranchType::Remote))?;
    for item in branch_iter {
        let (branch, _) = item?;
        if let Some(name) = branch.name()? {
            // Remote branch names look like "origin/main"
            if let Some(short) = name.strip_prefix("origin/") {
                if short == "HEAD" {
                    continue;
                }
                let reference = branch
                    .get()
                    .name()
                    .unwrap_or("")
                    .to_owned();
                branches.insert(short.to_owned(), reference);
            }
        }
    }

    Ok(branches)
}

pub fn checkout_branch(repo: &Repository, refname: &str) -> StorageResult<()> {
    let obj = repo
        .revparse_single(refname)
        .map_err(|e| StorageError::Internal(format!("failed to parse ref '{refname}': {e}")))?;

    repo.checkout_tree(&obj, Some(git2::build::CheckoutBuilder::new().force()))
        .map_err(|e| StorageError::Internal(format!("failed to checkout '{refname}': {e}")))?;

    if let Some(name) = refname.strip_prefix("refs/remotes/origin/") {
        let head_ref = format!("refs/heads/{name}");
        repo.set_head(&head_ref).ok();
    } else {
        repo.set_head(refname).ok();
    }

    Ok(())
}

fn format_iso_time(seconds: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp(seconds, 0).unwrap_or_default();
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
