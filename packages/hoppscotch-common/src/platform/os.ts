import { ref, computed, ComputedRef } from "vue"
import { type } from "@tauri-apps/plugin-os"

// 导出接口定义
export interface PlatformOSInfo {
  isLinux: ComputedRef<boolean>
  isMacOS: ComputedRef<boolean>
  isWindows: ComputedRef<boolean>
}

// 创建响应式的平台信息
export function createPlatformOS(): PlatformOSInfo {
  const osType = ref<"linux" | "macos" | "windows" | "web">("web")

  // 初始化平台信息
  const initPlatformInfo = async () => {
    try {
      const currentType = await type()
      osType.value = currentType as "linux" | "macos" | "windows"
    } catch {
      console.debug("Not in Tauri environment, using web platform info")
    }
  }

  // 创建计算属性
  const isLinux = computed(() => osType.value === "linux")
  const isMacOS = computed(() => osType.value === "macos")
  const isWindows = computed(() => osType.value === "windows")

  // 初始化并返回
  initPlatformInfo()

  return {
    isLinux,
    isMacOS,
    isWindows,
  }
}
