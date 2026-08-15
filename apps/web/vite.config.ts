// @pxl/web/../vite.config.ts

import { defineConfig, type HmrContext, loadEnv } from "vite";
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
    handleHotUpdate(ctx: HmrContext) {
      const file = ctx.file;

      if (isCoreFile(file)) {
        ctx.server.ws.send({ type: "full-reload" });
        return [];
      }
      // Returning [] here (even though it's "empty") tells Vite "no
      // modules to update" for EVERY non-core file too, since Vite treats
      // any array return — including [] — as truthy and overwrites
      // ctx.modules with it. That silently disabled HMR for the entire
      // app. Returning nothing lets Vite fall through to its default
      // HMR behavior for anything that isn't a core-package file.
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
    // root `deno task dev` runs dev:api and dev:web concurrently in one
    // terminal (`sh -c "(deno task dev:api) & (deno task dev:web) & wait"`).
    // Vite's default clearScreen wipes the whole terminal — including the
    // API server's output — on every restart/error.
    clearScreen: false,

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
      // Vite's default fs.allow walks up from this config's own directory
      // looking for an npm/pnpm/yarn workspace root — this repo is a Deno
      // workspace, which that heuristic doesn't recognize, so it fell back
      // to just apps/web itself. That silently 403'd any `/@fs/` request
      // for a file outside apps/web — invisible for plain .ts imports
      // (those go through the deno plugin's own resolution, not this gate)
      // but broke real asset imports (import x from "*.png") the moment
      // any of them started pointing into packages/pxl/shared instead of
      // staying under apps/web, e.g. objects/wool_images.ts and
      // terrain/shader/ground_layer_images.ts's own textures/{terrain,
      // objects}/ imports. Explicitly allowing the repo root fixes every
      // such path at once instead of allow-listing each package.
      fs: { allow: [path.resolve(__dirname, "..", "..")] },
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
