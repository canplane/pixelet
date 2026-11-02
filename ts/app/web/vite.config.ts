// @pxl/web/vite.config.ts

import {
  defineConfig,
  type HmrContext,
  loadEnv,
  type Plugin,
  type ProxyOptions,
  type ViteDevServer,
} from "vite";
import { execFile, execSync } from "node:child_process";
import { appendFileSync, readFileSync, watch } from "node:fs";
import { createRequire } from "node:module";
import process from "node:process";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import react from "@vitejs/plugin-react";

import {
  API_PREFIX,
  ASSET_PREFIX,
  GEO_PREFIX,
  SITE_URL,
  TILE_PREFIX,
} from "@pxl/client/driver/namespace/route.spec.ts";
import {
  DECL,
  enumAt,
  literal,
  type Members,
  readEnums,
  stale,
} from "@pxl/build/const-enum.ts";
import { REPO, WORKSPACE } from "@pxl/build/paths.ts";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** Where `name` is installed for the package at `dir`: pnpm gives each
 * package its own dependencies. */
function package_dir(name: string, dir: string): string {
  return path.dirname(
    createRequire(path.join(dir, "package.json")).resolve(
      `${name}/package.json`,
    ),
  );
}

const geoBaseDefault = "http://127.0.0.1:3000";
const tileBaseDefault = "http://127.0.0.1:3001";
const apiBaseDefault = "http://127.0.0.1:3002";

/**
 * Which source a build is: the short hash, and `-dirty` where the working
 * tree had uncommitted changes.
 */
function buildId(): string {
  try {
    const git = (args: string) =>
      execSync(`git ${args}`, {
        cwd: __dirname,
        stdio: ["ignore", "pipe", "ignore"],
      })
        .toString()
        .trim();
    const dirty = git("status --porcelain") !== "" ? "-dirty" : "";
    return `${git("rev-parse --short HEAD")}${dirty}`;
  } catch {
    return "unknown";
  }
}

/**
 * Which build this is: `#<n>` where CI handed the build its number, and
 * `#<n>-<id>` — the number the deployment serves, then what this one actually
 * is — where it did not; before any number is cached, the id alone. The count is
 * taken by CI's number job (script/build-log.py --take); the deployment
 * states its own in /build.json (`buildStamp`), and ts/client/.build_number is
 * this checkout's cache of it (script/build-number.py, run by `make dev`).
 */
function buildNumber(id: string): string {
  const official = process.env.PXL_BUILD_NUMBER;
  if (official) return `#${official}`;
  try {
    const last = readFileSync(
      path.join(WORKSPACE, "client", ".build_number"),
      "utf8",
    ).trim();
    return /^\d+$/.test(last) ? `#${last}-${id}` : id;
  } catch {
    return id;
  }
}

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
          // Also appended to a file, for a reader without this terminal.
          if (lines.length > 0) {
            appendFileSync(
              CONSOLE_LOG,
              lines.map((l) => `${new Date().toISOString()} ${l}\n`).join(""),
            );
          }
          res.statusCode = 204;
          res.end();
        });
      });
    },
  };
}

/** The page's own URL where index.html names it (`%SITE_URL%`), its links'
 * previews: the site's (spec/spec/route.json `origins`). */
function siteUrl(): Plugin {
  return {
    name: "pxl-site-url",
    transformIndexHtml: (html) => html.replaceAll("%SITE_URL%", SITE_URL),
  };
}

/** index.html's comments are for its source: a build serves it without
 * them. Vite's `minify` takes the bundle, never the HTML. */
function stripHtmlComments(): Plugin {
  return {
    name: "pxl-strip-html-comments",
    apply: "build",
    transformIndexHtml: (html) =>
      html.replace(/[ \t]*<!--[\s\S]*?-->[ \t]*\n?/g, ""),
  };
}

/**
 * `virtual:pxl-build` — what a build is: when it was built (UTC), which
 * source, which official number (deploy.ts reads it). A dev server drops the
 * module from the graph on each page load, so its values are those of the
 * source on screen.
 */
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
      return (
        `export const time = ${JSON.stringify(new Date().toISOString())};\n` +
        `export const id = ${JSON.stringify(id)};\n` +
        `export const number = ${JSON.stringify(buildNumber(id))};\n`
      );
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
    // What the build is, said by the deployment itself at /build.json, so a
    // checkout learns the number being served with no credential.
    generateBundle() {
      const official = process.env.PXL_BUILD_NUMBER;
      this.emitFile({
        type: "asset",
        fileName: "build.json",
        source:
          JSON.stringify({
            number: official ? Number(official) : null,
            id: buildId(),
            time: new Date().toISOString(),
          }) + "\n",
      });
    },
  };
}

