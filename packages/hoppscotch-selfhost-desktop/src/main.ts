import { createHoppApp } from "@hoppscotch/common"
import { proxyInterceptor } from "@hoppscotch/common/platform/std/interceptors/proxy"
import { NativeInterceptorService } from "./platform/interceptors/native"
import { nextTick, ref, watch } from "vue"
import { emit, listen } from "@tauri-apps/api/event"
import { type } from "@tauri-apps/plugin-os"
import { useSettingStatic } from "@hoppscotch/common/composables/settings"
import { WebviewWindow } from "@tauri-apps/api/webviewWindow"
import { stdFooterItems } from "@hoppscotch/common/platform/std/ui/footerItem"
import { stdSupportOptionItems } from "@hoppscotch/common/platform/std/ui/supportOptionsItem"
import { ioDef } from "./platform/io"
import { interopModule } from "./interop"
import { createPlatformOS } from "@hoppscotch/common/platform/os"

const headerPaddingLeft = ref("0px")
const headerPaddingTop = ref("0px")

;(async () => {
  const platform = await type()

  createHoppApp("#app", {
    ui: {
      additionalFooterMenuItems: stdFooterItems,
      additionalSupportOptionsMenuItems: stdSupportOptionItems,
      appHeader: {
        paddingLeft: headerPaddingLeft,
        paddingTop: headerPaddingTop,
        onHeaderAreaClick() {
          if (platform === "macos") {
            // Drag thw window when the user drags the header area
            // TODO: Ignore click on headers and fields
            WebviewWindow.getCurrent().startDragging()
          }
        },
      },
    },
    io: ioDef,
    addedHoppModules: [interopModule],
    interceptors: {
      default: "native",
      interceptors: [
        { type: "service", service: NativeInterceptorService },
        {
          type: "standalone",
          interceptor: { ...proxyInterceptor, supportsDigestAuth: true },
        },
      ],
    },
    platformFeatureFlags: {
      exportAsGIST: false,
      hasTelemetry: false,
      cookiesEnabled: true,
      promptAsUsingCookies: false,
    },
    platformOS: createPlatformOS(),
  })

  watch(
    useSettingStatic("BG_COLOR")[0],
    async () => {
      await nextTick()

      await emit(
        "hopp-bg-changed",
        getComputedStyle(document.documentElement).getPropertyValue(
          "--primary-color"
        )
      )
    },
    { immediate: true }
  )

  if (platform === "macos") {
    listen("will-enter-fullscreen", () => {
      headerPaddingTop.value = "0px"
      headerPaddingLeft.value = "0px"
    })

    listen("will-exit-fullscreen", () => {
      headerPaddingTop.value = "2px"
      headerPaddingLeft.value = "70px"
    })

    headerPaddingTop.value = "2px"
    headerPaddingLeft.value = "70px"
  }
})()

function isTextInput(target: EventTarget | null) {
  if (target instanceof HTMLInputElement) {
    return (
      target.type === "text" ||
      target.type === "email" ||
      target.type === "password" ||
      target.type === "number" ||
      target.type === "search" ||
      target.type === "tel" ||
      target.type === "url" ||
      target.type === "textarea"
    )
  } else if (target instanceof HTMLTextAreaElement) {
    return true
  } else if (target instanceof HTMLElement && target.isContentEditable) {
    return true
  }

  return false
}

window.addEventListener(
  "keydown",
  function (e) {
    if (e.key === "Backspace" && !isTextInput(e.target)) {
      e.preventDefault()
    }
  },
  true
)
