<template>
  <span class="inline-flex">
    <tippy
      id="git-commit"
      interactive
      trigger="click"
      theme="popover"
      :hide-on-click="true"
    >
      <HoppButtonSecondary
        v-tippy="{ theme: 'tooltip' }"
        :title="currentBranch"
        :label="currentBranch"
        :loading="loadingState"
        style="padding-right: 2px"
        :on-shown="() => tippyActions!.focus()"
      />
      <i v-if="!loadingState" style="padding-top: 10px">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="1em"
          height="1em"
          viewBox="0 0 24 24"
        >
          <path
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="m6 9l6 6l6-6"
          />
        </svg>
      </i>
      <template #content="{ hide }">
        <div v-show="!loadingState">
          <div
            v-if="isGitDir"
            ref="tippyActions"
            class="flex flex-col focus:outline-none pd-0"
            style="min-width: 200px"
            @keyup.escape="hide()"
          >
            <HoppSmartItem
              label="Update..."
              :icon="IconArrowDownLeft"
              @click="
                () => {
                  pull()
                  hide()
                }
              "
            />
            <HoppSmartItem
              label="Commit..."
              :icon="IconGitCommitHorizontal"
              @click="
                () => {
                  showCommit()
                  hide()
                }
              "
            />
            <HoppSmartItem
              v-if="hasRemoteUrl"
              label="Push..."
              :icon="IconArrowDownRight"
              @click="
                () => {
                  push()
                  hide()
                }
              "
            />
            <HoppSmartItem
              v-if="!hasRemoteUrl"
              label="Define remote"
              @click="
                () => {
                  gitDefineRemote()
                  hide()
                }
              "
            />
            <div v-if="isGitDir && hasRemoteUrl">
              <hr />
              <HoppSmartItem
                v-if="branchRemote.length > 0"
                label="New Branch..."
                :icon="IconPlus"
                @click="
                  () => {
                    newBranch()
                    hide()
                  }
                "
              />
              <hr />
              <div class="pb-2 pl-4 text-tiny text-secondaryLight">Local</div>
              <div class="flex flex-col">
                <HoppSmartItem
                  v-for="branche in branchLocals"
                  :key="branche.branchName"
                  :label="branche.branchName"
                  :title="branche.branchName"
                  :icon="
                    branche.branchName === currentBranch
                      ? IconCheck
                      : IconGitBranch
                  "
                  @click="
                    () => {
                      checkoutGitRepository(branche.branchName)
                      hide()
                    }
                  "
                ></HoppSmartItem>
              </div>
              <hr />
              <div class="pb-2 pl-4 text-tiny text-secondaryLight">Remote</div>
              <div class="flex flex-col">
                <HoppSmartItem
                  v-for="branche in branchRemote"
                  :key="branche.branchName"
                  :label="branche.branchName"
                  :title="branche"
                  :icon="IconGitBranch"
                  @click="
                    () => {
                      checkoutGitRepository(branche.branchName)
                      hide()
                    }
                  "
                ></HoppSmartItem>
              </div>
            </div>
          </div>
          <div
            v-if="!isGitDir && isGitInstalled"
            ref="tippyActions"
            class="flex flex-col focus:outline-none"
            style="min-width: 200px"
          >
            <HoppSmartItem
              label="Clone Git Repository..."
              @click="
                () => {
                  gitClone()
                  hide()
                }
              "
            />
            <HoppSmartItem
              label="Create Git Repository..."
              @click="
                () => {
                  initGitRepository()
                  hide()
                }
              "
            />
          </div>
          <div v-if="!isGitInstalled">
            <HoppSmartItem label="Please Install Git" @click="installGit" />
          </div>
        </div>
      </template>
    </tippy>
    <HoppButtonPrimary
      label="Git Sync"
      :loading="loadingStateGitSync"
      class="h-8"
      style="margin-left: 20px"
      @click="sync"
    />
    <SmartGitClone
      :show="showModalClone"
      @hide-modal="showModalClone = false"
      @init="checkGitStatus"
    />
    <SmartGitDefineRemote
      :show="showModalDefineRemote"
      @hide-modal="showModalDefineRemote = false"
      @init="checkGitStatus"
      @init-git-branches="initGitBranches"
    />
    <SmartGitNewBranch
      :show="showNewBranch"
      @hide-modal="showNewBranch = false"
      @init="checkGitStatus"
    />
    <SmartGitConflicts
      :show="showModalConflicts"
      @hide-modal="showModalConflicts = false"
      @init="checkGitStatus"
    />
  </span>
</template>

<script setup lang="ts">
import { ref } from "vue"
import { useI18n } from "@composables/i18n"
import { HoppButtonPrimary } from "@mamahuhu/ui"
import { useToast } from "~/composables/toast"
import { invokeAction, defineActionHandler } from "@helpers/actions"
import {
  listGitBranches,
  gitInit,
  gitCheckout,
  gitPull,
  pullWithAble,
  pushWithConflict,
  GResultEnum,
  BranchInfo,
  gitCheckStatus,
  gitSync,
} from "@helpers/git"
import { useService } from "dioc/vue"
import { PersistenceService } from "~/services/persistence"
import { TippyComponent } from "vue-tippy"
import IconArrowDownLeft from "~icons/lucide/arrow-down-left"
import IconArrowDownRight from "~icons/lucide/arrow-down-right"
import IconGitCommitHorizontal from "~icons/lucide/git-commit-horizontal"
import IconPlus from "~icons/lucide/plus"
import IconGitBranch from "~icons/lucide/git-branch"
import IconCheck from "~icons/lucide/check"
import { open } from "@tauri-apps/plugin-shell"