/**
 * Inlines every `const enum`: Vite transpiles each file on its own, and oxc
 * erases the declaration, which leaves every importer bound to a missing
 * export.
 *
 * The table is ts/build/const-enum.ts's, shared with the device's Babel pass.
 * Here each use is replaced by its value — a file's own enums in that file
 * only — and the declarations struck. An access left behind after a
 * declaration went is a build error. The table is read once, as the server
 * starts, so an enum declared since, or a member added to one, is the same
 * error, saying the server has to be restarted. A file that declares enums
 * and reaches the transform without its declarations is a build error.
 */
function constEnum(roots: string[]): Plugin {
  const table = readEnums(roots);
  const restart = "restart the dev server";

  // An id can carry a query or a hash (`?url`), which hides the extension
  // from a `$`-anchored test: the file holding the declaration would lose it
  // while the importer keeps the name it can no longer bind.
  const sourceOf = (id: string) => path.resolve(id.split("?")[0].split("#")[0]);
  const declares = (file: string) =>
    table.exported.has(file) || table.local.has(file);
  const IMPORT =
    /import\s*(?:type\s+)?\{([^}]*)\}\s*from\s*(['"][^'"]+['"])\s*;?/g;
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
    transform(code, id) {
      const at = sourceOf(id);
      if (!/\.tsx?$/.test(at) || id.includes("node_modules")) return null;
      if (declares(at) && code.search(DECL) < 0) {
        throw new Error(
          `const-enum: ${at} reached the bundler with its enums already emitted (${id})`,
        );
      }
      for (const [, isExported, name] of code.matchAll(DECL)) {
        if (!(isExported ? table.exported : table.local).get(at)?.has(name)) {
          throw new Error(`${stale(`const enum ${name}`, restart)} (${id})`);
        }
      }
      // What each name in the file is: its own enums, and every enum an
      // import clause names, followed to the file that declares it — so an
      // alias is the enum it aliases, and two files' `Command` are two.
      const bound = new Map<string, Members>([
        ...(table.local.get(at) ?? []),
        ...(table.exported.get(at) ?? []),
      ]);
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
        // Whole names only: INPUT_POSE is a prefix of INPUT_POSE_END.
        out = out.replace(
          new RegExp(`\\b${name}\\.(\\w+)\\b`, "g"),
          (whole, key: string) =>
            members.has(key) ? literal(members.get(key)!) : whole,
        );
        struck.push(name);
      }
      if (!struck.length) return null;
      out = out.replace(DECL, "");
      // A struck enum leaves the clause that imported it too.
      out = out.replace(IMPORT, (whole, clause: string, from: string) => {
        const all = items(clause);
        const kept = all
          .filter((i) => !imported.has(i.local))
          .map((i) => i.item);
        if (kept.length === all.length) return whole;
        return kept.length ? `import { ${kept.join(", ")} } from ${from};` : "";
      });
      for (const name of struck) {
        const left = new RegExp(`\\b${name}\\.\\w`).exec(out);
        if (left) {
          throw new Error(
            `${stale(
              `${name}.${
                /\w+/.exec(out.slice(left.index + name.length + 1))![0]
              }`,
              restart,
            )} (${id})`,
          );
        }
      }
      return { code: out, map: null };
    },
  };
}

/**
 * Prefixes a dev server forwards, split as the Worker splits them: `tiles` to
 * the tile server, the rest to the search server. `published` ones go to the
 * published deployment under `dev`.
 */
export const DEV_PROXY_ROUTES: readonly {
  path: string;
  tiles?: boolean;
  app?: boolean;
  published?: boolean;
}[] = [
  { path: TILE_PREFIX, tiles: true, published: true },
  { path: ASSET_PREFIX, tiles: true, published: true },
  { path: GEO_PREFIX, published: true },
  { path: API_PREFIX, app: true, published: true },
];

// Where `make dev` points when nothing overrides it: the project's
// own published deployment, the site (spec/spec/route.json `origins`).
// VITE_REMOTE_ORIGIN overrides it.
const remoteOriginDefault = SITE_URL;

