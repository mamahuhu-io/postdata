use super::result::ResultExt;
use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{
    AutotagOption, BranchType, Cred, DiffDelta, DiffOptions, Error, FetchOptions, IndexAddOption,
    Oid, PushOptions, RemoteCallbacks, Repository, Signature, StatusOptions, Time,
};
use git2::{Direction, FetchPrune, Remote};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_plugin_async_wrapper::async_wrapper;

const REFS_HEADS: &str = "refs/heads/";
const REFS_REMOTES: &str = "refs/remotes/";

#[derive(Debug)]
pub struct CurrentBranchInfo {
    pub commit_id: git2::Oid,
    pub remote_name: String,
    pub branch_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullUpdateInfo {
    pub new_commit_id: String,
    pub old_commit_id: String,
    pub num_commits: usize,
    pub file_changes: HashMap<String, (usize, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub branch_name: String,
    pub branch_type: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub full_id: String,
    pub short_id: String,
    pub summary: String,
    pub time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitOperationStatus {
    Success,
    BranchNotExistsRemote,
    UncommittedChanges,
    UnPushedChanges,
    AlreadyUpToDate,
    NoRemoteConfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationResult<T> {
    pub status: GitOperationStatus,
    pub result: T,
}

impl<T> GitOperationResult<T> {
    fn new(status: GitOperationStatus, result: T) -> Self {
        GitOperationResult { result, status }
    }
}

fn get_repo(repo_path: String) -> Result<Repository, String> {
    Repository::open(repo_path).to_string_err()
}

fn create_signature(repo: &Repository) -> Result<Signature, Error> {
    let config = repo.config()?;
    let name = config.get_string("user.name")?;
    let email = config.get_string("user.email")?;
    Signature::now(&name, &email)
}

fn create_callbacks() -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
        } else if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            // TODO use credential helper
            Cred::userpass_plaintext("your_username", "your_password")
        } else {
            Err(git2::Error::from_str(
                "No valid authentication method available",
            ))
        }
    });

    callbacks
}

fn create_fetch_options() -> FetchOptions<'static> {
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(create_callbacks());
    fetch_options.prune(FetchPrune::On);
    fetch_options.download_tags(AutotagOption::All);
    fetch_options
}

fn get_current_branch_info(repo: &Repository) -> Result<CurrentBranchInfo, Error> {
    let head = repo.head()?;
    let branch_name = head.shorthand().unwrap_or_default().to_string();
    let config = repo.config()?;

    let remote_name = config.get_string(&format_branch_remote(&branch_name))?;

    let commit_id = head
        .target()
        .ok_or_else(|| git2::Error::from_str("Failed to get HEAD commit ID"))?;

    Ok(CurrentBranchInfo {
        commit_id: commit_id,
        remote_name: remote_name.to_string(),
        branch_name: branch_name.to_string(),
    })
}

fn git_add(repo: &Repository) -> Result<Oid, String> {
    let mut index = repo.index().to_string_err()?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .to_string_err()?;

    index.write().to_string_err()?;
    let oid = index.write_tree().to_string_err()?;

    Ok(oid)
}

#[async_wrapper]
pub fn git_init(repo_path: String) -> Result<(), String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err(format!("Path '{}' does not exist.", repo_path));
    }

    match Repository::init(repo_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to initialize repository: {}", e)),
    }
}

#[async_wrapper]
pub fn git_push(repo_path: String) -> Result<GitOperationStatus, String> {
    let repo = get_repo(repo_path.to_string())?;
    let branch_info = get_current_branch_info(&repo).to_string_err()?;

    if branch_info.remote_name.is_empty() {
        return Ok(GitOperationStatus::NoRemoteConfigured);
    }

    if !has_unpushed_commits_sync(repo_path)? {
        return Ok(GitOperationStatus::UnPushedChanges);
    }

    let mut remote = repo.find_remote(&branch_info.remote_name).to_string_err()?;

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(create_callbacks());

    let refspec = format_refs_heads(&branch_info.branch_name);
    remote
        .push(&[&refspec], Some(&mut push_options))
        .to_string_err()?;

    Ok(GitOperationStatus::Success)
}

