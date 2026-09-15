// @pxl/web/../vite.config.ts

import { defineConfig, type HmrContext, loadEnv, type Plugin } from "vite";
import deno from "@deno/vite-plugin";
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import process from "node:process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import svgr from "vite-plugin-svgr";

import { DEV_PROXY_ROUTES } from "./src/lib/env/route.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const apiBaseDefault = "http://127.0.0.1:3000";

/**
 * Which source a build is: `r<commits>-<hash>`, and `-dirty` when the
 * working tree had uncommitted changes. The hash is the identity; the count
 * is only there so two builds read in order at a glance, and it is not
 * stable — a rewritten history renumbers it. Derived from git rather than a
 * counter kept in a file, which would have to be bumped by a commit of its
 * own and would count builds rather than source.
 */
function build_id(): string {
  try {
    const git = (args: string) => execSync(`git ${args}`, { cwd: __dirname, stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
    const dirty = git("status --porcelain") !== "" ? "-dirty" : "";
    return `r${git("rev-list --count HEAD")}-${git("rev-parse --short HEAD")}${dirty}`;
  } catch {
    return "unknown";
  }
}

/**
 * Which official build this is: `b<n>` when script/deploy.ts handed the
 * build its number, `b<n>+` for any other build after the last deploy this
 * checkout made, `b?` before one. The count lives in the bucket, not in git
 * (see deploy.ts); .build_number is only this checkout's copy of the last
 * number it published.
 */
function build_number(): string {
  const official = process.env.PXL_BUILD_NUMBER;
  if (official) return `b${official}`;
  try {
    const last = readFileSync(path.join(__dirname, ".build_number"), "utf8").trim();
    return /^\d+$/.test(last) ? `b${last}+` : "b?";
  } catch {
    return "b?";
  }
}

/**
 * `virtual:pxl-build` — what a build is: when it was built, which source,
 * which official number (lib/env/config.ts reads it). A module rather than
 * `define` constants so that a dev server gives every page load the values
 * as they are then, not as they were when the server started: the module is
 * dropped from the graph on each navigation and computed again on import,
 * so a ledger report taken from a long-running dev server names the source
 * actually on screen.
 *
 * The time is evaluated at build, so it updates itself with nothing to
 * remember — a date someone has to bump by hand goes stale silently. It
 * does mean the bundle hash changes on every build even when no source
 * did; that is the cost of it being accurate, chosen over the commit date.
 * UTC: it is formatted for the reader's own timezone where it is shown
 * (about_section.tsx).
 */
function build_stamp(): Plugin {
  const id = "virtual:pxl-build";
  const resolved = "\0" + id;
  return {
    name: "pxl-build-stamp",
    resolveId(source) {
      return source === id ? resolved : undefined;
    },
    load(source) {
      if (source !== resolved) return undefined;
      return `export const time = ${JSON.stringify(new Date().toISOString())};\n` +
        `export const id = ${JSON.stringify(build_id())};\n` +
        `export const number = ${JSON.stringify(build_number())};\n`;
    },
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        if (req.headers.accept?.includes("text/html")) {
          const mod = server.moduleGraph.getModuleById(resolved);
          if (mod) server.moduleGraph.invalidateModule(mod);
        }
        next();
      });
    },
  };
}

// Where `deno task dev:remote` points when nothing overrides it: the project's
// own published deployment. Override with VITE_REMOTE_ORIGIN — which
// .env.example now pre-fills with this exact value, so updating that file
// alone (no source edit) is enough going forward; this constant only exists
// so dev:remote still works before anyone has copied .env.example to .env.
// (Previously pointed at the workers.dev subdomain, which silently 404'd on
// every request once that route was disabled at the Cloudflare edge —
// nothing here ever surfaced that a hardcoded domain had gone stale.)
const remoteOriginDefault = "https://plei.me";

