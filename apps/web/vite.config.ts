// @pxl/web/../vite.config.ts

import { defineConfig, loadEnv } from "vite";
import deno from "@deno/vite-plugin";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import svgr from "vite-plugin-svgr";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const apiBaseDefault = "http://127.0.0.1:3000";

// anti-HMR
const corePathIncludes = ["/packages/pxl/shared/", "/packages/xpute/core/"];
function fullReloadOnCoreChange() {
  function isCoreFile(file: string): boolean {
    const p = file.replaceAll("\\", "/");
    // monorepo packages are treated as "core": always full reload on change
    for (const pat of corePathIncludes) {
      if (p.includes(pat)) return true;
    }
    return false;
  }

  return {
    name: "full-reload-core",
    enforce: "pre" as const,
    handleHotUpdate(ctx: any) {
      const file: string = ctx.file ?? "";
      if (!file) return [];

      if (isCoreFile(file)) {
        ctx.server.ws.send({ type: "full-reload" });
        return [];
      }
      return [];
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, Deno.cwd(), "VITE_");

  const appEnv = env.VITE_APP_ENV || mode;
  const isDev = appEnv === "development";

  const apiBase = env.VITE_API_BASE_URL || apiBaseDefault;

  return {
    plugins: [
      deno(),
      react(),
      tailwindcss(), // Tailwind
      svgr({ svgrOptions: { titleProp: true } }), // SVGR

      // monorepo core packages change => force full reload (no HMR)
      fullReloadOnCoreChange(),
    ],

    resolve: {
      alias: { "@": path.resolve(__dirname, "src") },
    },

    build: {
      target: "esnext",
      minify: "esbuild",
      sourcemap: mode !== "production",
    },

    server: {
      host: true,
      proxy: {
        "/api": {
          target: apiBase,
          changeOrigin: true,
        },
        "/io": {
          target: apiBase,
          changeOrigin: true,
          ws: true,
        },
      },
    },

    define: {
      __ENV__: JSON.stringify("browser"),
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),

      /** A droplet fades. A pixelet remains. 🫧 */
      __APP_NAME__: JSON.stringify("pixelet"),

      __ROLE__: JSON.stringify("client"),
    },
  };
});