#[async_wrapper]
pub fn git_pull(repo_path: String) -> Result<GitOperationResult<Option<PullUpdateInfo>>, String> {
    let repo = get_repo(repo_path.to_string())?;

    let branch_info = get_current_branch_info(&repo).to_string_err()?;

    {
        let mut fetch_options = create_fetch_options();

        let mut remote = repo.find_remote(&branch_info.remote_name).to_string_err()?;
        remote
            .fetch(
                &[format_ref_specs(&branch_info.remote_name, false)],
                Some(&mut fetch_options),
                None,
            )
            .to_string_err()?;

        let (has_remote, remote_commit_oid) =
            has_current_branch_exists_remote(&mut remote, &*branch_info.branch_name)?;
        if !has_remote {
            return Ok(GitOperationResult::new(
                GitOperationStatus::BranchNotExistsRemote,
                None,
            ));
        }
        if remote_commit_oid == branch_info.commit_id {
            return Ok(GitOperationResult::new(
                GitOperationStatus::AlreadyUpToDate,
                None,
            ));
        }
    }

    let fetch_head = repo.find_reference("FETCH_HEAD").to_string_err()?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .to_string_err()?;

    let (analysis, _) = repo.merge_analysis(&[&fetch_commit]).to_string_err()?;
    if analysis.is_up_to_date() {
        return Ok(GitOperationResult::new(
            GitOperationStatus::AlreadyUpToDate,
            None,
        ));
    }

    let mut new_commit_id = Oid::zero();
    if analysis.is_fast_forward() {
        let refname = format_refs_heads(&branch_info.branch_name);
        let mut reference = repo.find_reference(&refname).to_string_err()?;
        reference
            .set_target(fetch_commit.id(), "Fast-forward")
            .to_string_err()?;
        repo.set_head(&refname).to_string_err()?;
        repo.checkout_head(Some(CheckoutBuilder::default().force()))
            .to_string_err()?;
        new_commit_id = fetch_commit.id();
    } else {
        let head_commit = repo
            .head()
            .to_string_err()?
            .peel_to_commit()
            .to_string_err()?;
        let mut index = repo
            .merge_commits(
                &head_commit,
                &repo.find_commit(fetch_commit.id()).to_string_err()?,
                None,
            )
            .to_string_err()?;
        if index.has_conflicts() {
            let conflicts: Vec<_> = index
                .conflicts()
                .to_string_err()?
                .collect::<Result<Vec<_>, _>>()
                .to_string_err()?;
            for conflict in conflicts {
                if let Some(their_entry) = conflict.their.as_ref() {
                    apply_resolution(&repo, &mut index, their_entry)?;
                } else if let Some(our_entry) = conflict.our.as_ref() {
                    apply_resolution(&repo, &mut index, our_entry)?;
                } else if let Some(ancestor_entry) = conflict.ancestor.as_ref() {
                    apply_resolution(&repo, &mut index, ancestor_entry)?;
                }
            }
            index.write().to_string_err()?;
        } else {
            let result_tree = repo
                .find_tree(index.write_tree_to(&repo).to_string_err()?)
                .to_string_err()?;
            let signature = repo.signature().to_string_err()?;
            let merge_commit = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    "Merge commit",
                    &result_tree,
                    &[
                        &head_commit,
                        &repo.find_commit(fetch_commit.id()).to_string_err()?,
                    ],
                )
                .to_string_err()?;
            new_commit_id = merge_commit;
        }
    }

    drop(fetch_head);
    drop(fetch_commit);
    let pull_update_info = handle_pull_info(repo, &branch_info.commit_id, &new_commit_id)?;

    Ok(GitOperationResult::new(
        GitOperationStatus::Success,
        Some(pull_update_info),
    ))
}

fn apply_resolution(
    repo: &Repository,
    index: &mut git2::Index,
    entry: &git2::IndexEntry,
) -> Result<(), String> {
    let path = Path::new(std::str::from_utf8(&entry.path).to_string_err()?);

    // Write the resolved content to the working directory
    let obj = repo
        .find_object(entry.id, Some(git2::ObjectType::Blob))
        .to_string_err()?;
    std::fs::write(path, obj.as_blob().unwrap().content()).to_string_err()?;

    index.add_path(path).to_string_err()?;

    Ok(())
}

