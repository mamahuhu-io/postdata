<template>
  <HoppSmartModal
    v-if="show"
    dialog
    title="Resolve Conflicts"
    @close="hideModal"
  >
    <template #body>
      <p class="leading-5 text-sm h-20">
        1. Modifying the same file locally and remotely may cause conflicts<br />
        2. Conflicts occurred when attempting to merge<br />
        3. If keep is remote, local files that are overwritten will be backup<br />
        <span style="padding-left: 18px" class="text-zinc-500 text-xs"
          >Backup directory: backup/&lt;timestamp&gt;</span
        >
      </p>
      <HoppSmartRadioGroup v-model="strategySelection" :radios="strategys" />
    </template>
    <template #footer>
      <span class="flex space-x-2">
        <HoppButtonPrimary
          label="Submit"
          :loading="loadingState"
          outline
          @click="submit"
        />
        <HoppButtonSecondary
          :label="t('action.cancel')"
          outline
          filled
          @click="hideModal"
        />
      </span>
    </template>
  </HoppSmartModal>
</template>

<script setup lang="ts">
import { watch, ref, computed } from "vue"
import { useToast } from "@composables/toast"
import { useI18n } from "@composables/i18n"
import { HoppButtonPrimary } from "@mamahuhu/ui"
import { resolveConflicts, GResultEnum } from "@helpers/git"
import { useService } from "dioc/vue"
import { PersistenceService } from "~/services/persistence"

const toast = useToast()
const t = useI18n()
const loadingState = ref(false)
const persistenceService = useService(PersistenceService)

const strategys = computed(() => [
  { value: "Local" as const, label: "Local: Keep local changes" },
  { value: "Remote" as const, label: "Remote: Keep remote changes" },
  { value: "Manual" as const, label: "Manual: Resolve manually" },
])

type StrategyMode = (typeof strategys)["value"][number]["value"]

const strategySelection = ref<StrategyMode>("Local")

const props = withDefaults(
  defineProps<{
    show: boolean
  }>(),
  {
    show: false,
  }
)

const emit = defineEmits<{
  (e: "init"): void
  (e: "hide-modal"): void
}>()

watch(
  () => props.show,
  (show) => {
    if (!show) {
      strategySelection.value = "Local"
      hideModal()
    }
  }
)

const hideModal = () => {
  strategySelection.value = "Local"
  emit("hide-modal")
}

const submit = () => {
  if (loadingState.value) {
    return
  }

  if (strategySelection.value === "Manual") {
    toast.info("Resolve conflicts manually, do nothing this time！")
    return
  }

  loadingState.value = true
  resolveConflicts(strategySelection.value)
    .then((result) => {
      if (result.status === GResultEnum.Success) {
        toast.success(result.result)
        hideModal()
      }
    })
    .catch((error) => {
      toast.error(error)
    })
    .finally(() => {
      persistenceService.setupLocalPersistence()
      loadingState.value = false
    })
}
</script>
