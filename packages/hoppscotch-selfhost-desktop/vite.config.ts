import { defineConfig, loadEnv, normalizePath } from "vite"
import { META_TAGS } from "./meta"
import { viteStaticCopy as StaticCopy } from "vite-plugin-static-copy"
import HtmlConfig from "vite-plugin-html-config"
import Vue from "@vitejs/plugin-vue"
import VueI18n from "@intlify/vite-plugin-vue-i18n"
import Components from "unplugin-vue-components/vite"
import Icons from "unplugin-icons/vite"
import Inspect from "vite-plugin-inspect"
import Pages from "vite-plugin-pages"
import Layouts from "vite-plugin-vue-layouts"
import IconResolver from "unplugin-icons/resolver"
import { FileSystemIconLoader } from "unplugin-icons/loaders"
import * as path from "path"
import legacy from "@vitejs/plugin-legacy"
import Unfonts from "unplugin-fonts/vite"

const ENV = loadEnv("development", path.resolve(__dirname, "../../"))

export default defineConfig({
  envDir: path.resolve(__dirname, "../../"),
  // TODO: Migrate @hoppscotch/data to full ESM
  define: {
    // For 'util' polyfill required by dep of '@apidevtools/swagger-parser'
    "process.env": {},
  },
  server: {
    port: 3000,
  },
  preview: {
    port: 3000,
  },
  publicDir: path.resolve(__dirname, "../hoppscotch-common/public"),
  build: {
    sourcemap: true,
    emptyOutDir: true,
    rollupOptions: {
      maxParallelFileOps: 2,
    },
  },
  resolve: {
    alias: {
      // TODO: Maybe leave ~ only for individual apps and not use on common
      "~": path.resolve(__dirname, "../hoppscotch-common/src"),
      "@hoppscotch/common": "@hoppscotch/common/src",
      "@composables": path.resolve(
        __dirname,
        "../hoppscotch-common/src/composables"
      ),
      "@modules": path.resolve(__dirname, "../hoppscotch-common/src/modules"),
      "@components": path.resolve(
        __dirname,
        "../hoppscotch-common/src/components"
      ),
      "@helpers": path.resolve(__dirname, "../hoppscotch-common/src/helpers"),
      "@functional": path.resolve(
        __dirname,
        "../hoppscotch-common/src/helpers/functional"
      ),
      "@workers": path.resolve(__dirname, "../hoppscotch-common/src/workers"),
      "@platform": path.resolve(__dirname, "./src/platform"),
      "@lib": path.resolve(__dirname, "./src/lib"),
      stream: "stream-browserify",
      util: "util",
    },
    dedupe: ["vue"],
  },
  plugins: [
    Inspect(), // go to url -> /__inspect
    HtmlConfig({
      metas: META_TAGS(),
    }),
    Vue(),
    Pages({
      routeStyle: "nuxt",
      dirs: "../hoppscotch-common/src/pages",
      importMode: "async",
    }),
    StaticCopy({
      targets: [
        {
          src: normalizePath(path.resolve(__dirname, "./.sitemap-gen/*")),
          dest: normalizePath(path.resolve(__dirname, "./dist")),
        },
      ],
    }),
    Layouts({
      layoutsDirs: "../hoppscotch-common/src/layouts",
      defaultLayout: "default",
    }),
    VueI18n({
      runtimeOnly: false,
      compositionOnly: true,
      include: [path.resolve(__dirname, "locales")],
    }),
    Components({
      dts: "../hoppscotch-common/src/components.d.ts",
      dirs: ["../hoppscotch-common/src/components", "./src/components"],
      directoryAsNamespace: true,
      resolvers: [
        IconResolver({
          prefix: "icon",
          customCollections: ["hopp", "auth", "brands"],
        }),
        (compName: string) => {
          if (compName.startsWith("Hopp"))
            return { name: compName, from: "@mamahuhu/ui" }
          else return undefined
        },
      ],
      types: [
        {
          from: "vue-tippy",
          names: ["Tippy"],
        },
      ],
    }),
    Icons({
      compiler: "vue3",
      customCollections: {
        hopp: FileSystemIconLoader("../hoppscotch-common/assets/icons"),
        auth: FileSystemIconLoader("../hoppscotch-common/assets/icons/auth"),
        brands: FileSystemIconLoader(
          "../hoppscotch-common/assets/icons/brands"
        ),
      },
    }),
    Unfonts({
      fontsource: {
        families: [
          {
            name: "Inter Variable",
            variables: ["variable-full"],
          },
          {
            name: "Material Symbols Rounded Variable",
            variables: ["variable-full"],
          },
          {
            name: "Roboto Mono Variable",
            variables: ["variable-full"],
          },
        ],
      },
    }),
    legacy({
      modernPolyfills: ["es.string.replace-all"],
      renderLegacyChunks: false,
    }),
  ],
})