fn handle_pull_info(
    repo: Repository,
    old_commit_id: &Oid,
    new_commit_id: &Oid,
) -> Result<PullUpdateInfo, String> {
    let mut walker = repo.revwalk().to_string_err()?;
    walker
        .push_range(&format!("{}..{}", old_commit_id, new_commit_id))
        .to_string_err()?;
    let num_commits = walker.count();

    let mut diff_opts = DiffOptions::new();
    diff_opts.context_lines(3);
    diff_opts.show_untracked_content(true);
    diff_opts.include_untracked(true);
    diff_opts.ignore_filemode(true);
    diff_opts.reverse(true);
    diff_opts.include_unmodified(false);
    diff_opts.patience(true);

    let old_tree = repo
        .find_commit(*old_commit_id)
        .to_string_err()?
        .tree()
        .to_string_err()?;
    let new_tree = repo
        .find_commit(*new_commit_id)
        .to_string_err()?
        .tree()
        .to_string_err()?;
    let diff = repo
        .diff_tree_to_tree(Some(&new_tree), Some(&old_tree), Some(&mut diff_opts))
        .to_string_err()?;

    let mut file_changes: HashMap<String, (usize, usize)> = HashMap::new();

    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |delta, _hunk, line| {
            if let (Some(delta), Some(line)) = (Some(delta), Some(line)) {
                let file_path = get_new_file_path(&delta);
                let entry = file_changes.entry(file_path).or_insert((0, 0));
                match line.origin() {
                    '+' => entry.0 += 1,
                    '-' => entry.1 += 1,
                    _ => {}
                }
            }
            true
        }),
    )
    .to_string_err()?;

    Ok(PullUpdateInfo {
        new_commit_id: short_oid_str(*new_commit_id),
        old_commit_id: short_oid_str(*old_commit_id),
        num_commits,
        file_changes,
    })
}

#[async_wrapper]
pub fn git_commit(repo_path: String, message: String) -> Result<(), String> {
    let repo = get_repo(repo_path.to_string())?;

    let oid = git_add(&repo)?;
    let tree = repo.find_tree(oid).to_string_err()?;

    let parent_commit = if let Ok(head) = repo.head() {
        if head.is_branch() {
            Some(head.peel_to_commit().to_string_err()?)
        } else {
            None
        }
    } else {
        None
    };

    let signature = create_signature(&repo).to_string_err()?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        parent_commit
            .as_ref()
            .map(|p| vec![p])
            .unwrap_or_default()
            .as_slice(),
    )
    .to_string_err()?;

    Ok(())
}

#[async_wrapper]
pub fn has_git_remote_url(repo_path: String) -> Result<bool, String> {
    let repo = match Repository::open(repo_path) {
        Ok(repo) => repo,
        Err(e) => return Err(e.to_string()),
    };

    let remote_names = repo.remotes().to_string_err()?;

    Ok(!remote_names.is_empty())
}

#[async_wrapper]
pub fn is_git_repository(repo_path: String) -> Result<bool, String> {
    match Repository::open(repo_path) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[async_wrapper]
pub fn git_configure_remote(
    repo_path: String,
    remote_name: String,
    url: String,
) -> Result<(), String> {
    let repo = get_repo(repo_path.to_string())?;
    let mut config = repo.config().to_string_err()?;

    config
        .set_str(&format_remote_url(&remote_name), &url)
        .to_string_err()?;

    for branch in repo.branches(Some(BranchType::Local)).to_string_err()? {
        let (branch, _) = branch.to_string_err()?;
        let branch_name = branch.name().to_string_err()?.unwrap_or_default();
        config
            .set_str(&format_branch_remote(branch_name), &remote_name)
            .to_string_err()?;
        config
            .set_str(
                &format_branch_merge(branch_name),
                &format_refs_heads(branch_name),
            )
            .to_string_err()?;
    }

    let mut fetch_options = create_fetch_options();

    let mut remote = repo.find_remote(&remote_name).to_string_err()?;

    config
        .set_str(
            &format_remote_fetch(&remote_name),
            &format_ref_specs(&remote_name, true),
        )
        .map_err(|e| e.to_string())?;

    remote
        .fetch(
            &[format_ref_specs(&remote_name, false)],
            Some(&mut fetch_options),
            None,
        )
        .map_err(|e| format!("Failed to fetch remote: {}", e))?;

    Ok(())
}

#[async_wrapper]
pub fn has_uncommitted_changes(repo_path: String) -> Result<bool, String> {
    return has_uncommitted_changes_sync(repo_path);
}

fn has_uncommitted_changes_sync(repo_path: String) -> Result<bool, String> {
    let repo = get_repo(repo_path)?;

    let mut status_opts = StatusOptions::new();
    status_opts
        .include_untracked(true)
        .recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut status_opts)).to_string_err()?;

    for entry in statuses.iter() {
        let status = entry.status();
        if status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_wt_new()
            || status.is_wt_modified()
            || status.is_wt_deleted()
        {
            return Ok(true);
        }
    }

    Ok(false)
}

