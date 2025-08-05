<template>
  <HoppSmartModal v-if="show" dialog @close="hideModal">
    <template #body>
      <div class="w-full mx-auto text-sm">
        <p class="w-1/2 mx-auto"><AppLogo class="w-28 h-28 mx-auto" /></p>
        <p class="text-center p-2 mx-auto font-extrabold"></p>
        <p
          v-if="!hasUpgrade"
          class="w-full text-center p-1 mx-auto font-medium flex items-center justify-center"
        >
          <HoppSmartSpinner v-if="checkIngState" />
          <span v-if="!checkIngState && !hasUpgrade && !hasUpdateError">
            There are currently no updates available.
          </span>
          <span
            v-if="!checkIngState && !hasUpgrade && hasUpdateError"
            class="text-rose-600"
          >
            Check update failed, please check network!
          </span>
        </p>
        <div v-if="hasUpgrade">
          <p class="w-full text-center p-1 mx-auto font-medium">
            Current: {{ `v${version}` }}
            <span class="text-green-600">-></span>
            Latest:{{ `v${updateInfo.version}` }} <br />
          </p>
          <p
            class="w-full text-center p-1 mx-auto"
            style="padding: 0px 50px 0px 50px"
          >
            <a class="px-2">Update date: {{ updateInfo.date.split(".")[0] }}</a>
            <a
              class="px-2"
              target="_blank"
              href="https://github.com/mamahuhu-io/postdata/releases"
              >Update details</a
            >
          </p>
        </div>
        <div
          v-if="upgradeIngState"
          class="relative mb-0 mt-4 h-4 rounded-full bg-gray-200"
        >
          <div
            class="h-4 animate-pulse rounded-full bg-gradient-to-r from-green-500 to-blue-500"
            :style="{ width: upgradeProgress + '%' }"
          >
            <span
              class="absolute inset-0 flex items-center justify-center text-xs font-semibold text-white"
              >{{ upgradeProgress.toFixed(2) }}%</span
            >
          </div>
        </div>
      </div>
    </template>
    <template #footer>
      <span class="flex space-x-2">
        <HoppButtonSecondary
          :label="t('action.close')"
          outline
          filled
          style="width: 80px"
          @click="hideModal"
        />
        <HoppButtonPrimary
          v-if="hasUpgrade && !upgradeFinish"
          label="Upgrade"
          :loading="upgradeIngState"
          outline
          style="width: 80px"
          @click="upgrade"
        />
        <HoppButtonPrimary
          v-if="upgradeFinish"
          label="Restart"
          :loading="false"
          outline
          style="width: 80px"
          @click="relaunchApp"
        />
      </span>
    </template>
  </HoppSmartModal>
</template>

<script setup lang="ts">
import { version } from "~/../package.json"
import { useI18n } from "@composables/i18n"
import { check } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import { ref, watch } from "vue"
import { HoppSmartSpinner } from "@mamahuhu/ui"
import { useToast } from "~/composables/toast"

const t = useI18n()
const latestVersion = ref<string>("")
const checkIngState = ref<boolean>(true)
const hasUpgrade = ref<boolean>(false)
const updateInfo = ref<any>()
const hasUpdateError = ref<boolean>(false)
const upgradeIngState = ref<boolean>(false)
const upgradeFinish = ref<boolean>(false)
const upgradeProgress = ref<number>(1)

const toast = useToast()

const props = withDefaults(
  defineProps<{
    show: boolean
  }>(),
  {
    show: false,
    loadingState: false,
  }
)

watch(
  () => props.show,
  (newVal) => {
    if (newVal) {
      checkUpdate()
    } else {
      latestVersion.value = ""
      checkIngState.value = true
      hasUpgrade.value = false
      hasUpdateError.value = false
      upgradeIngState.value = false
      upgradeFinish.value = false
      upgradeProgress.value = 1
    }
  }
)

const emit = defineEmits<{
  (e: "hide-modal"): void
}>()

const hideModal = () => {
  emit("hide-modal")
}

const checkUpdate = async () => {
  checkIngState.value = true
  try {
    const update = await check()
    if (update) {
      console.log(
        `found update ${update.version} from ${update.date} with notes ${update.body}`
      )
      updateInfo.value = update
      latestVersion.value = update.version
      hasUpgrade.value = true
      checkIngState.value = false
    }
  } catch (e: any) {
    checkIngState.value = false
    hasUpdateError.value = true
    console.error(e)
  }
}

const upgrade = async () => {
  upgradeIngState.value = true
  try {
    const update = await check()
    if (update) {
      let downloaded = 0
      let contentLength = 0
      // alternatively we could also call update.download() and update.install() separately
      await update.downloadAndInstall((event: any) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength
            console.log(`started downloading ${event.data.contentLength} bytes`)
            break
          case "Progress":
            downloaded += event.data.chunkLength
            console.log(`downloaded ${downloaded} from ${contentLength}`)
            break
          case "Finished":
            console.log("download finished")
            break
        }
        upgradeProgress.value = (downloaded / contentLength) * 100
      })
      upgradeFinish.value = true
    }
  } catch (e: any) {
    console.error(e)
    toast.error("Upgrade failed, please check network!")
  }
}

const relaunchApp = async () => {
  await relaunch()
}
</script>
