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

  return {
    plugins: [deno()],

    resolve: {
      alias: { "@": path.resolve(__dirname, "src") },
    },

    build: {
      target: "esnext",
      minify: "esbuild",
      sourcemap: mode !== "production",

      outDir: "dist/main",
      emptyOutDir: true,

      ssr: "./src/index.ts",

      rollupOptions: {
        output: {
          entryFileNames: "server.js",
          format: "es",
        },
      },
    },

    define: {
      __ENV__: JSON.stringify("deno"),
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),
      __APP_NAME__: JSON.stringify("pixelet"),
      __ROLE__: JSON.stringify("server"),
    },
  };
});