// anti-HMR: a change here reloads the page. These modules hold the one
// kernel's memory and device, the place's callbacks and the shell's
// subscriptions, which a hot-swapped copy would duplicate. The shell's
// components alone take Vite's own HMR.
const corePathIncludes = [
  "/xpute/ts/kit/",
  "/xpute/ts/host/",
  "/ts/client/src/host/",
  "/ts/client/src/abi/",
  "/ts/client/src/port/",
  "/ts/client/src/driver/",
  "/ts/client/src/shell/catalog/",
  "/ts/app/web/src/guest.",
];

// One reload for a change that reaches both sides: while the kernel builds, a
// core change waits for it. The wait before a reload lets a
// build the same write starts claim it: the generator writes the .ts beside
// the .rs, and the Rust watcher reports the one after Vite's reports the other.
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

function fullReloadOnCoreChange(): Plugin {
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
    enforce: "pre",

    // Vite's watcher covers `root`, ts/app/web, alone.
    configureServer(server) {
      for (const pat of corePathIncludes) {
        server.watcher.add(path.resolve(REPO, pat.replace(/^\/|\/$/g, "")));
      }
      server.watcher.add(path.join(WORKSPACE, "client", "src"));
    },

    handleHotUpdate(ctx: HmrContext) {
      const file = ctx.file;

      if (isCoreFile(file)) {
        reloadOnce(ctx.server);
        ctx.server.config.logger.info(
          `core reload ${path.relative(REPO, file)}`,
          { timestamp: true },
        );
        return [];
      }
      // Nothing returned, not `[]`: Vite takes any array as the modules to
      // update, which would disable HMR for every non-core file.
    },
  };
}

