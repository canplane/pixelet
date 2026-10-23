// @pxl/web/vite.config.ts

import { defineConfig, type HmrContext, loadEnv, type Plugin, type ProxyOptions, type ViteDevServer } from "vite";
import deno from "@deno/vite-plugin";
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import process from "node:process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import react from "@vitejs/plugin-react";

import { DEV_PROXY_ROUTES } from "../client/src/config/route.ts";
import { DECL, enumAt, literal, type Members, readEnums, stale } from "../client/script/const-enum.ts";
import { mapped } from "../client/script/import-map.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const geoBaseDefault = "http://127.0.0.1:3000";
const tileBaseDefault = "http://127.0.0.1:3001";
const apiBaseDefault = "http://127.0.0.1:3002";

/**
 * Which source a build is: the short hash, and `-dirty` where the working
 * tree had uncommitted changes. Derived from git rather than a counter kept
 * in a file, which would have to be bumped by a commit of its own and would
 * count builds rather than source. The commit count that used to lead it
 * (`r<commits>`) is gone: it renumbers whenever the history is rewritten,
 * which main's is here as a matter of course, and the hash was the identity
 * either way.
 */
function buildId(): string {
  try {
    const git = (args: string) => execSync(`git ${args}`, { cwd: __dirname, stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
    const dirty = git("status --porcelain") !== "" ? "-dirty" : "";
    return `${git("rev-parse --short HEAD")}${dirty}`;
  } catch {
    return "unknown";
  }
}

/**
 * Which build this is: `#<n>` where CI handed the build its number, and
 * `#<n>-<id>` — the last number this checkout saw published, then what this
 * one actually is — where it did not. A build with no number from CI is not
 * that build whatever it was built from, so the source says so itself rather
 * than a sign meaning "after"; before any number is cached, the id alone.
 * The count lives in the bucket and is taken by the gate job
 * (script/build-counter.ts); apps/client/.build_number is only this checkout's cache of
 * it, written by any read of `./script/build-log.ts`.
 */
function buildNumber(id: string): string {
  const official = process.env.PXL_BUILD_NUMBER;
  if (official) return `#${official}`;
  try {
    const last = readFileSync(path.join(__dirname, "..", "client", ".build_number"), "utf8").trim();
    return /^\d+$/.test(last) ? `#${last}-${id}` : id;
  } catch {
    return id;
  }
}

/**
 * `virtual:pxl-build` — what a build is: when it was built, which source,
 * which official number (config/deploy.ts reads it). A module rather than
 * `define` constants so that a dev server gives every page load the values
 * as they are then, not as they were when the server started: the module is
 * dropped from the graph on each navigation and computed again on import,
 * so a profiler report taken from a long-running dev server names the source
 * actually on screen.
 *
 * The time is evaluated at build, so it updates itself with nothing to
 * remember — a date someone has to bump by hand goes stale silently. It
 * does mean the bundle hash changes on every build even when no source
 * did; that is the cost of it being accurate, chosen over the commit date.
 * UTC: it is formatted for the reader's own timezone where it is shown
 * (shell/setting/section/about.tsx).
 */
/** The other half of `src/remote-console.ts`: what a device posts to
 * `/__console` is printed where this server's own output goes, so a phone or a
 * tablet on a tunnel can be read without attaching a debugger to it. Dev only
 * — a plugin's `configureServer` never runs for a build. */
const CONSOLE_LOG = "./.dev-console.log";

function remoteConsole(): Plugin {
  return {
    name: "pxl-remote-console",
    configureServer(server) {
      server.middlewares.use("/__console", (req, res) => {
        if (req.method !== "POST") {
          res.statusCode = 405;
          return res.end();
        }
        let body = "";
        req.on("data", (c: Buffer) => {
          if (body.length < 1 << 16) body += c.toString();
        });
        req.on("end", () => {
          const lines = body.split("\n").filter(Boolean);
          for (const line of lines) console.log(line);
          // Also to a file, because the terminal this prints in belongs to
          // whoever started the server and an agent reading the same session
          // cannot see it. Appended rather than held, so `tail -f` works and
          // nothing here has to be flushed or closed.
          if (lines.length > 0) {
            Deno.writeTextFileSync(CONSOLE_LOG, lines.map((l) => `${new Date().toISOString()} ${l}\n`).join(""), { append: true, create: true });
          }
          res.statusCode = 204;
          res.end();
        });
      });
    },
  };
}

function buildStamp(): Plugin {
  const id = "virtual:pxl-build";
  const resolved = "\0" + id;
  return {
    name: "pxl-build-stamp",
    resolveId(source) {
      return source === id ? resolved : undefined;
    },
    load(source) {
      if (source !== resolved) return undefined;
      const id = buildId();
      return `export const time = ${JSON.stringify(new Date().toISOString())};\n` +
        `export const id = ${JSON.stringify(id)};\n` +
        `export const number = ${JSON.stringify(buildNumber(id))};\n`;
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
 * should hold the number alone. It does not, because Vite transpiles each
 * file on its own and nothing ever sees the declaration and its use at once:
 * esbuild emitted the runtime object, reverse mapping and all, so a build
 * carried every command's and every errno's name as a string
 * (`e[e.WORLD_SNAPSHOT=6933]="WORLD_SNAPSHOT"`); oxc erases the declaration
 * as the type syntax it is, which leaves every importer bound to an export
 * that no longer exists.
 *
 * The table is apps/client/script/const-enum.ts's, shared with the device's Babel pass,
 * and it refuses an enum it cannot inline rather than leave one. Here each
 * use is replaced by its value — a file's own enums in that file only — and
 * the declarations struck. An access left behind after a declaration went
 * is a build error, never a bundle that reaches a browser and throws. The
 * table is read once, as the server starts, so an enum declared since, or a
 * member added to one, is the same error, saying the server has to be
 * restarted.
 *
 * A file of the tree outside this root (xpute/ts) is one the Deno resolver
 * hands back as a virtual module its loader has already emitted, every enum
 * an object by then; such a file is read from its own path instead, as one
 * inside the root is. A file that declares enums and reaches the transform
 * without its declarations is a build error.
 */
function constEnum(roots: string[]): Plugin {
  const table = readEnums(roots);
  const restart = "restart the dev server";

  // An id can carry a query or a hash — `?worker`, and the `#deno` the Deno
  // resolver appends to the virtual id it hands back — and either one hides
  // the extension from a `$`-anchored test. Missing a module that way is
  // silent and asymmetric: the file holding the declaration is reached under
  // its plain path and loses it, while the importer keeps the name it can no
  // longer bind.
  //
  // The Deno resolver's virtual id names the file last, after `::`
  // (`\0deno::TypeScript::file:///…::/…/x.ts`), and an import is resolved
  // against the file itself, so that is what is kept.
  const sourceOf = (id: string) => {
    const plain = id.split("?")[0].split("#")[0];
    const at = plain.lastIndexOf("::");
    return path.resolve(at < 0 ? plain : plain.slice(at + 2).replace(/^file:\/\//, ""));
  };
  const declares = (file: string) => table.exported.has(file) || table.local.has(file);
  const IMPORT = /import\s*(?:type\s+)?\{([^}]*)\}\s*from\s*(['"][^'"]+['"])\s*;?/g;
  const items = (clause: string) =>
    clause
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => {
        const [orig, as] = s.replace(/^type\s+/, "").split(/\s+as\s+/);
        return { item: s, orig, local: as ?? orig };
      });

  return {
    name: "const-enum",
    // Before the transpiler reaches the types: the declarations are still here.
    enforce: "pre",
    async resolveId(id, importer, options) {
      const resolved = await this.resolve(id, importer, { ...options, skipSelf: true });
      if (resolved === null || !resolved.id.startsWith("\0deno::")) return null;
      const file = sourceOf(resolved.id);
      return /\.tsx?$/.test(file) && roots.some((r) => file.startsWith(r + path.sep)) ? file : null;
    },
    transform(code, id) {
      const at = sourceOf(id);
      if (!/\.tsx?$/.test(at) || id.includes("node_modules")) return null;
      if (declares(at) && code.search(DECL) < 0) throw new Error(`const-enum: ${at} reached the bundler with its enums already emitted (${id})`);
      for (const [, isExported, name] of code.matchAll(DECL)) {
        if (!(isExported ? table.exported : table.local).get(at)?.has(name)) throw new Error(`${stale(`const enum ${name}`, restart)} (${id})`);
      }
      // What each name in the file is: its own enums, and every enum an
      // import clause names, followed to the file that declares it — so an
      // alias is the enum it aliases, and two files' `Command` are two.
      const bound = new Map<string, Members>([...(table.local.get(at) ?? []), ...(table.exported.get(at) ?? [])]);
      const imported = new Set<string>();
      for (const [, clause, from] of code.matchAll(IMPORT)) {
        for (const { orig, local } of items(clause)) {
          const members = enumAt(table, at, from.slice(1, -1), orig);
          if (members) {
            bound.set(local, members);
            imported.add(local);
          }
        }
      }
      let out = code;
      const struck: string[] = [];
      for (const [name, members] of bound) {
        if (!out.includes(name)) continue;
        // Whole names only: a member that is a prefix of another — a table
        // read before SENSOR_ENABLE became SENSOR_ENABLE_LOCATION — would
        // otherwise leave `7444_LOCATION` for the parser instead of a name
        // the stale check below can say.
        out = out.replace(new RegExp(`\\b${name}\\.(\\w+)\\b`, "g"), (whole, key: string) => (members.has(key) ? literal(members.get(key)!) : whole));
        struck.push(name);
      }
      if (!struck.length) return null;
      out = out.replace(DECL, "");
      // An enum's name is gone from what it was bound to, so it goes from
      // the clause that imported it too, or the bundler is left binding an
      // export nobody declares.
      out = out.replace(IMPORT, (whole, clause: string, from: string) => {
        const all = items(clause);
        const kept = all.filter((i) => !imported.has(i.local)).map((i) => i.item);
        if (kept.length === all.length) return whole;
        return kept.length ? `import { ${kept.join(", ")} } from ${from};` : "";
      });
      for (const name of struck) {
        const left = new RegExp(`\\b${name}\\.\\w`).exec(out);
        if (left) throw new Error(`${stale(`${name}.${/\w+/.exec(out.slice(left.index + name.length + 1))![0]}`, restart)} (${id})`);
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

// anti-HMR. The shell's kernel glue is here too: its modules hold the one
// kernel's memory and device, and a hot-swapped copy ran beside them — a
// second device beside the renderer's objects on the first. The built module
// is apps/web's own, under src/kernel/.
const corePathIncludes = ["/xpute/ts/kit/", "/xpute/ts/host/", "/apps/client/src/kernel/", "/apps/client/src/catalog/", "/apps/web/src/kernel/"];

// One reload for a change that reaches both sides: while the kernel builds, a
// core change waits for it, so the page never runs the shell generated from a
// new spec beside a module built from the old. The wait before a reload lets a
// build the same write starts claim it: the generator writes the .ts beside
// the .rs, and Deno's watcher reports the one after Vite's reports the other.
const RELOAD_SETTLE_MS = 200;
const kernelBuild = { running: false, held: false };
let reloadTimer: ReturnType<typeof setTimeout> | undefined;

function reloadOnce(server: ViteDevServer) {
  kernelBuild.held = true;
  clearTimeout(reloadTimer);
  reloadTimer = setTimeout(() => {
    if (!kernelBuild.running) sendHeldReload(server);
  }, RELOAD_SETTLE_MS);
}

function sendHeldReload(server: ViteDevServer) {
  if (!kernelBuild.held) return;
  kernelBuild.held = false;
  server.ws.send({ type: "full-reload" });
}

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
    // nothing under xpute/ts/ or apps/client/ is watched, handleHotUpdate
    // below never runs for a core file, and the server keeps serving the
    // transform it cached at first import — a core edit does nothing, and a
    // browser reload does not help because the staleness is on the server.
    configureServer(server) {
      for (const pat of corePathIncludes) {
        server.watcher.add(path.resolve(__dirname, "..", "..", pat.replace(/^\/|\/$/g, "")));
      }
      server.watcher.add(path.resolve(__dirname, "..", "client", "src"));
    },

    handleHotUpdate(ctx: HmrContext) {
      const file = ctx.file;

      if (isCoreFile(file)) {
        // Invalidate by hand, because Vite cannot make the connection
        // itself. The deno plugin resolves a core import to a virtual id —
        // `\0deno::TypeScript::@xpute/kit/...::/abs/path.ts` — and the
        // module graph is keyed by resolved id while onFileChange looks up
        // by file path, so a changed core file would leave its transform
        // cached under a name nothing matches, and the full-reload below
        // would fetch the same stale module again. The absolute path is
        // inside the virtual id, so matching on it is enough to find them.
        for (const mod of ctx.server.moduleGraph.idToModuleMap.values()) {
          if (mod.id?.includes(file)) ctx.server.moduleGraph.invalidateModule(mod);
        }
        reloadOnce(ctx.server);
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

// The kernel's module, built again when what it is built from changes. It is
// built once as the server starts (deno.jsonc `dev`'s dependency), and the
// page reloads when the module under src/kernel/ is replaced
// (fullReloadOnCoreChange), so a Rust edit otherwise waited for a hand-run
// build or a restart of everything. Every change under crates/ asks, outside
// cargo's own target/: cargo knows which of them the module is built from, and
// build-wasm writes nothing when it is not one of them. One build at a time; a
// change during one asks for one more after it, which covers every change it
// missed.
/**
 * The tree's own prefixes, as the importing file's import map gives them
 * (apps/client/script/import-map.ts), the reader Metro resolves through too.
 * A file's platform variants (pill.ios.tsx beside pill.tsx) are a device's;
 * the page takes the default every import names.
 */
function importMaps(): Plugin {
  return {
    name: "pxl-import-maps",
    enforce: "pre",
    resolveId(id, importer) {
      if (!importer || importer.startsWith("\0")) return null;
      const hit = mapped(importer, id);
      return hit ? this.resolve(hit.path, importer, { skipSelf: true }) : null;
    },
  };
}

function rebuildKernelOnRustChange(): Plugin {
  return {
    name: "rebuild-kernel",
    apply: "serve",
    configureServer(server) {
      const crates = path.resolve(__dirname, "..", "..", "crates");
      const target = path.join(crates, "target") + path.sep;
      let again = false;
      const build = async () => {
        if (kernelBuild.running) {
          again = true;
          return;
        }
        kernelBuild.running = true;
        do {
          again = false;
          const done = await new Deno.Command("deno", { args: ["run", "-A", "script/build-wasm.ts"], cwd: __dirname }).output();
          const said = new TextDecoder().decode(done.success ? done.stdout : done.stderr).trim();
          if (done.success) server.config.logger.info(said.split("\n").at(-1) ?? "", { timestamp: true });
          else server.config.logger.error(`kernel build failed\n${said}`, { timestamp: true });
        } while (again);
        kernelBuild.running = false;
        sendHeldReload(server);
      };
      const watcher = Deno.watchFs(crates);
      server.httpServer?.once("close", () => watcher.close());
      (async () => {
        for await (const event of watcher) {
          if (event.paths.some((p) => !p.startsWith(target))) build();
        }
      })();
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
  // The other direction is honored: running the dev server with
  // VITE_APP_ENV=production turns dev-only surfaces off locally, which is
  // the point of the variable. Only the "development" value is overridden,
  // so a production build labeled `staging` still says staging.
  //
  // The override lands on the NAME, and isDev is derived from it: a guard
  // on isDev alone would leave __APP_ENV__ reading "development" while the
  // debug section it implies is absent. One value, one rule.
  const rawAppEnv = env.VITE_APP_ENV || mode;
  const appEnv = mode === "production" && rawAppEnv === "development" ? "production" : rawAppEnv;
  const isDev = appEnv === "development";

  // Three local servers, one per root plus the tiles a deployment reads out of
  // the bucket instead. See
  // doc/minutes/20260918-the-server-in-rust-and-the-line-already-drawn.md.
  const geoBase = env.VITE_GEO_BASE_URL || geoBaseDefault;
  const tileBase = env.VITE_TILE_BASE_URL || tileBaseDefault;
  const apiBase = env.VITE_API_BASE_URL || apiBaseDefault;

  //   delivery (default) — an already-published deployment, so no local
  //                          API, database or Docker. `deno task dev`, and
  //                          what a deployed build is.
  //   oven               — the servers above, baking on demand.
  //                          `deno task dev:oven`.
  //   shelf              — the same, the tiles from the last bakes under
  //                          ./out. `deno task dev:shelf`.
  const devSource = env.VITE_DEV_SOURCE || "delivery";
  const delivery = devSource === "delivery";

  const remoteOrigin = delivery ? (env.VITE_REMOTE_ORIGIN || remoteOriginDefault) : "";
  // What the proxy hands the page is never stored. The objects behind it are
  // `immutable` for a year, which a deployment means; here a restart of this
  // server cuts a body in flight, and a browser kept the empty one as the
  // asset under that header through every reload (2026-09-28).
  const noStore: ProxyOptions["configure"] = (proxy) => {
    proxy.on("proxyRes", (res) => {
      res.headers["cache-control"] = "no-store";
    });
  };
  const proxy = Object.fromEntries(DEV_PROXY_ROUTES.map((route) => [
    route.path,
    {
      target: remoteOrigin && route.published ? remoteOrigin : route.tiles ? tileBase : route.app ? apiBase : geoBase,
      changeOrigin: true,
      configure: noStore,
    },
  ]));

  return {
    // Root `deno task dev:oven` runs the three servers and this in one
    // terminal, and Vite's default clearScreen would wipe the whole of it —
    // the servers' output too — on every restart or error.
    clearScreen: false,

    plugins: [
      buildStamp(),
      remoteConsole(),
      constEnum([path.resolve(__dirname, "src"), path.resolve(__dirname, "..", "client", "src"), path.resolve(__dirname, "..", "..", "xpute", "ts")]),
      importMaps(),
      deno(),
      react(),

      rebuildKernelOnRustChange(),

      // monorepo core packages change => force full reload (no HMR)
      fullReloadOnCoreChange(),
    ],

    resolve: {
      alias: [
        // The shell imports React Native's API, which deno.jsonc resolves to
        // React Native itself for the type-check; a web bundle takes the
        // same names from react-native-web. Exact, so a package under
        // `react-native/…` is not rewritten into one that does not exist.
        { find: /^react-native$/, replacement: "react-native-web" },
        // react-native-svg picks its platform by file suffix, which a web
        // bundle reached through Deno's resolver never sees: the package
        // resolves to its CommonJS entry, whose native half reaches into
        // React Native's Flow sources. The web half is named instead.
        { find: /^react-native-svg$/, replacement: path.resolve(__dirname, "../../node_modules/react-native-svg/lib/module/ReactNativeSVG.web.js") },
        // Where a bundled image's address is kept: React Native's own
        // registry reads it off React Native, which the web does not run, and
        // react-native-web keeps the same registry under this name.
        { find: /^@react-native\/assets-registry\/registry$/, replacement: path.resolve(__dirname, "../../node_modules/react-native-web/dist/modules/AssetRegistry/index.js") },
      ],
      // Inside such a library, a file per platform stands beside the shared
      // one (`elements.web.js` beside `elements.js`), and a bundler for a
      // platform is expected to prefer that platform's.
      extensions: [".web.mjs", ".web.js", ".web.ts", ".web.tsx", ".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"],
    },
    optimizeDeps: {
      rolldownOptions: { resolve: { extensions: [".web.mjs", ".web.js", ".web.ts", ".web.tsx", ".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"] } },
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
      // Named rather than left to `true`, because the two Vite ships are not
      // interchangeable and which one is the default has already changed
      // once. Measured on this bundle: oxc 762.86 kB / 248.49 kB gzipped,
      // esbuild 785.03 / 261.15.
      minify: "oxc",
      sourcemap: mode !== "production",
    },

    server: {
      host: true,
      // A tunnel's hostname, so a device that is not this machine can reach
      // the dev server over HTTPS: WebGPU is a secure context away, and a
      // LAN address over plain http is not one, so an iPad on `http://<ip>`
      // has no `navigator.gpu` at all. Named rather than opened to
      // everything — Vite checks the Host header against this list, and the
      // check is what stands between a dev server and DNS rebinding.
      allowedHosts: [".trycloudflare.com"],
      // Vite's default fs.allow walks up from this config's own directory
      // looking for an npm/pnpm/yarn workspace root; this is a Deno
      // workspace, which that heuristic does not recognize, so it falls
      // back to apps/web itself and 403s any `/@fs/` request for a file
      // outside it — invisible for plain .ts imports, which the deno plugin
      // resolves, but fatal for an asset import (`import x from "*.png"`)
      // that points into xpute/ts/. The repo root allowed once covers every
      // such path.
      fs: { allow: [path.resolve(__dirname, "..", "..")] },
      // Each prefix forwarded whole and unrewritten.
      proxy,
    },

    // The production bundle served here, against the same routes: with
    // `VITE_DEV_SOURCE=delivery`, the deployment's data under the built
    // client, which is what a measurement of the client alone wants —
    // the dev server's unbundled modules and hooks are not in it.
    preview: { proxy },

    define: {
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),
      // Vite's own import.meta.env.DEV, as a global a bundle Vite did not
      // make can set too (config/bundle.ts).
      __DEV_BUNDLE__: JSON.stringify(mode !== "production"),

      __DEV_SOURCE__: JSON.stringify(devSource),

      // react-native-web's Animated reads React Native's `global`, which a
      // browser calls `globalThis`.
      global: "globalThis",
    },
  };
});
