// @pxl/api/../vite.config.ts

import { defineConfig, loadEnv } from "vite";
import deno from "@deno/vite-plugin";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, Deno.cwd(), "");

  const appEnv = env.APP_ENV || mode;
  const isDev = appEnv === "development";

  const buildTarget = env.VITE_BUILD_TARGET || "main";

  let ssr: string;
  let entryFileNames: string;

  switch (buildTarget) {
    case "worker":
      ssr = "./src/xpute/worker/worker.ts";
      entryFileNames = "worker.js";
      break;

    case "main":
    default:
      ssr = "./src/index.ts";
      entryFileNames = "index.js";
      break;
  }

  return {
    plugins: [deno()],

    resolve: {
      alias: { "@": path.resolve(__dirname, "src") },
    },

    build: {
      target: "esnext",
      minify: "esbuild",
      sourcemap: mode !== "production",

      outDir: "dist",
      emptyOutDir: false,

      ssr,

      rollupOptions: {
        output: {
          entryFileNames,
          format: "es",
        },
      },
    },

    define: {
      __ENV__: JSON.stringify("deno"),
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),

      /** A droplet fades. A pixelet remains. 🫧 */
      __APP_NAME__: JSON.stringify("pixelet"),

      __ROLE__: JSON.stringify("server"),
    },
  };
});