// const i18n = useFullI18n()
const t = useI18n()
const toast = useToast()
const branchLocals = ref<BranchInfo[]>([])
const branchRemote = ref<BranchInfo[]>([])
const currentBranch = ref("version control")
const isGitDir = ref(false)
const hasRemoteUrl = ref(false)
const showModalClone = ref(false)
const showModalDefineRemote = ref(false)
const showNewBranch = ref(false)
const showModalConflicts = ref(false)
const loadingStateGitSync = ref(false)
const loadingState = ref(false)
const isGitInstalled = ref(true)
const persistenceService = useService(PersistenceService)
const sync = () => {
  ;(async () => {
    loadingStateGitSync.value = true
    gitSync(`${t("git.default_commit_message")}`, true)
      .then((result) => {
        if (result.status === GResultEnum.Success) {
          toast.success(result.status)
          checkGitStatus()
          initGitBranches()
          persistenceService.setupLocalPersistence()
        } else if (result.status === GResultEnum.MergeConflict) {
          showModalConflicts.value = true
        }
        loadingStateGitSync.value = false
      })
      .catch((error) => {
        toast.error(error)
        checkGitStatus()
        initGitBranches()
        persistenceService.setupLocalPersistence()
        loadingStateGitSync.value = false
      })
  })()
}

const checkGitStatus = () => {
  gitCheckStatus()
    .then((result) => {
      hasRemoteUrl.value = false
      isGitDir.value = false
      if (result.status === GResultEnum.Success) {
        isGitDir.value = true
        hasRemoteUrl.value = true
        gitPull()
          .then(() => {
            initGitBranches()
          })
          .catch(() => {
            initGitBranches()
          })
      } else if (result.status === GResultEnum.GitNotInstalled) {
        hasRemoteUrl.value = false
        isGitInstalled.value = false
      } else if (result.status === GResultEnum.NotGitRepository) {
        isGitDir.value = false
        currentBranch.value = "version control"
      } else if (result.status === GResultEnum.NoRemoteConfigured) {
        isGitDir.value = true
        hasRemoteUrl.value = false
      }
    })
    .catch((error) => {
      toast.error(error)
    })
}
checkGitStatus()

const initGitBranches = () => {
  listGitBranches()
    .then((result) => {
      branchLocals.value = []
      branchRemote.value = []
      for (const b of result) {
        if (b.branchType === "local") {
          if (b.isCurrent) {
            currentBranch.value = b.branchName
          }
          branchLocals.value.push(b)
        } else if (b.branchType === "remote") {
          branchRemote.value.push(b)
        }
      }
    })
    .catch((error) => {
      toast.error(error)
    })
}

const initGitRepository = async () => {
  gitInit()
    .then((result) => {
      toast.success(result)
      checkGitStatus()
    })
    .catch((error) => {
      toast.error(error)
      checkGitStatus()
    })
}

const checkoutGitRepository = async (branchName: string) => {
  loadingState.value = true
  const branchNameFormat = formatBranchName(branchName)
  gitCheckout(branchNameFormat)
    .then((result) => {
      toast.success(result)
      checkGitStatus()
      initGitBranches()
      persistenceService.setupLocalPersistence()
      loadingState.value = false
    })
    .catch((error) => {
      checkGitStatus()
      initGitBranches()
      toast.error(error)
      loadingState.value = false
    })
}

const formatBranchName = (val: string) => {
  if (val.startsWith("*")) {
    return val.replace("*", "").trim()
  } else if (val.includes("/")) {
    const valArr = val.split("/")
    return valArr[valArr.length - 1]
  }
  return val
}

const pull = () => {
  loadingState.value = true
  pullWithAble()
    .then((result) => {
      // toast.success(JSON.stringify(result))
      if (result.status === GResultEnum.Success) {
        toast.success(result.result)
        initGitBranches()
      } else if (result.status === GResultEnum.MergeConflict) {
        showModalConflicts.value = true
      }
      loadingState.value = false
    })
    .catch((error) => {
      loadingState.value = false
      const errorLines = String(error).split("\n")
      toast.error(errorLines.join("<br/>"))
    })
}

const push = () => {
  loadingState.value = true
  pushWithConflict(false)
    .then((result) => {
      console.log("gitPush", result)
      if (result.status === GResultEnum.Success) {
        toast.success(result.result)
        initGitBranches()
      } else if (result.status === GResultEnum.MergeConflict) {
        showModalConflicts.value = true
      } else if (result.status === GResultEnum.RemoteAHead) {
        toast.info(
          "Local is lagging behind remote. Please update before pushing!"
        )
      }
      loadingState.value = false
    })
    .catch((error) => {
      console.log("gitPush", error)
      toast.error(error)
      loadingState.value = false
    })
}

const installGit = () => {
  open("https://git-scm.com/")
}

const showCommit = () => {
  invokeAction("flyouts.git-commit.toggle")
}

const showConflicts = () => {
  showModalConflicts.value = true
}

const gitClone = () => {
  showModalClone.value = true
}

const newBranch = () => {
  showNewBranch.value = true
}

const gitDefineRemote = () => {
  showModalDefineRemote.value = true
}

defineActionHandler("git-extension.show-conflicts", () => {
  showConflicts()
})
defineActionHandler("git-extension.init-extension-data", () => {
  toast.info("git-extension.init-extension-data")
  checkGitStatus()
  persistenceService.setupLocalPersistence()
})

const tippyActions = ref<TippyComponent | null>(null)
</script>

<style scoped>
.pd-0 :deep(.mr-4) {
  margin-right: 2px !important;
}
</style>
