use crate::util::file::copy_files;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};
use tauri_plugin_async_wrapper::async_wrapper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,    // commit hash
    pub author: String,  // author
    pub date: String,    // date time
    pub message: String, // commit message
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ConflictStrategy {
    Manual,
    Remote,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitOperationStatus {
    Success,
    BranchNotExistsRemote,
    UncommittedChanges,
    UnPushedChanges,
    AlreadyUpToDate,
    NoRemoteConfigured,
    GitNotInstalled,
    GitVersionTooLow,
    NotGitRepository,
    MergeConflict,
    LocalAHead,
    RemoteAHead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOperationResult<T = String> {
    pub status: GitOperationStatus,
    pub result: T,
}

impl<T> GitOperationResult<T> {
    fn new(status: GitOperationStatus, result: T) -> Self {
        GitOperationResult { result, status }
    }
}

impl GitOperationResult {
    fn with_status_only(status: GitOperationStatus) -> Self {
        GitOperationResult {
            status,
            result: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub branch_name: String,
    pub branch_type: String,
    pub is_current: bool,
}

/// Git 工具结构体
pub struct Git {
    git_path: String, // git executable path, default is "git"
}

impl Git {
    /// create a new git instance
    pub fn new(git_path: Option<&str>) -> Self {
        Git {
            git_path: git_path.unwrap_or("git").to_string(),
        }
    }

    /// execute git command and return result
    ///
    /// # parameters
    ///
    /// * `args` - git command arguments
    ///
    /// # return value
    ///
    /// return Result<Output, Error>, where Output contains stdout and stderr
    pub fn run(&self, args: &[&str]) -> Result<Output> {
        Command::new(&self.git_path)
            .args(args)
            .output()
            .map_err(|e| e.into())
    }

    /// execute git command in specified directory
    fn run_with_cwd<S: AsRef<OsStr>>(
        &self,
        args: impl IntoIterator<Item = S>,
        path: &str,
    ) -> Result<Output> {
        Command::new(&self.git_path)
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| e.into())
    }

    /// initialize a git repository
    pub fn init(&self, path: &str) -> Result<String> {
        let output = self.run(&["init", path])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// add all changes to staging area
    pub fn add_all(&self, path: &str) -> Result<String> {
        let output = self.run_with_cwd(&["add", "."], path)?;
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    /// commit changes
    pub fn commit(&self, path: &str, message: &str) -> Result<String> {
        let output = self.run_with_cwd(&["commit", "-m", message], path)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// push changes to remote repository, if upstream is not set, add --set-upstream origin HEAD
    pub fn push(&self, path: &str, is_force: bool) -> Result<String> {
        let has_upstream = self.has_upstream(path)?;

        let mut args: Vec<String> = Vec::new();
        args.push("push".to_string());
        if !has_upstream {
            let remote = self
                .get_current_remote(path)
                .unwrap_or_else(|_| "origin".to_string());
            args.push("--set-upstream".to_string());
            args.push(remote);
            args.push("HEAD".to_string());
        }
        if is_force {
            args.push("--force".to_string());
        }
        let output = self.run_with_cwd(args, path)?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stderr).to_string())
        } else {
            Err(anyhow::anyhow!(
                "{}",
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }

    pub fn push_with_conflict(&self, path: &str, is_force: bool) -> Result<GitOperationResult> {
        let remote = self.get_current_remote(path)?;
        let branch = self.get_current_branch(path)?;
        self.fetch(path, &remote, &branch)?;
        println!("fetch: {}", remote);
        // let remote_changes_files = self.get_remote_changes_files(path, &remote, &branch)?;
        // println!("remote_changes_files: {:?}", remote_changes_files);
        // if !remote_changes_files.is_empty() {
        //     return Ok(GitOperationResult::with_status_only(
        //         GitOperationStatus::RemoteChanges,
        //     ));
        // }
        let rev_list = self.rev_list(path, &remote, &branch)?;
        println!("rev_list: {:?}", rev_list);
        if matches!(rev_list, GitOperationStatus::RemoteAHead) {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::RemoteAHead,
            ));
        }
        let has_conflict = self.has_merge_conflict(path, &remote, &branch)?;
        println!("has_conflict: {}", has_conflict);
        if has_conflict {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::MergeConflict,
            ));
        }
        let push_result = self.push(path, is_force)?;
        Ok(GitOperationResult::new(
            GitOperationStatus::Success,
            push_result,
        ))
    }

    pub fn has_upstream(&self, path: &str) -> Result<bool> {
        let output = self.run_with_cwd(
            &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
            path,
        )?;
        Ok(output.status.success())
    }

    /// pull remote changes
    pub fn pull(&self, path: &str) -> Result<String> {
        let output = self.run_with_cwd(&["pull"], path)?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            Err(anyhow::anyhow!("{}", stderr.to_string()))
        }
    }

    pub fn pull_with_able(&self, path: &str) -> Result<GitOperationResult> {
        let remote = self.get_current_remote(path)?;
        let branch = self.get_current_branch(path)?;

        self.fetch(path, &remote, &branch)?;

        let is_pull_able = self.is_pull_able(path, &remote, &branch)?;
        if is_pull_able {
            let pull_result = self.pull(path)?;
            Ok(GitOperationResult::new(
                GitOperationStatus::Success,
                pull_result,
            ))
        } else {
            Ok(GitOperationResult::with_status_only(
                GitOperationStatus::MergeConflict,
            ))
        }
    }

    /// clone remote repository
    pub fn clone(&self, url: &str, path: &str) -> Result<String> {
        let output = self.run(&["clone", url, path])?;
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    pub fn fetch(&self, path: &str, remote: &str, branch: &str) -> Result<String> {
        let output = self.run_with_cwd(&["fetch", remote, branch], path)?;
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    pub fn reset(&self, path: &str, remote: &str, branch: &str) -> Result<String> {
        let output = self.run_with_cwd(
            &["reset", "--hard", &format!("{}/{}", remote, branch)],
            path,
        )?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn has_origin(&self, path: &str) -> Result<bool> {
        let output = self.run_with_cwd(&["remote"], path)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().any(|line| line.trim() == "origin"))
    }

    /// set remote repository address (overwrite)
    pub fn ensure_remote_origin(
        &self,
        path: &str,
        remote_name: &str,
        url: &str,
    ) -> Result<GitOperationResult, String> {
        if self.has_origin(path).map_err(|e| e.to_string())? {
            let output = self
                .run_with_cwd(&["remote", "set-url", remote_name, url], path)
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                self.run_with_cwd(&["config", "push.autoSetupRemote", "true"], path)
                    .map_err(|e| e.to_string())?;
                Ok(GitOperationResult::with_status_only(
                    GitOperationStatus::Success,
                ))
            } else {
                Err(String::from_utf8_lossy(&output.stderr).into())
            }
        } else {
            let output = self
                .run_with_cwd(&["remote", "add", remote_name, url], path)
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                self.run_with_cwd(&["config", "push.autoSetupRemote", "true"], path)
                    .map_err(|e| e.to_string())?;
                Ok(GitOperationResult::with_status_only(
                    GitOperationStatus::Success,
                ))
            } else {
                Err(String::from_utf8_lossy(&output.stderr).into())
            }
        }
    }

    /// get current branch name
    pub fn get_current_branch(&self, path: &str) -> Result<String> {
        let output = self.run_with_cwd(&["branch", "--show-current"], path)?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git command failed"));
        }

        let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch_name)
    }

    pub fn get_current_remote(&self, path: &str) -> Result<String> {
        // get current branch name
        let current_branch = self.get_current_branch(path)?;

        // query the remote configured for the branch
        let output = self.run_with_cwd(
            &["config", &format!("branch.{}.remote", current_branch)],
            path,
        )?;

        if output.status.success() {
            let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !remote.is_empty() {
                return Ok(remote);
            }
        }

        Err(anyhow::anyhow!("No remote configured for current branch"))
    }

    /// get all local branches and mark current branch (add * prefix)
    pub fn list_local_branches(&self, path: &str) -> Result<Vec<String>> {
        self.run_with_cwd(&["branch"], path).and_then(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect())
        })
    }

    /// list all branches (remote tracking branches)
    pub fn list_remote_branches(&self, path: &str) -> Result<Vec<String>> {
        self.run_with_cwd(&["branch", "-r"], path)
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(stdout
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && !s.contains("->"))
                    .collect())
            })
    }

    /// switch to specified branch
    pub fn checkout(&self, path: &str, branch_name: &str) -> Result<String> {
        let output = self.run_with_cwd(&["checkout", branch_name], path)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(stdout.parse().unwrap())
        } else {
            Ok(stderr.parse().unwrap()) // if failed, return error message
        }
    }

    /// get recent 20 commit history
    pub fn git_commit_history(&self, path: &str, n: usize) -> Result<Vec<CommitInfo>> {
        // first check if it is a git repository
        let repo_check = self.run_with_cwd(&["rev-parse", "--git-dir"], path)?;
        if !repo_check.status.success() {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        // check if there is commit history
        let has_commits = self.run_with_cwd(&["rev-list", "--count", "HEAD"], path)?;
        if !has_commits.status.success() {
            // no commit history, return empty list
            return Ok(Vec::new());
        }

        let commit_count = String::from_utf8_lossy(&has_commits.stdout)
            .trim()
            .to_string();
        if commit_count == "0" {
            // no commit, return empty list
            return Ok(Vec::new());
        }

        let output = self.run_with_cwd(
            &[
                "log",
                "-n",
                &n.to_string(),
                "--pretty=format:%H|%an|%ai|%s", // format: hash|author|date|message
            ],
            path,
        )?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git command failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 4 {
                commits.push(CommitInfo {
                    hash: parts[0].to_string(),
                    author: parts[1].to_string(),
                    date: parts[2].to_string(),
                    message: parts[3].to_string(),
                });
            }
        }

        Ok(commits)
    }

    /// check git installation status, repository status and remote configuration
    pub fn git_check_status(&self, path: &str) -> Result<GitOperationResult> {
        // check if git is installed
        let version_output = Command::new(&self.git_path).arg("--version").output()?;
        if !version_output.status.success() {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::GitNotInstalled,
            ));
        }

        // check if it is a git repository
        let repo_check = self.run_with_cwd(&["rev-parse", "--git-dir"], path)?;
        if !repo_check.status.success() {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::NotGitRepository,
            ));
        }

        // check if remote repository is configured
        let remote_check = self.run_with_cwd(&["remote", "get-url", "origin"], path)?;
        if !remote_check.status.success() {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::NoRemoteConfigured,
            ));
        }

        Ok(GitOperationResult::with_status_only(
            GitOperationStatus::Success,
        ))
    }

    pub fn git_new_branch(&self, path: &str, branch_name: &str) -> Result<String> {
        self.run_with_cwd(&["checkout", "-b", branch_name], path)?;
        self.checkout(path, branch_name)
    }

    pub fn git_sync(
        &self,
        path: &str,
        message: &str,
        has_pull: bool,
    ) -> Result<GitOperationResult> {
        let remote = self.get_current_remote(path)?;
        let branch = self.get_current_branch(path)?;
        self.fetch(path, &remote, &branch)?;

        if has_pull {
            let is_pull_able = self.is_pull_able(path, &remote, &branch)?;
            if is_pull_able {
                self.pull(path)?;
            } else {
                return Ok(GitOperationResult::with_status_only(
                    GitOperationStatus::MergeConflict,
                ));
            }
        }

        self.add_all(path)?;
        self.commit(path, message)?;

        if !has_pull {
            let has_conflict = self.has_merge_conflict(path, &remote, &branch)?;
            if has_conflict {
                return Ok(GitOperationResult::with_status_only(
                    GitOperationStatus::MergeConflict,
                ));
            }
        }

        self.push(path, false)?;
        Ok(GitOperationResult::with_status_only(
            GitOperationStatus::Success,
        ))
    }

    /// check if there is a merge conflict
    pub fn has_merge_conflict(
        &self,
        path: &str,
        remote_name: &str,
        branch_name: &str,
    ) -> Result<bool> {
        let upstream_ref = format!("{}/{}", remote_name, branch_name);
        let base_output = self.run_with_cwd(&["merge-base", "HEAD", &upstream_ref], path)?;

        if !base_output.status.success() {
            return Ok(false);
        }

        let binding = String::from_utf8_lossy(&base_output.stdout).to_string();
        let base_hash = binding.trim();

        // call git merge-tree <base> HEAD origin/master
        let merge_tree_output =
            self.run_with_cwd(&["merge-tree", base_hash, "HEAD", &upstream_ref], path)?;

        if !merge_tree_output.status.success() {
            return Ok(false);
        }

        let output = String::from_utf8_lossy(&merge_tree_output.stdout).to_string();

        // check if there is a conflict
        let has_conflict =
            output.contains("<<<<<<<") || output.contains("=======") || output.contains(">>>>>>>");

        Ok(has_conflict)
    }

    pub fn get_local_changes_files(&self, path: &str) -> Result<Vec<String>> {
        let output = self.run_with_cwd(&["status", "--porcelain"], path)?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut files = Vec::new();

        for line in stdout.lines() {
            // porcelain format: XY <file>
            // XY: two characters, respectively represent the status of the staging area and the working area
            // the 3rd character starts with the file name
            if line.len() < 4 {
                continue;
            }
            let file = line[3..].to_string();
            files.push(file);
        }

        Ok(files)
    }

    pub fn get_remote_changes_files(
        &self,
        path: &str,
        remote_name: &str,
        branch_name: &str,
    ) -> Result<Vec<String>> {
        let upstream_ref = format!("{}/{}", remote_name, branch_name);
        let output = self.run_with_cwd(&["diff", "--name-only", "HEAD", &upstream_ref], path)?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(files)
    }

    pub fn get_local_remote_diff_files(
        &self,
        path: &str,
        remote_name: &str,
        branch_name: &str,
    ) -> Result<Vec<String>> {
        let diff_files = self.get_remote_changes_files(path, remote_name, branch_name)?;
        let local_files = self.get_local_changes_files(path)?;
        let remote_set: HashSet<_> = diff_files.iter().collect();
        let local_set: HashSet<_> = local_files.iter().collect();
        Ok(remote_set
            .intersection(&local_set)
            .clone()
            .map(|s| s.to_string())
            .collect())
    }

    pub fn is_pull_able(&self, path: &str, remote_name: &str, branch_name: &str) -> Result<bool> {
        let local_remote_files =
            self.get_local_remote_diff_files(path, remote_name, branch_name)?;
        if !local_remote_files.is_empty() {
            return Ok(false);
        }
        let has_conflict = self.has_merge_conflict(path, remote_name, branch_name);
        if has_conflict.unwrap_or(false) {
            return Ok(false);
        }
        Ok(true)
    }

    pub fn backup_conflict_files(&self, path: &str, files: Vec<String>) -> Result<bool> {
        if files.is_empty() {
            return Ok(true);
        }
        // generate timestamp directory
        let current_dir = Path::new(path);
        let timestamp = Local::now().format("%Y%m%d%H%M%S").to_string();
        let backup_dir = current_dir.join("backup").join(timestamp);
        copy_files(current_dir, &backup_dir, files)?;
        Ok(true)
    }

    pub fn resolve_conflicts(
        &self,
        path: &str,
        strategy: ConflictStrategy,
    ) -> Result<GitOperationResult> {
        let remote = self.get_current_remote(path)?;
        let branch = self.get_current_branch(path)?;
        self.fetch(path, &remote, &branch)?;
        match strategy {
            ConflictStrategy::Manual => Ok(GitOperationResult::with_status_only(
                GitOperationStatus::MergeConflict,
            )),
            ConflictStrategy::Remote => {
                let files = self.get_local_remote_diff_files(path, &remote, &branch)?;
                self.backup_conflict_files(path, files)?;
                self.fetch(path, &remote, &branch)?;
                let rest_result = self.reset(path, &remote, &branch)?;
                Ok(GitOperationResult::new(
                    GitOperationStatus::Success,
                    rest_result,
                ))
            }
            ConflictStrategy::Local => {
                self.add_all(path)?;
                self.commit(path, "conflict resolve")?;
                let push_result = self.push(path, true)?;
                Ok(GitOperationResult::new(
                    GitOperationStatus::Success,
                    push_result,
                ))
            }
        }
    }

    pub fn rev_list(
        &self,
        path: &str,
        remote_name: &str,
        branch_name: &str,
    ) -> Result<GitOperationStatus> {
        let output = self.run_with_cwd(
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{}/{}...HEAD", remote_name, branch_name),
            ],
            path,
        )?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        let result = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = result.trim().split_whitespace().collect();

        if parts.len() != 2 {
            return Ok(GitOperationStatus::Success);
        }

        let left: i32 = parts[0].parse().unwrap_or(0);
        let right: i32 = parts[1].parse().unwrap_or(0);

        match (left, right) {
            (0, 0) => {
                // ✅ Branch is synchronized, no need to push/pull
                Ok(GitOperationStatus::Success)
            }
            (0, r) if r > 0 => {
                // ✅ Can push directly, local ahead {r} commits
                Ok(GitOperationStatus::LocalAHead)
            }
            (l, 0) if l > 0 => {
                // ⚠️ Local is lagging behind {l} commits, need to pull first
                Ok(GitOperationStatus::RemoteAHead)
            }
            (l, r) if l > 0 && r > 0 => {
                // ⚠️ Local ahead {r} commits, remote ahead {l} commits, history fork, need to pull --rebase or merge
                Ok(GitOperationStatus::MergeConflict)
            }
            _ => Err(anyhow::anyhow!(
                "Unknown situation: left={left}, right={right}"
            )),
        }
    }
}

