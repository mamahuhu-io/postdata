import { invoke } from "@tauri-apps/api/core"
import { PersistenceService } from "~/services/persistence"
import { getService } from "~/modules/dioc"

const dataPathKey = "dataPath"

export interface BranchInfo {
  branchName: string
  branchType: string
  isCurrent: boolean
}
export interface PullUpdateInfo {
  newCommitId: string
  oldCommitId: string
  numCommits: number
  fileChanges: Record<string, [number, number]>
}
export interface CommitInfo {
  fullId: string
  shortId: string
  summary: string
  time: string
}
export enum GResultEnum {
  Success = "Success",
  BranchNotExistsRemote = "BranchNotExistsRemote",
  UncommittedChanges = "UncommittedChanges",
  AlreadyUpToDate = "AlreadyUpToDate",
  NoRemoteConfigured = "NoRemoteConfigured",
  UnPushedChanges = "UnPushedChanges",
  GitNotInstalled = "GitNotInstalled",
  GitVersionTooLow = "GitVersionTooLow",
  NotGitRepository = "NotGitRepository",
}
export interface GResult<T> {
  status: GResultEnum
  result: T
}

const getDataPath = () => {
  const persistenceService = getService(PersistenceService)
  const dataPath = persistenceService.getLocalConfig(dataPathKey)
  return dataPath
}

export const listGitBranches = async () => {
  const result = await invoke<BranchInfo[]>("git_branches", {
    path: getDataPath(),
  })
  return result
}

export const gitInit = async () => {
  return await invoke<string>("git_init", {
    path: getDataPath(),
  })
}

export const gitCheckout = async (branchName: string) => {
  return await invoke<string>("git_checkout", {
    path: getDataPath(),
    branchName: branchName,
  })
}

export const gitPull = async () => {
  return await invoke<string>("git_pull", {
    path: getDataPath(),
  })
}

export const gitPush = async () => {
  return await invoke<string>("git_push", {
    path: getDataPath(),
  })
}

export const gitCommit = async (message: string) => {
  return await invoke<string>("git_commit", {
    path: getDataPath(),
    message: message,
  })
}

export const gitSync = async (message: string, hasPull: boolean) => {
  return await invoke<string>("git_sync", {
    path: getDataPath(),
    message: message,
    hasPull: hasPull,
  })
}

export const listGitCommitHistory = async (num: number) => {
  return await invoke<CommitInfo[]>("git_commit_history", {
    path: getDataPath(),
    n: num,
  })
}

export const gitClone = async (repoUrl: string) => {
  return await invoke<string>("git_clone", {
    url: repoUrl,
    path: getDataPath(),
  })
}

export const gitConfigureRemote = async (
  remoteName: string,
  url: string,
  message: string
) => {
  return await invoke<GResult<string>>("git_configure_remote", {
    path: getDataPath(),
    remoteName: remoteName,
    url: url,
    message: message,
  })
}

export const getCurrentBranchName = async (): Promise<string> => {
  let currentBranch = "master"
  const resultBranch = await listGitBranches()
  for (const b of resultBranch) {
    if (b.isCurrent) {
      currentBranch = b.branchName
    }
  }
  return currentBranch
}

export const gitNewBranch = async (branchName: string) => {
  return await invoke<string>("git_new_branch", {
    path: getDataPath(),
    branchName: branchName,
  })
}

export const gitCheckStatus = async () => {
  return await invoke<string>("git_check_status", {
    path: getDataPath(),
  })
}
