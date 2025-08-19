<template>
  <HoppSmartSlideOver :show="show" title="Commit" @close="close()">
    <template #content>
      <textarea
        v-model="commitMessage"
        rows="5"
        cols="10"
        style="margin: 10px"
        wrap="soft"
        placeholder="Please enter a submit message"
        class="rounded-lg border-black"
      ></textarea>
      <span
        class="flex space-x-2"
        style="justify-content: center; width: 400px"
      >
        <HoppButtonPrimary
          label="Commit"
          :loading="loadingStateCommit"
          outline
          style="width: 150px"
          @click="commit"
        />
        <HoppButtonSecondary
          label="Commit And Push"
          :loading="loadingStateCommitAndPush"
          outline
          filled
          style="width: 150px"
          @click="gitCommitAndPush"
        />
      </span>
      <div
        ref="tippyActions"
        class="flex flex-col focus:outline-none"
        style="min-width: 200px; margin-top: 20px"
      >
        <div class="pb-2 pl-4 text-sm font-bold text-secondaryLight">
          The last 15 commit times
        </div>

        <HoppSmartItem
          v-for="c in commitHistorys"
          :key="c.hash"
          :label="formatLabel(c)"
          :title="formatTitle(c)"
          :icon="IconGitVommitVertical"
        ></HoppSmartItem>
      </div>
    </template>
  </HoppSmartSlideOver>
</template>

<script setup lang="ts">
import { ref, watch } from "vue"
import { useI18n } from "@composables/i18n"
import { useToast } from "~/composables/toast"
import {
  gitCommit,
  listGitCommitHistory,
  CommitInfo,
  gitSync,
  GResultEnum,
} from "@helpers/git"
import IconGitVommitVertical from "~icons/lucide/git-commit-vertical"
import { invokeAction } from "@helpers/actions"

const t = useI18n()
const toast = useToast()

const commitHistorys = ref<CommitInfo[]>([])
const loadingStateCommit = ref(false)
const loadingStateCommitAndPush = ref(false)
const commitMessage = ref("")

const props = defineProps<{
  show: boolean
}>()
watch(
  () => props.show,
  (newVal) => {
    if (newVal) {
      commitMessage.value = ""
      init()
    }
  }
)

const init = () => {
  listGitCommitHistory(15)
    .then((result) => {
      // console.log(result)
      commitHistorys.value = result
    })
    .catch((error) => {
      toast.error(error)
    })
}

// init()

const validate = async () => {
  if (!commitMessage.value || commitMessage.value.length < 5) {
    toast.error(`${t("git.commit_message_error_message")}`)
    return false
  }
  return true
}

const commit = () => {
  ;(async () => {
    loadingStateCommit.value = true
    if (!(await validate())) {
      loadingStateCommit.value = false
      return
    }
    gitCommit(commitMessage.value)
      .then(() => {
        toast.success(`${t("git.commit_success_message")}`)
        loadingStateCommit.value = false
        init()
        close()
      })
      .catch((error) => {
        toast.error(error)
        loadingStateCommit.value = false
      })
      .finally(() => {
        invokeAction("git-extension.init-extension-data")
      })
  })()
}

const gitCommitAndPush = () => {
  ;(async () => {
    loadingStateCommitAndPush.value = true
    if (!(await validate())) {
      loadingStateCommitAndPush.value = false
      return
    }
    gitSync(commitMessage.value, false)
      .then((result) => {
        if (result.status === GResultEnum.Success) {
          toast.success(result.result)
        } else if (result.status === GResultEnum.MergeConflict) {
          toast.info(result.status)
          invokeAction("git-extension.show-conflicts")
        }
      })
      .catch((error) => {
        toast.error(error)
      })
      .finally(() => {
        loadingStateCommitAndPush.value = false
        init()
        invokeAction("git-extension.init-extension-data")
      })
  })()
}

const close = () => {
  invokeAction("flyouts.git-commit.toggle")
}

const formatLabel = (commit: CommitInfo) => {
  return `${commit.message.length > 20 ? commit.message.slice(0, 15) + "..." : commit.message} ${commit.author}`
}

const formatTitle = (commit: CommitInfo) => {
  return `commitId: ${commit.hash}\n\nmessage: ${commit.message}\n\ndate: ${commit.date}\n\nauthor: ${commit.author}`
}
</script>

<style scoped>
textarea {
  resize: vertical;
  overflow: auto;
  padding: 10px;
  margin-top: 10px;
  word-wrap: break-word;
  white-space: pre-wrap;
}
</style>