// The kernel's module, built again when what it is built from changes. It is
// built once as the server starts (package.json `dev`), and the
// page reloads when src/guest.wasm is replaced
// (fullReloadOnCoreChange). Every change under rust/ asks, outside cargo's
// own target/: build-wasm writes nothing when the module is not built from
// it. One build at a time; a change during one asks for one more after it.
function rebuildKernelOnRustChange(): Plugin {
  return {
    name: "rebuild-kernel",
    apply: "serve",
    configureServer(server) {
      const crates = path.join(REPO, "rust");
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
          const done = await promisify(execFile)("./script/build-wasm.py", {
            cwd: __dirname,
          }).then(
            ({ stdout }) => ({ success: true, said: stdout }),
            (e: { stderr?: string; message: string }) => ({
              success: false,
              said: e.stderr || e.message,
            }),
          );
          const said = done.said.trim();
          if (done.success) {
            server.config.logger.info(said.split("\n").at(-1) ?? "", {
              timestamp: true,
            });
          } else {
            server.config.logger.error(`kernel build failed\n${said}`, {
              timestamp: true,
            });
          }
        } while (again);
        kernelBuild.running = false;
        sendHeldReload(server);
      };
      const watcher = watch(crates, { recursive: true }, (_, file) => {
        if (file && !path.join(crates, file).startsWith(target)) build();
      });
      server.httpServer?.once("close", () => watcher.close());
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");

  // A production build is never a development build, whatever .env says:
  // Vite loads .env for every mode. Only the "development" value is
  // overridden, so a production build labeled `staging` still says staging.
  // isDev is derived from the name.
  const rawAppEnv = env.VITE_APP_ENV || mode;
  const appEnv =
    mode === "production" && rawAppEnv === "development"
      ? "production"
      : rawAppEnv;
  const isDev = appEnv === "development";

  // Three local servers, one per root plus the tiles a deployment reads out of
  // the bucket instead.
  const geoBase = env.VITE_GEO_BASE_URL || geoBaseDefault;
  const tileBase = env.VITE_TILE_BASE_URL || tileBaseDefault;
  const apiBase = env.VITE_API_BASE_URL || apiBaseDefault;

  //   delivery (default) — an already-published deployment, so no local
  //                          API, database or Docker. `make dev`, and
  //                          what a deployed build is.
  //   oven               — the servers above, baking on demand.
  //                          `make dev-oven`.
  //   shelf              — the same, the tiles from the last bakes under
  //                          ./out. `make dev-shelf`.
  const devSource = env.VITE_DEV_SOURCE || "delivery";
  const delivery = devSource === "delivery";

  const remoteOrigin = delivery
    ? env.VITE_REMOTE_ORIGIN || remoteOriginDefault
    : "";
  // What the proxy hands the page is never stored: the objects behind it are
  // `immutable` for a year, and a restart of this server cuts a body in
  // flight.
  const noStore: ProxyOptions["configure"] = (proxy) => {
    proxy.on("proxyRes", (res) => {
      res.headers["cache-control"] = "no-store";
    });
  };
  const proxy = Object.fromEntries(
    DEV_PROXY_ROUTES.map((route) => [
      route.path,
      {
        target:
          remoteOrigin && route.published
            ? remoteOrigin
            : route.tiles
              ? tileBase
              : route.app
                ? apiBase
                : geoBase,
        changeOrigin: true,
        configure: noStore,
      },
    ]),
  );

  return {
    // Root `make dev-oven` runs the three servers and this in one terminal.
    clearScreen: false,

    plugins: [
      siteUrl(),
      stripHtmlComments(),
      buildStamp(),
      remoteConsole(),
      constEnum([
        path.resolve(__dirname, "src"),
        path.join(WORKSPACE, "client", "src"),
        path.join(REPO, "xpute", "ts"),
      ]),
      react(),

      rebuildKernelOnRustChange(),

      // monorepo core packages change => force full reload (no HMR)
      fullReloadOnCoreChange(),
    ],

    resolve: {
      // The aliases are ../../tsconfig.json's.
      tsconfigPaths: true,
      alias: [
        // The shell imports React Native's API; a web bundle takes the
        // same names from react-native-web. Exact, so a package under
        // `react-native/…` is not rewritten.
        {
          find: /^react-native$/,
          replacement: package_dir("react-native-web", __dirname),
        },
        // react-native-svg's package entry is its CommonJS one, whose native
        // half reaches into React Native's Flow sources: the web half is
        // named.
        {
          find: /^react-native-svg$/,
          replacement: path.join(
            package_dir("react-native-svg", path.join(WORKSPACE, "client")),
            "lib/module/ReactNativeSVG.web.js",
          ),
        },
        // Where a bundled image's address is kept: React Native's own
        // registry reads it off React Native, which the web does not run, and
        // react-native-web keeps the same registry under this name.
        {
          find: /^@react-native\/assets-registry\/registry$/,
          replacement: path.join(
            package_dir("react-native-web", __dirname),
            "dist/modules/AssetRegistry/index.js",
          ),
        },
      ],
      // Inside such a library, a file per platform stands beside the shared
      // one (`elements.web.js` beside `elements.js`), and a bundler for a
      // platform is expected to prefer that platform's.
      extensions: [
        ".web.mjs",
        ".web.js",
        ".web.ts",
        ".web.tsx",
        ".mjs",
        ".js",
        ".mts",
        ".ts",
        ".jsx",
        ".tsx",
        ".json",
      ],
    },
    optimizeDeps: {
      rolldownOptions: {
        resolve: {
          extensions: [
            ".web.mjs",
            ".web.js",
            ".web.ts",
            ".web.tsx",
            ".mjs",
            ".js",
            ".mts",
            ".ts",
            ".jsx",
            ".tsx",
            ".json",
          ],
        },
      },
    },

    // .glb is not in Vite's default asset extensions: under concurrent
    // dev-server requests a `?url` import alone is now and then parsed as JS.
    assetsInclude: ["**/*.glb"],

    build: {
      target: "esnext",
      minify: "oxc",
      sourcemap: mode !== "production",
    },

    server: {
      host: true,
      // A tunnel's hostname, so a device that is not this machine can reach
      // the dev server over HTTPS: WebGPU needs a secure context. Vite checks
      // the Host header against this list, against DNS rebinding.
      allowedHosts: [".trycloudflare.com"],
      // Vite's default fs.allow is the pnpm workspace, ts/, which leaves out
      // xpute/ts/'s assets.
      fs: { allow: [REPO] },
      // Each prefix forwarded whole and unrewritten.
      proxy,
    },

    // The production bundle served here, against the same routes: with
    // `VITE_DEV_SOURCE=delivery`, the deployment's data under the built
    // client.
    preview: { proxy },

    define: {
      __APP_ENV__: JSON.stringify(appEnv),
      __IS_DEV__: JSON.stringify(isDev),
      // Vite's own import.meta.env.DEV, as a global a bundle Vite did not
      // make can set too (src/bundle.ts).
      __DEV_BUNDLE__: JSON.stringify(mode !== "production"),

      __DEV_SOURCE__: JSON.stringify(devSource),

      // react-native-web's Animated reads React Native's `global`, which a
      // browser calls `globalThis`.
      global: "globalThis",
    },
  };
});