#[async_wrapper]
pub fn has_unpushed_commits(repo_path: String) -> Result<bool, String> {
    return has_unpushed_commits_sync(repo_path);
}

fn has_unpushed_commits_sync(repo_path: String) -> Result<bool, String> {
    let repo = get_repo(repo_path.to_string())?;

    let head = match repo.head() {
        Ok(head) => head,
        Err(e) => return Err(format!("Failed to get HEAD reference: {}", e)),
    };

    if !head.is_branch() {
        return Ok(false);
    }

    let branch_name = match head.shorthand() {
        Some(name) => name,
        None => return Err("Failed to get branch name".to_string()),
    };

    let local_branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(branch) => branch,
        Err(_) => return Ok(false),
    };

    let local_commit = match local_branch.get().peel_to_commit() {
        Ok(commit) => commit,
        Err(_) => return Ok(false),
    };

    let config = repo.config().to_string_err()?;
    let remote_name = match config.get_string(&format_branch_remote(branch_name)) {
        Ok(remote) => remote,
        Err(_) => return Ok(true),
    };
    let merge_ref = match config.get_string(&format_branch_merge(branch_name)) {
        Ok(merge) => merge.strip_prefix(REFS_HEADS).unwrap_or(&merge).to_string(),
        Err(_) => return Ok(true),
    };

    let fetch_head = format_refs_remotes_branch(&remote_name, &merge_ref);
    let remote_branch = match repo.find_reference(&fetch_head) {
        Ok(ref_) => ref_,
        Err(_) => return Ok(true),
    };

    let remote_commit = match remote_branch.peel_to_commit() {
        Ok(commit) => commit,
        Err(_) => return Ok(true),
    };

    let mut revwalk = repo.revwalk().to_string_err()?;
    revwalk.push(local_commit.id()).to_string_err()?;
    revwalk.hide(remote_commit.id()).to_string_err()?;

    let unpushed_commits: Vec<_> = revwalk.collect::<Result<Vec<_>, _>>().to_string_err()?;

    Ok(!unpushed_commits.is_empty())
}

#[async_wrapper]
pub fn git_branches(repo_path: String) -> Result<Vec<BranchInfo>, String> {
    let repo = get_repo(repo_path.to_string())?;

    let mut branches = Vec::new();
    if repo.is_empty().unwrap_or(false) {
        return Ok(branches);
    }

    let branch_info = get_current_branch_info(&repo).to_string_err()?;

    for branch in repo.branches(Some(BranchType::Local)).to_string_err()? {
        let (branch, _) = branch.to_string_err()?;
        let branch_name = branch.name().to_string_err()?.unwrap_or_default();
        branches.push(BranchInfo {
            branch_name: branch_name.to_string(),
            branch_type: "local".to_string(),
            is_current: branch_name == branch_info.branch_name,
        });
    }

    for branch in repo.branches(Some(BranchType::Remote)).to_string_err()? {
        let (branch, _) = branch.to_string_err()?;
        let branch_name = branch.name().to_string_err()?.unwrap_or_default();
        if branch_name == format!("{}/HEAD", branch_info.remote_name) {
            continue;
        }
        branches.push(BranchInfo {
            branch_name: branch_name.to_string(),
            branch_type: "remote".to_string(),
            is_current: false,
        });
    }

    Ok(branches)
}

