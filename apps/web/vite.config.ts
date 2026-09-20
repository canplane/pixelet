// @pxl/web/vite.config.ts

import { defineConfig, type HmrContext, loadEnv, type Plugin } from "vite";
import deno from "@deno/vite-plugin";
import { execSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import process from "node:process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import svgr from "vite-plugin-svgr";

import { DEV_PROXY_ROUTES } from "./src/config/route.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const geoBaseDefault = "http://127.0.0.1:3000";
const tileBaseDefault = "http://127.0.0.1:3001";
const apiBaseDefault = "http://127.0.0.1:3002";

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
 * Which official build this is: `b<n>` when CI handed the build its number,
 * `b<n>+` for any other build after the last one this checkout saw published,
 * `b?` before one. The count lives in the bucket and is taken by the gate job
 * (script/build-counter.ts); .build_number is only this checkout's cache of it,
 * written by `deno task deploy` and by any read of `./script/build-log.ts`.
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
 * which official number (config/deploy.ts reads it). A module rather than
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
 * (feature/setting/section/about.tsx).
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

/**
 * A `const enum` is a compile-time name for a number, and a published bundle
 * should hold the number alone. It does not: Vite transpiles each file on its
 * own with esbuild and bundles the results with Rollup, so nothing ever sees
 * the declaration and its use at once, and esbuild — which inlines a const
 * enum perfectly well when it bundles — emits the runtime object instead,
 * reverse mapping and all. A build carried every command's and every errno's
 * name as a string that way (`e[e.WORLD_SNAPSHOT=6933]="WORLD_SNAPSHOT"`).
 *
 * What a per-file pass cannot do is done by reading the declarations up front:
 * every member of every `export const enum` under `roots`, then each member
 * access replaced by its value and the declarations struck. The sources keep
 * their enums; only the bundle loses them.
 *
 * It refuses rather than guesses. A member whose value is not a plain literal
 * or arithmetic over literals leaves its whole enum alone, and an access left
 * behind after the declaration went is a build error, never a bundle that
 * reaches a browser and throws.
 */
function const_enum(roots: string[]): Plugin {
  const DECL = /export\s+const\s+enum\s+(\w+)\s*\{([^}]*)\}/g;
  /** Numbers, strings and arithmetic over them, and nothing that runs. */
  const VALUE = /^(?:0x[0-9a-fA-F]+|\d+|"[^"\\]*"|'[^'\\]*'|[-+*/|&^~()<>\s])+$/;
  const enums = new Map<string, Map<string, string>>();

  for (const root of roots) {
    const stack = [root];
    while (stack.length) {
      const dir = stack.pop()!;
      for (const e of readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, e.name);
        if (e.isDirectory()) {
          if (e.name !== "node_modules") stack.push(full);
          continue;
        }
        if (!/\.tsx?$/.test(e.name) || e.name.includes(".test.")) continue;
        for (const [, name, body] of readFileSync(full, "utf8").matchAll(DECL)) {
          const members = new Map<string, string>();
          let whole = true;
          for (let line of body.replace(/\/\*[\s\S]*?\*\//g, "").split("\n")) {
            line = line.replace(/\/\/.*/, "").trim().replace(/,$/, "");
            if (!line || line.startsWith("*")) continue;
            const at = line.indexOf("=");
            const key = at < 0 ? "" : line.slice(0, at).trim();
            const value = at < 0 ? "" : line.slice(at + 1).trim();
            if (!/^\w+$/.test(key) || !VALUE.test(value)) {
              whole = false;
              break;
            }
            members.set(key, /^["']/.test(value) ? value : String(Function(`return (${value})`)()));
          }
          if (whole) enums.set(name, members);
        }
      }
    }
  }

  return {
    name: "const-enum",
    // Before esbuild erases the types: the declarations are still here.
    enforce: "pre",
    transform(code, id) {
      if (!/\.tsx?$/.test(id) || id.includes("node_modules")) return null;
      let out = code;
      const struck: string[] = [];
      for (const [name, members] of enums) {
        if (!out.includes(name)) continue;
        for (const [key, value] of members) out = out.replaceAll(`${name}.${key}`, value);
        struck.push(name);
      }
      if (!struck.length) return null;
      out = out.replace(DECL, "");
      // The name is gone from the module that had it, so it has to go from
      // every clause that imported it too, or the bundler is left binding an
      // export nobody declares.
      out = out.replace(/import\s*\{([^}]*)\}\s*from\s*(['"][^'"]+['"])\s*;?/g, (whole, clause: string, from: string) => {
        const kept = clause
          .split(",")
          .map((s) => s.trim())
          .filter((s) => s && !struck.includes(s.replace(/^type\s+/, "")));
        if (kept.length === clause.split(",").map((s) => s.trim()).filter(Boolean).length) return whole;
        return kept.length ? `import { ${kept.join(", ")} } from ${from};` : "";
      });
      for (const name of struck) {
        const left = new RegExp(`\\b${name}\\.\\w`).exec(out);
        if (left) throw new Error(`const-enum: ${id} still reads ${left[0]} after ${name} was inlined`);
      }
      return { code: out, map: null };
    },
  };
}

// Where `deno task dev` points when nothing overrides it: the project's
// own published deployment. VITE_REMOTE_ORIGIN overrides it, and
// .env.example carries this same value, so the origin is changed there;
// this constant is only for a checkout that has not copied .env.example to
// .env yet. A domain that is retired at the Cloudflare edge answers 404 to
// everything, and nothing here would say why.
const remoteOriginDefault = "https://plei.me";

// anti-HMR. The shell's kernel glue is here too: kernel.ts boots the device
// and the world at module load, and its importers are component modules Vite
// hot-swaps, so a hot update ran that boot again against the one kernel the
// memory holds — a second device beside the renderer's objects on the first.
const corePathIncludes = ["/packages/xpute/core/", "/apps/web/src/kernel/", "/apps/web/src/catalog/"];
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

    // Vite's watcher covers `root`, which here is apps/web: without this,
    // nothing under packages/ is watched, handleHotUpdate below never runs
    // for a core file, and the server keeps serving the transform it cached
    // at first import — a core edit does nothing, and a browser reload does
    // not help because the staleness is on the server.
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
        // `\0deno::TypeScript::@xpute/core/...::/abs/path.ts` — and the
        // module graph is keyed by resolved id while onFileChange looks up
        // by file path, so a changed core file would leave its transform
        // cached under a name nothing matches, and the full-reload below
        // would fetch the same stale module again. The absolute path is
        // inside the virtual id, so matching on it is enough to find them.
        for (const mod of ctx.server.moduleGraph.idToModuleMap.values()) {
          if (mod.id?.includes(file)) ctx.server.moduleGraph.invalidateModule(mod);
        }
        ctx.server.ws.send({ type: "full-reload" });
        // Vite prints "page reload" for the ones it decides itself and says
        // nothing about this one, so without a line here a core edit looks
        // exactly like a core edit that was never noticed.
        ctx.server.config.logger.info(`core reload ${path.relative(path.resolve(__dirname, "..", ".."), file)}`, { timestamp: true });
        return [];
      }
      // Nothing returned, not `[]`: Vite takes any array — an empty one
      // too — as the modules to update and overwrites ctx.modules with it,
      // which would disable HMR for every non-core file. Returning nothing
      // lets Vite fall through to its default HMR for those.
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, Deno.cwd(), "VITE_");

  // A production build is never a development build, whatever .env says.
  // VITE_APP_ENV lives in .env, which Vite loads for *every* mode, so a
  // developer's local `VITE_APP_ENV=development` would ride into the
  // deployed bundle and ship the debug settings section and the dev routes
  // with it.
  //
  // The other direction is honoured: running the dev server with
  // VITE_APP_ENV=production turns dev-only surfaces off locally, which is
  // the point of the variable. Only the "development" value is overridden,
  // so a production build labelled `staging` still says staging.
  //
  // The override lands on the NAME, and isDev is derived from it: a guard
  // on isDev alone would leave __APP_ENV__ reading "development" while the
  // debug section it implies is absent. One value, one rule.
  const rawAppEnv = env.VITE_APP_ENV || mode;
  const appEnv = mode === "production" && rawAppEnv === "development" ? "production" : rawAppEnv;
  const isDev = appEnv === "development";

  // Three local servers, one per root plus the tiles a deployment reads out of
  // the bucket instead. See
  // issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
  const geoBase = env.VITE_GEO_BASE_URL || geoBaseDefault;
  const tileBase = env.VITE_TILE_BASE_URL || tileBaseDefault;
  const apiBase = env.VITE_API_BASE_URL || apiBaseDefault;

  //   remote (default) — an already-published deployment, so no local API,
  //                        database or Docker. `deno task dev`.
  //   local             — the servers above, baking on demand.
  //                        `deno task dev:local`.
  const remote = env.VITE_DEV_SOURCE !== "local";

  // Injected as __PUBLISHED_TILES__: route.ts builds every tile path from
  // it, config.ts reads it for what a 404 means.
  const published = mode === "production" || remote;

  const remoteOrigin = remote ? (env.VITE_REMOTE_ORIGIN || remoteOriginDefault) : "";
  const proxy = Object.fromEntries(DEV_PROXY_ROUTES.map((route) => [
    route.path,
    remoteOrigin && route.published ? { target: remoteOrigin, changeOrigin: true } : { target: route.tiles ? tileBase : route.app ? apiBase : geoBase, changeOrigin: true },
  ]));

  return {
    // Root `deno task dev:local` runs the three servers and this in one
    // terminal, and Vite's default clearScreen would wipe the whole of it —
    // the servers' output too — on every restart or error.
    clearScreen: false,

    plugins: [
      build_stamp(),
      const_enum([path.resolve(__dirname, "src"), path.resolve(__dirname, "..", "..", "packages")]),
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
    // plugins above: without the deno resolver there, a worker's first
    // `@xpute/` import does not resolve and the build stops. ES modules,
    // for a worker spawned with `type: "module"`.
    worker: {
      format: "es",
      plugins: () => [deno()],
    },

    // .glb is not in Vite's default asset extensions: without this each
    // import relies on its own `?url` suffix to be treated as an asset
    // rather than parsed as JS, and under concurrent dev-server requests
    // one of 36 character variants is misclassified now and then and throws
    // "invalid JS syntax", a different file each time. Declared here, every
    // .glb is an asset in both dev and build.
    assetsInclude: ["**/*.glb"],

    build: {
      target: "esnext",
      minify: "esbuild",
      sourcemap: mode !== "production",
    },

    server: {
      host: true,
      // Vite's default fs.allow walks up from this config's own directory
      // looking for an npm/pnpm/yarn workspace root; this is a Deno
      // workspace, which that heuristic does not recognize, so it falls
      // back to apps/web itself and 403s any `/@fs/` request for a file
      // outside it — invisible for plain .ts imports, which the deno plugin
      // resolves, but fatal for an asset import (`import x from "*.png"`)
      // that points into packages/. The repo root allowed once covers every
      // such path.
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
