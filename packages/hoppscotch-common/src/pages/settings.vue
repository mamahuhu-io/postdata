<template>
  <div>
    <div class="container divide-y divide-dividerLight">
      <div class="md:grid md:grid-cols-3 md:gap-4">
        <div class="p-8 md:col-span-1">
          <h3 class="heading">
            {{ t("settings.general") }}
          </h3>
          <p class="my-1 text-secondaryLight">
            {{ t("settings.general_description") }}
          </p>
        </div>
        <div class="space-y-8 p-8 md:col-span-2">
          <section>
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.language") }}
            </h4>
            <div class="mt-4">
              <SmartChangeLanguage />
            </div>
          </section>

          <section>
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.query_parameters_encoding") }}
            </h4>
            <div class="my-1 text-secondaryLight">
              {{ t("settings.query_parameters_encoding_description") }}
            </div>
            <div class="mt-4">
              <SmartEncodingPicker />
            </div>
          </section>

          <section>
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.experiments") }}
            </h4>
            <div class="my-1 text-secondaryLight">
              {{ t("settings.experiments_notice") }}
              <HoppSmartAnchor
                class="link"
                to="https://github.com/mamahuhu-io/postdata/issues/new/choose"
                blank
                :label="t('app.contact_us')"
              />.
            </div>
            <div class="space-y-4 py-4">
              <div class="flex items-center">
                <HoppSmartToggle
                  v-if="hasPlatformTelemetry"
                  :on="TELEMETRY_ENABLED"
                  @change="showConfirmModal"
                >
                  {{ t("settings.telemetry") }}
                </HoppSmartToggle>
              </div>
              <div class="flex items-center">
                <HoppSmartToggle
                  :on="EXPAND_NAVIGATION"
                  @change="toggleSetting('EXPAND_NAVIGATION')"
                >
                  {{ t("settings.expand_navigation") }}
                </HoppSmartToggle>
              </div>
              <div class="flex items-center">
                <HoppSmartToggle
                  :on="SIDEBAR_ON_LEFT"
                  @change="toggleSetting('SIDEBAR_ON_LEFT')"
                >
                  {{ t("settings.sidebar_on_left") }}
                </HoppSmartToggle>
              </div>
              <div v-if="hasAIExperimentsSupport" class="flex items-center">
                <HoppSmartToggle
                  :on="ENABLE_AI_EXPERIMENTS"
                  @change="toggleSetting('ENABLE_AI_EXPERIMENTS')"
                >
                  {{ t("settings.ai_experiments") }}
                </HoppSmartToggle>
              </div>
            </div>
          </section>

          <section>
            <h4 class="font-semibold text-secondaryDark">
              Data Storage Preferences
            </h4>
            <div class="mt-4">
              <SmartChangeDataStorge />
            </div>
          </section>
        </div>
      </div>

      <div class="md:grid md:grid-cols-3 md:gap-4">
        <div class="p-8 md:col-span-1">
          <h3 class="heading">
            {{ t("settings.theme") }}
          </h3>
          <p class="my-1 text-secondaryLight">
            {{ t("settings.theme_description") }}
          </p>
        </div>
        <div class="space-y-8 p-8 md:col-span-2">
          <section v-if="!isLinux">
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.background") }}
            </h4>
            <div class="my-1 text-secondaryLight">
              {{ t(getColorModeName(colorMode.preference)) }}
              <span v-if="colorMode.preference === 'system'">
                ({{ t(getColorModeName(colorMode.value)) }})
              </span>
            </div>
            <div class="mt-4">
              <SmartColorModePicker />
            </div>
          </section>
          <section>
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.accent_color") }}
            </h4>
            <div class="my-1 text-secondaryLight">
              {{ ACCENT_COLOR.charAt(0).toUpperCase() + ACCENT_COLOR.slice(1) }}
            </div>
            <div class="mt-4">
              <SmartAccentModePicker />
            </div>
          </section>
        </div>
      </div>

      <div class="md:grid md:grid-cols-3 md:gap-4">
        <div class="p-8 md:col-span-1">
          <h3 class="heading">
            {{ t("settings.interceptor") }}
          </h3>
          <p class="my-1 text-secondaryLight">
            {{ t("settings.interceptor_description") }}
          </p>
        </div>
        <div class="space-y-8 p-8 md:col-span-2">
          <section class="flex flex-col space-y-2">
            <h4 class="font-semibold text-secondaryDark">
              {{ t("settings.interceptor") }}
            </h4>
            <AppInterceptor :is-tooltip-component="false" />
          </section>
          <section v-for="[id, settings] in interceptorsWithSettings" :key="id">
            <h4 class="font-semibold text-secondaryDark">
              {{ settings.entryTitle(t) }}
            </h4>
            <component :is="settings.component" />
          </section>
        </div>
      </div>

      <template v-if="platform.ui?.additionalSettingsSections?.length">
        <template
          v-for="item in platform.ui?.additionalSettingsSections"
          :key="item.id"
        >
          <component :is="item" />
        </template>
      </template>
    </div>
    <HoppSmartConfirmModal
      :show="confirmRemove"
      :title="`${t('confirm.remove_telemetry')} ${t(
        'settings.telemetry_helps_us'
      )}`"
      @hide-modal="confirmRemove = false"
      @resolve="
        () => {
          toggleSetting('TELEMETRY_ENABLED')
          confirmRemove = false
        }
      "
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue"
import { applySetting, toggleSetting } from "~/newstore/settings"
import { useSetting } from "@composables/settings"
import { useI18n } from "@composables/i18n"
import { useColorMode } from "@composables/theming"
import { usePageHead } from "@composables/head"
import { useService } from "dioc/vue"
import { InterceptorService } from "~/services/interceptor.service"
import { pipe } from "fp-ts/function"
import * as O from "fp-ts/Option"
import * as A from "fp-ts/Array"
import { platform } from "~/platform"

const t = useI18n()
const colorMode = useColorMode()

const { isLinux } = platform.platformOS

usePageHead({
  title: computed(() => t("navigation.settings")),
})

const interceptorService = useService(InterceptorService)
const interceptorsWithSettings = computed(() =>
  pipe(
    interceptorService.availableInterceptors.value,
    A.filterMap((interceptor) =>
      interceptor.settingsPageEntry
        ? O.some([
            interceptor.interceptorID,
            interceptor.settingsPageEntry,
          ] as const)
        : O.none
    )
  )
)

const ACCENT_COLOR = useSetting("THEME_COLOR")
const PROXY_URL = useSetting("PROXY_URL")
const TELEMETRY_ENABLED = useSetting("TELEMETRY_ENABLED")
const EXPAND_NAVIGATION = useSetting("EXPAND_NAVIGATION")
const SIDEBAR_ON_LEFT = useSetting("SIDEBAR_ON_LEFT")
const ENABLE_AI_EXPERIMENTS = useSetting("ENABLE_AI_EXPERIMENTS")

const hasPlatformTelemetry = Boolean(platform.platformFeatureFlags.hasTelemetry)

const confirmRemove = ref(false)

const proxySettings = computed(() => ({
  url: PROXY_URL.value,
}))

const hasAIExperimentsSupport =
  !!platform.experiments?.aiExperiments?.enableAIExperiments

watch(
  proxySettings,
  ({ url }) => {
    applySetting("PROXY_URL", url)
  },
  { deep: true }
)

const showConfirmModal = () => {
  if (TELEMETRY_ENABLED.value) confirmRemove.value = true
  else toggleSetting("TELEMETRY_ENABLED")
}

const getColorModeName = (colorMode: string) => {
  switch (colorMode) {
    case "system":
      return "settings.system_mode"
    case "light":
      return "settings.light_mode"
    case "dark":
      return "settings.dark_mode"
    case "black":
      return "settings.black_mode"
    default:
      return "settings.system_mode"
  }
}
</script>
