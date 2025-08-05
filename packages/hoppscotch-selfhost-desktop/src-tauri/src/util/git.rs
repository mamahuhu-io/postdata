use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use tauri_plugin_async_wrapper::async_wrapper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,    // commit hash
    pub author: String,  // 作者
    pub date: String,    // 日期时间
    pub message: String, // 提交信息
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
    PushConflict,
    PullConflict,
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
    git_path: String, // Git 可执行文件路径，默认为 "git"
}

impl Git {
    /// 创建一个新的 Git 实例
    pub fn new(git_path: Option<&str>) -> Self {
        Git {
            git_path: git_path.unwrap_or("git").to_string(),
        }
    }

    /// 执行 Git 命令并返回结果
    ///
    /// # 参数
    ///
    /// * `args` - Git 命令的参数列表
    ///
    /// # 返回值
    ///
    /// 返回 Result<Output, Error>，其中 Output 包含 stdout 和 stderr
    pub fn run(&self, args: &[&str]) -> Result<Output> {
        Command::new(&self.git_path)
            .args(args)
            .output()
            .map_err(|e| e.into())
    }

    /// 在指定目录下执行 Git 命令
    fn run_with_cwd(&self, args: &[&str], path: &str) -> Result<Output> {
        Command::new(&self.git_path)
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| e.into())
    }

    /// 初始化一个 Git 仓库
    pub fn init(&self, path: &str) -> Result<String> {
        let output = self.run(&["init", path])?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 添加所有更改到暂存区
    pub fn add_all(&self, path: &str) -> Result<String> {
        let output = self.run_with_cwd(&["add", "."], path)?;
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    /// 提交更改
    pub fn commit(&self, path: &str, message: &str) -> Result<String> {
        let output = self.run_with_cwd(&["commit", "-m", message], path)?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    /// 推送更改到远程仓库
    pub fn push(&self, path: &str) -> Result<String> {
        let remote = self
            .get_current_remote(path)
            .unwrap_or_else(|_| "origin".to_string());
        let output = self.run_with_cwd(&["push", "--set-upstream", &remote, "HEAD"], path)?;

        // 检查推送是否成功
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("rejected") || stderr.contains("non-fast-forward") {
                return Err(anyhow::anyhow!(
                    "Push rejected: remote has changes that need to be pulled first"
                ));
            }
        }

        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    /// 拉取远程更改
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

    /// 克隆远程仓库
    pub fn clone(&self, url: &str, path: &str) -> Result<String> {
        let output = self.run(&["clone", url, path])?;
        Ok(String::from_utf8_lossy(&output.stderr).to_string())
    }

    pub fn has_origin(&self, path: &str) -> Result<bool> {
        let output = self.run_with_cwd(&["remote"], path)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().any(|line| line.trim() == "origin"))
    }

    /// 设置远程仓库地址（覆盖）
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

    /// 获取当前分支名称
    pub fn get_current_branch(&self, path: &str) -> Result<String> {
        let output = self.run_with_cwd(&["branch", "--show-current"], path)?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Git command failed"));
        }

        let branch_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(branch_name)
    }

    pub fn get_current_remote(&self, path: &str) -> Result<String> {
        // 获取当前分支名
        let current_branch = self.get_current_branch(path)?;

        // 查询该分支配置的远程
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

    /// 获取所有本地分支并标记当前分支（前面加 *）
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

    /// 列出所有分支（远程跟踪分支）
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

    /// 切换到指定分支
    pub fn checkout(&self, path: &str, branch_name: &str) -> Result<String> {
        let output = self.run_with_cwd(&["checkout", branch_name], path)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(stdout.parse().unwrap())
        } else {
            Ok(stderr.parse().unwrap()) // 如果失败，返回错误信息
        }
    }

    /// 获取最近 20 条提交历史
    pub fn git_commit_history(&self, path: &str, n: usize) -> Result<Vec<CommitInfo>> {
        // 首先检查是否为 git 仓库
        let repo_check = self.run_with_cwd(&["rev-parse", "--git-dir"], path)?;
        if !repo_check.status.success() {
            return Err(anyhow::anyhow!("Not a git repository"));
        }

        // 检查是否有提交历史
        let has_commits = self.run_with_cwd(&["rev-list", "--count", "HEAD"], path)?;
        if !has_commits.status.success() {
            // 没有提交历史，返回空列表
            return Ok(Vec::new());
        }

        let commit_count = String::from_utf8_lossy(&has_commits.stdout)
            .trim()
            .to_string();
        if commit_count == "0" {
            // 没有提交，返回空列表
            return Ok(Vec::new());
        }

        let output = self.run_with_cwd(
            &[
                "log",
                "-n",
                &n.to_string(),
                "--pretty=format:%H|%an|%ai|%s", // 格式：hash|author|date|message
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

    /// 检查 Git 安装状态、仓库状态和远程配置
    pub fn git_check_status(&self, path: &str) -> Result<GitOperationResult> {
        // 检查 git 是否安装
        let version_output = Command::new(&self.git_path).arg("--version").output()?;
        if !version_output.status.success() {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::GitNotInstalled,
            ));
        }

        // 检查是否为 git 仓库
        let repo_check = self.run_with_cwd(&["rev-parse", "--git-dir"], path)?;
        if !repo_check.status.success() {
            return Ok(GitOperationResult::with_status_only(
                GitOperationStatus::NotGitRepository,
            ));
        }

        // 检查是否配置了远程仓库
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
        if has_pull {
            match self.pull(path) {
                Ok(_) => {}
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("Merge conflict") {
                        return Ok(GitOperationResult::new(
                            GitOperationStatus::MergeConflict,
                            "Pull failed due to merge conflicts. Please resolve conflicts manually.".to_string(),
                        ));
                    } else {
                        return Ok(GitOperationResult::new(
                            GitOperationStatus::PullConflict,
                            format!("Pull failed: {}", error_msg),
                        ));
                    }
                }
            }
        }

        self.add_all(path)?;
        self.commit(path, message)?;

        match self.push(path) {
            Ok(_) => Ok(GitOperationResult::with_status_only(
                GitOperationStatus::Success,
            )),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Push rejected") {
                    Ok(GitOperationResult::new(
                        GitOperationStatus::PushConflict,
                        "Push failed: remote has changes that need to be pulled first.".to_string(),
                    ))
                } else {
                    Ok(GitOperationResult::new(
                        GitOperationStatus::PushConflict,
                        format!("Push failed: {}", error_msg),
                    ))
                }
            }
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
fn git_push(path: String) -> Result<String, String> {
    let git = Git::new(None);
    git.push(&path).map_err(|e| e.to_string())
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