#[async_wrapper]
fn git_init(path: String) -> Result<String, String> {
    let git = Git::new(None);
    git.init(&path).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_add(path: String) -> Result<String, String> {
    let git = Git::new(None);
    git.add_all(&path).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_commit(path: String, message: String) -> Result<String, String> {
    let git = Git::new(None);
    git.add_all(&path).map_err(|e| e.to_string())?;
    git.commit(&path, &message).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_push(path: String, is_force: bool) -> Result<String, String> {
    let git = Git::new(None);
    git.push(&path, is_force).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_pull(path: String) -> Result<String, String> {
    let git = Git::new(None);
    git.pull(&path).map_err(|e| e.to_string())
}

//git_checkout
#[async_wrapper]
fn git_checkout(path: String, branch_name: String) -> Result<String, String> {
    let git = Git::new(None);
    git.checkout(&path, &branch_name).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_clone(url: String, path: String) -> Result<String, String> {
    let git = Git::new(None);
    git.clone(&url, &path).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_configure_remote(
    path: String,
    remote_name: String,
    url: String,
    message: String,
) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.ensure_remote_origin(&path, &remote_name, &url)?;
    git.git_sync(&path, &message, false)
        .map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_commit_history(path: String, n: usize) -> Result<Vec<CommitInfo>, String> {
    let git = Git::new(None);
    git.git_commit_history(&path, n).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_check_status(path: String) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.git_check_status(&path).map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_branches(path: String) -> Result<Vec<BranchInfo>, String> {
    let git = Git::new(None);
    let mut branches = Vec::new();

    // Get local branches
    match git.list_local_branches(&path) {
        Ok(local_branches) => {
            for branch in local_branches {
                let branch_name = branch.trim_start_matches("* ").to_string();
                branches.push(BranchInfo {
                    branch_name: branch_name.clone(),
                    branch_type: "local".to_string(),
                    is_current: branch.starts_with("* "),
                });
            }
        }
        Err(e) => return Err(e.to_string()),
    }

    // Get remote branches
    match git.list_remote_branches(&path) {
        Ok(remote_branches) => {
            for branch in remote_branches {
                branches.push(BranchInfo {
                    branch_name: branch,
                    branch_type: "remote".to_string(),
                    is_current: false,
                });
            }
        }
        Err(e) => return Err(e.to_string()),
    }
    Ok(branches)
}

// git_sync
#[async_wrapper]
fn git_sync(path: String, message: String, has_pull: bool) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.git_sync(&path, &message, has_pull)
        .map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_new_branch(path: String, branch_name: String) -> Result<String, String> {
    let git = Git::new(None);
    git.git_new_branch(&path, &branch_name)
        .map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_resolve_conflicts(
    path: String,
    strategy: ConflictStrategy,
) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.resolve_conflicts(&path, strategy)
        .map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_push_with_conflict(path: String, is_force: bool) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.push_with_conflict(&path, is_force)
        .map_err(|e| e.to_string())
}

#[async_wrapper]
fn git_pull_with_able(path: String) -> Result<GitOperationResult, String> {
    let git = Git::new(None);
    git.pull_with_able(&path).map_err(|e| e.to_string())
}