#[async_wrapper]
pub fn get_changed_files_in_commit(
    repo_path: String,
    commit_id: String,
) -> Result<Vec<String>, String> {
    let repo = get_repo(repo_path.to_string())?;
    let commit_oid = Oid::from_str(&commit_id).to_string_err()?;
    let commit = repo.find_commit(commit_oid).to_string_err()?;
    let parent_commit = if commit.parent_count() > 0 {
        Some(commit.parent(0).to_string_err()?)
    } else {
        None
    };

    let tree = commit.tree().to_string_err()?;

    let mut changed_files = Vec::new();
    if let Some(parent) = parent_commit {
        let parent_tree = parent.tree().to_string_err()?;
        let diff = repo
            .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
            .to_string_err()?;
        for delta in diff.deltas() {
            let file_name = delta
                .new_file()
                .path()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy();
            changed_files.push(file_name.to_string());
        }
    }

    Ok(changed_files)
}

#[async_wrapper]
pub fn get_file_diff_in_commit(
    repo_path: String,
    commit_id: String,
    file_path: String,
) -> Result<String, String> {
    let repo = get_repo(repo_path.to_string())?;

    let commit_oid = Oid::from_str(&commit_id).to_string_err()?;
    let commit = repo.find_commit(commit_oid).to_string_err()?;
    let parent_commit = if commit.parent_count() > 0 {
        Some(commit.parent(0).to_string_err()?)
    } else {
        None
    };

    let tree = commit.tree().to_string_err()?;

    let diff = if let Some(parent) = parent_commit {
        let parent_tree = parent.tree().to_string_err()?;
        repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
            .to_string_err()?
    } else {
        repo.diff_tree_to_tree(None, Some(&tree), None)
            .to_string_err()?
    };

    let mut diff_result = String::new();
    let mut opts = DiffOptions::new();
    opts.pathspec(file_path);

    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        let content = String::from_utf8_lossy(line.content());
        diff_result.push_str(&content);
        true
    })
    .to_string_err()?;

    Ok(diff_result)
}

#[async_wrapper]
pub fn git_checkout(repo_path: String, target: String) -> Result<String, String> {
    let repo = get_repo(repo_path.to_string())?;

    if let Ok(_reference) = repo.find_branch(&target, BranchType::Local) {
        repo.set_head(&format_refs_heads(&target)).to_string_err()?;
        repo.checkout_head(Some(
            CheckoutBuilder::default().force().remove_untracked(true),
        ))
        .to_string_err()?;
        return Ok(format!("Checked out to local branch {}", target));
    }

    let branch_info = get_current_branch_info(&repo).to_string_err()?;

    if let Ok(remote_branch) = repo.find_reference(&format_refs_remotes_branch(
        &branch_info.remote_name,
        &target,
    )) {
        let target_oid = remote_branch
            .target()
            .ok_or_else(|| "Failed to get target Oid from remote branch")?;

        let commit = repo.find_commit(target_oid).to_string_err()?;

        repo.branch(&target, &commit, true).to_string_err()?;
        repo.set_head(&format_refs_heads(&target)).to_string_err()?;
        repo.checkout_head(Some(
            CheckoutBuilder::default().force().remove_untracked(true),
        ))
        .to_string_err()?;
        let mut config = repo.config().to_string_err()?;
        config
            .set_str(&format_branch_remote(&target), &branch_info.remote_name)
            .to_string_err()?;
        config
            .set_str(&format_branch_merge(&target), &format_refs_heads(&target))
            .to_string_err()?;
        return Ok(format!("Checked out to remote branch {}", target));
    }

    if let Ok(commit) = repo.find_commit(repo.revparse_single(&target).to_string_err()?.id()) {
        repo.set_head_detached(commit.id()).to_string_err()?;
        repo.checkout_head(Some(CheckoutBuilder::default().safe()))
            .to_string_err()?;
        return Ok(format!("Checked out to commit '{}'", target));
    }

    Err(format!(
        "Target '{}' not found (branch, remote branch, or commit ID)",
        target
    ))
}