// anti-HMR. The shell's kernel glue is here too: kernel.ts boots the device
// and the world at module load, and its importers are component modules Vite
// hot-swaps, so a hot update ran that boot again against the one kernel the
// memory holds — a second device beside the renderer's objects on the first.
const corePathIncludes = ["/packages/pxl/shared/", "/packages/xpute/core/", "/apps/web/src/wasm/"];
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

    // Vite's watcher covers `root`, which here is apps/web — so nothing under
    // packages/ was ever watched, handleHotUpdate below never ran for a core
    // file, and the server went on serving the transform it had cached at
    // first import. Editing a core file appeared to do nothing at all, and a
    // browser reload did not help because the staleness was on the server.
    // Confirmed rather than guessed: a core edit produced no line in the dev
    // log, while an apps/web edit in the same second produced two.
    configureServer(server) {
      for (const pat of corePathIncludes) {
        server.watcher.add(path.resolve(__dirname, "..", "..", pat.replace(/^\/|\/$/g, "")));
      }
    },

    handleHotUpdate(ctx: HmrContext) {
      const file = ctx.file;

      if (isCoreFile(file)) {
        // Invalidate by hand, because Vite cannot make the connection
        // itself. The deno plugin resolves a core import to a virtual id —
        // `\0deno::TypeScript::@pxl/shared/...::/abs/path.ts` — and the
        // module graph is keyed by resolved id while onFileChange looks up
        // by file path, so a changed core file leaves its transform cached
        // under a name nothing matched. The full-reload below then fetched
        // the same stale module again, which is why editing a core file
        // looked like editing nothing at all and a browser reload did not
        // help. The absolute path is inside the virtual id, so matching on
        // it is enough to find them.
        for (const mod of ctx.server.moduleGraph.idToModuleMap.values()) {
          if (mod.id?.includes(file)) ctx.server.moduleGraph.invalidateModule(mod);
        }
        ctx.server.ws.send({ type: "full-reload" });
        // Vite prints "page reload" for the ones it decides itself and says
        // nothing about this one, so without a line here a core edit looks
        // exactly like a core edit that was never noticed — which is what it
        // was, before configureServer above put packages/ under the watcher.
        ctx.server.config.logger.info(`core reload ${path.relative(path.resolve(__dirname, "..", ".."), file)}`, { timestamp: true });
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

  //   local  (default) — the live API, baking on demand.
  //   remote            — an already-published deployment, so no local API,
  //                       database or Docker. `deno task dev:remote`.
  const remote = (env.VITE_DEV_SOURCE || "local") === "remote";

  // Injected as __PUBLISHED_TILES__: route.ts builds every tile path from
  // it, config.ts reads it for what a 404 means.
  const published = mode === "production" || remote;

  const remoteOrigin = remote ? (env.VITE_REMOTE_ORIGIN || remoteOriginDefault) : "";
  const proxy = Object.fromEntries(DEV_PROXY_ROUTES.map((route) => [
    route.path,
    remoteOrigin && !route.local_only ? { target: remoteOrigin, changeOrigin: true } : { target: apiBase, changeOrigin: true, ws: route.ws },
  ]));

  return {
    // root `deno task dev` runs dev:api and dev:web concurrently in one
    // terminal (`sh -c "(deno task dev:api) & (deno task dev:web) & wait"`).
    // Vite's default clearScreen wipes the whole terminal — including the
    // API server's output — on every restart/error.
    clearScreen: false,

    plugins: [
      build_stamp(),
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

    // A worker is bundled by its own Rollup pass, which gets none of the
    // plugins above: without the deno resolver there, the bake worker's first
    // `@xpute/` import failed to resolve and the build stopped. ES modules
    // because the worker is spawned with `type: "module"`.
    worker: {
      format: "es",
      plugins: () => [deno()],
    },

    // .glb isn't in Vite's default recognized-asset extension list, so
    // without this each import (companion character/pet models) has to
    // rely solely on its own `?url` query suffix to be treated as an
    // asset rather than parsed as JS — which held up fine at a handful of
    // files but became flaky once the character variant list grew to 36
    // (concurrent dev-server requests would occasionally misclassify one
    // and throw "invalid JS syntax", nondeterministically, a different
    // file each time). Declaring the extension here makes every .glb a
    // recognized asset unconditionally, in both dev and build.
    assetsInclude: ["**/*.glb"],

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
      // staying under apps/web. Explicitly allowing the repo root fixes
      // every such path at once instead of allow-listing each package.
      fs: { allow: [path.resolve(__dirname, "..", "..")] },
      // Each prefix forwarded whole and unrewritten.
      proxy,
    },

    // The production bundle served here, against the same routes: with
    // `VITE_DEV_SOURCE=remote`, the deployment's data under the built
    // client, which is what a measurement of the client alone wants —
    // the dev server's unbundled modules and hooks are not in it.
    preview: { proxy },

    define: {
      __ENV__: JSON.stringify("browser"),
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),

      __PUBLISHED_TILES__: JSON.stringify(published),

      /** A droplet fades. A pixelet remains. 🫧 */
      __APP_NAME__: JSON.stringify("pixelet"),

      __ROLE__: JSON.stringify("client"),
    },
  };
});
