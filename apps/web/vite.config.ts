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

// Where `deno task dev:remote` points when nothing overrides it: the project's
// own published tile deployment. Override with VITE_TILE_PROXY_TARGET — which
// .env.example now pre-fills with this exact value, so updating that file
// alone (no source edit) is enough going forward; this constant only exists
// so dev:remote still works before anyone has copied .env.example to .env.
// (Previously pointed at the workers.dev subdomain, which silently 404'd on
// every request once that route was disabled at the Cloudflare edge —
// nothing here ever surfaced that a hardcoded domain had gone stale.)
const tileOriginDefault = "https://plei.me";
const tileVersionDefault = "v1";

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

  // A production build is never a development build, whatever .env says.
  // VITE_APP_ENV lives in .env, which Vite loads for *every* mode — so a
  // developer's local `VITE_APP_ENV=development` rode along into the deployed
  // bundle and shipped the debug settings section and the dev routes with it.
  // Observed in production before this guard existed.
  //
  // The VITE_APP_ENV half is still honoured in the other direction: running
  // the dev server with VITE_APP_ENV=production turns dev-only surfaces off
  // locally, which is the point of having the variable at all. Only the
  // "development" value is overridden, so a production build labelled
  // `staging` still says staging.
  //
  // The override lands on the NAME, not on a separate boolean, and isDev is
  // derived from it. It used to be the other way round — the guard was
  // written into isDev alone while __APP_ENV__ kept the raw value — and the
  // two then disagreed in exactly the way this was meant to prevent: the
  // deployed settings panel read "development" while the debug section it
  // implies was correctly absent. One value, one rule, nothing left to
  // contradict.
  const rawAppEnv = env.VITE_APP_ENV || mode;
  const appEnv = mode === "production" && rawAppEnv === "development" ? "production" : rawAppEnv;
  const isDev = appEnv === "development";

  const apiBase = env.VITE_API_BASE_URL || apiBaseDefault;

  // Static tile origin for the serverless deployment: the pre-baked tiles
  // (apps/api/src/bake/terrain/tools/bake_tiles.ts) live in an object store
  // and are fetched directly, with no API server anywhere in the request
  // path. Left empty in dev, where the client keeps hitting the live
  // /api route through the proxy below and tiles are baked on demand.
  // Where tiles come from during development. One knob rather than two,
  // because the two settings it drives have to move together — pointing the
  // client at /tiles without also proxying that path just 404s, and the pair
  // being manually maintained in .env is exactly the kind of thing that ends
  // up half-applied. `deno task dev:remote` sets this.
  //
  //   local  (default) — the live API bakes on demand. Required for bake work.
  //   remote           — an already-published deployment, proxied. No Docker,
  //                      no PostGIS, no API process.
  //
  // Proxied rather than fetched cross-origin on purpose: requests stay
  // same-origin from the browser's point of view, so VITE_TILE_BASE_URL keeps
  // the relative form it has in production and the Worker needs no CORS
  // headers. Nothing about the deployed configuration is loosened to make
  // local development easier.
  //
  // A published prefix is frozen, so changes to the baking pipeline never show
  // up in remote mode. That same property makes it good for render work, where
  // a fixed input is what you want.
  const remoteTiles = (env.VITE_TILE_SOURCE || "local") === "remote";

  // Derived rather than required. The deployment serves tiles from its own
  // origin under /tiles/<version> (see apps/web/worker/index.ts), so a
  // production build that had to be told this could only ever be told the
  // same thing — or forgotten, which fails as tiles 404ing against an API
  // that is not deployed. Remote dev mode resolves to the same path for the
  // same reason. An explicit value still wins, for a host that differs.
  const tileVersion = env.VITE_TILE_VERSION || tileVersionDefault;
  const tileBase = env.VITE_TILE_BASE_URL ||
    (remoteTiles || mode === "production" ? `/tiles/${tileVersion}` : "");

  // Reverse geocoding origin. Defaults to OSM's shared public Nominatim;
  // see lib/env/config.ts for why this is configurable at all.
  const geocodeBase = env.VITE_GEOCODE_BASE_URL || "https://nominatim.openstreetmap.org";

  // Where tiles come from during development. One knob rather than two,
  // because the two settings it drives have to move together — pointing the
  // client at /tiles without also proxying that path just 404s, and the pair
  // being manually maintained in .env is exactly the kind of thing that ends
  // up half-applied. `deno task dev:remote` sets this.
  //
  //   local  (default) — the live API bakes on demand. Required for bake work.
  //   remote           — an already-published deployment, proxied. No Docker,
  //                      no PostGIS, no API process.
  //
  // Proxied rather than fetched cross-origin on purpose: requests stay
  // same-origin from the browser's point of view, so VITE_TILE_BASE_URL keeps
  // the relative form it has in production and the Worker needs no CORS
  // headers. Nothing about the deployed configuration is loosened to make
  // local development easier.
  //
  // A published prefix is frozen, so changes to the baking pipeline never show
  // up in remote mode. That same property makes it good for render work, where
  // a fixed input is what you want.
  const tileProxyTarget = remoteTiles ? (env.VITE_TILE_PROXY_TARGET || tileOriginDefault) : "";

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
        ...(tileProxyTarget
          ? {
            "/tiles": {
              target: tileProxyTarget,
              changeOrigin: true,
            },
          }
          : {}),
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

      __TILE_BASE_URL__: JSON.stringify(tileBase),
      __GEOCODE_BASE_URL__: JSON.stringify(geocodeBase),

      /** When this bundle was built, ISO-8601 in UTC. Evaluated here, so it
       * updates itself on every build with nothing to remember — which is
       * the whole point: a date someone has to bump by hand is a date that
       * goes stale silently.
       *
       * It does mean the bundle hash changes on every build even when no
       * source did, since this string is part of the output. That is the
       * cost of the thing being accurate rather than approximately right,
       * and it was chosen knowingly over stamping the commit date instead.
       *
       * UTC, not local: the value is formatted for the reader's own
       * timezone at the point it is displayed (see about_section.tsx).
       * Baking a timezone in here would be baking in the builder's. */
      __BUILD_TIME__: JSON.stringify(new Date().toISOString()),

      /** A droplet fades. A pixelet remains. 🫧 */
      __APP_NAME__: JSON.stringify("pixelet"),

      __ROLE__: JSON.stringify("client"),
    },
  };
});