#[async_wrapper]
pub fn git_commit_history(repo_path: String, n: usize) -> Result<Vec<CommitInfo>, String> {
    let repo = get_repo(repo_path.to_string())?;

    let head = repo.head().to_string_err()?;
    let head_commit = head.peel_to_commit().to_string_err()?;

    let mut commits = Vec::new();
    let mut revwalk = repo.revwalk().to_string_err()?;
    revwalk.push(head_commit.id()).to_string_err()?;

    for (i, oid_result) in revwalk.enumerate() {
        if i >= n {
            break;
        }
        let oid = oid_result.to_string_err()?;
        let commit = repo.find_commit(oid).to_string_err()?;

        let short_id = repo
            .find_object(oid, None)
            .to_string_err()?
            .short_id()
            .to_string_err()?
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let commit_time = format_time(commit.time()).to_string_err()?;

        commits.push(CommitInfo {
            full_id: commit.id().to_string(),
            short_id,
            summary: commit.summary().unwrap_or("No summary").to_string(),
            time: commit_time,
        });
    }

    Ok(commits)
}

#[async_wrapper]
pub async fn git_clone(repo_url: String, destination: String) -> Result<bool, String> {
    let fetch_options = create_fetch_options();

    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);

    match repo_builder.clone(&repo_url, Path::new(&destination)) {
        Ok(_repo) => Ok(true),
        Err(err) => Err(format!("Failed to clone repository: {}", err)),
    }
}

#[async_wrapper]
pub fn git_new_branch(repo_path: String, branch_name: String) -> Result<(), String> {
    let repo = get_repo(repo_path.to_string())?;

    let head = repo.head().to_string_err()?;
    let commit_id = head
        .target()
        .ok_or_else(|| "Failed to get HEAD commit ID")?;

    let commit = repo.find_commit(commit_id).to_string_err()?;

    let refname = format_refs_heads(&branch_name);
    let mut _new_branch = repo.branch(&branch_name, &commit, false).to_string_err()?;
    repo.set_head(&format_refs_heads(&branch_name))
        .to_string_err()?;
    repo.checkout_head(Some(
        CheckoutBuilder::default().force().remove_untracked(true),
    ))
    .to_string_err()?;

    let branch_info = get_current_branch_info(&repo).to_string_err()?;
    let mut config = repo.config().to_string_err()?;
    config
        .set_str(
            &format_branch_remote(&branch_name),
            &branch_info.remote_name,
        )
        .to_string_err()?;
    config
        .set_str(
            &format_branch_merge(&branch_name),
            &format_refs_heads(&branch_name),
        )
        .to_string_err()?;

    repo.set_head(&refname).to_string_err()?;
    repo.checkout_head(None).to_string_err()?;

    Ok(())
}

fn has_current_branch_exists_remote(
    remote: &mut Remote,
    branch_name: &str,
) -> Result<(bool, Oid), String> {
    remote
        .connect_auth(Direction::Fetch, Some(create_callbacks()), None)
        .to_string_err()?;
    let references = remote.list().to_string_err()?;
    for reference in references.iter() {
        if let Some(remote_ref) = reference.name().strip_prefix(REFS_HEADS) {
            if remote_ref == branch_name {
                return Ok((true, reference.oid()));
            }
        }
    }
    Ok((false, Oid::zero()))
}

fn format_time(commit_time: Time) -> Result<String, Error> {
    let timestamp = commit_time.seconds();
    let datetime = UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);
    let datetime = SystemTime::from(datetime);

    let datetime: chrono::DateTime<chrono::Utc> = datetime.into();
    Ok(datetime.to_rfc3339())
}

fn short_oid_str(oid: Oid) -> String {
    oid.to_string().chars().take(7).collect::<String>()
}

fn get_new_file_path(delta: &DiffDelta) -> String {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_string()
}

fn format_refs_heads(remote_name: &str) -> String {
    format!("{}{}", REFS_HEADS, remote_name)
}

fn format_branch_remote(branch_name: &str) -> String {
    format!("branch.{}.remote", branch_name)
}

fn format_branch_merge(branch_name: &str) -> String {
    format!("branch.{}.merge", branch_name)
}

fn format_remote_fetch(remote_name: &str) -> String {
    format!("remote.{}.fetch", remote_name)
}

fn format_remote_url(remote_name: &str) -> String {
    format!("remote.{}.url", remote_name)
}

fn format_ref_specs(remote_name: &str, is_plus: bool) -> String {
    format!(
        "{}{}*:{}{}/*",
        if is_plus { "+" } else { "" },
        REFS_HEADS,
        REFS_REMOTES,
        remote_name
    )
}

fn format_refs_remotes_branch(remote_name: &str, branch_name: &str) -> String {
    format!("{}{}/{}", REFS_REMOTES, remote_name, branch_name)
}
