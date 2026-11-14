// Runs gpu/web/host/src/host.test.ts where the page's GPU host runs: in
// Chrome, on its WebGPU. The test is bundled for the browser and served beside
// the tree's golden records, whose root the bundle's golden reader is handed.
// Chrome is started headless on the page, and the page posts its outcome back.
// Without Chrome, or without WebGPU in it, the run fails.
//
//   pnpm --dir xpute test:gpu:web          ($CHROME names the browser's binary)

import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

/** This is xpute's root, four directories up. It holds the golden records the
 * server serves and target/, which the test is bundled into. */
const ROOT = fileURLToPath(new URL("../../../../", import.meta.url));
const TEST = "gpu/web/host/src/host.test.ts";
const BUNDLE = join(ROOT, "target/browser/host.test.js");
const CHROME =
  process.env.CHROME ??
  (process.platform === "darwin"
    ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    : "google-chrome");
const SECONDS = 60;
/** Where the page serves xpute's root, which the bundle's golden reader takes
 * as its root (`GOLDEN_ROOT`): its golden records alone. */
const GOLDEN_ROOT = "/golden/";

async function main(): Promise<number> {
  // vite bundles the test and all it imports into one ES module, as a
  // library's entry.
  await build({
    configFile: false,
    root: ROOT,
    logLevel: "warn",
    define: { GOLDEN_ROOT: JSON.stringify(GOLDEN_ROOT) },
    build: {
      outDir: dirname(BUNDLE),
      emptyOutDir: false,
      copyPublicDir: false,
      minify: false,
      lib: {
        entry: join(ROOT, TEST),
        formats: ["es"],
        fileName: () => basename(BUNDLE),
      },
    },
  });
  const { promise: outcome, resolve } = Promise.withResolvers<string>();
  const page = `/${TEST.replace(/\.ts$/, ".js")}`;
  const server = createServer(async (req, res) => {
    const path = new URL(req.url ?? "/", "http://localhost").pathname;
    if (req.method === "POST" && path === "/outcome") {
      let body = "";
      for await (const chunk of req) body += chunk;
      resolve(body);
      return res.writeHead(204).end();
    }
    if (path === "/") {
      return res
        .writeHead(200, { "content-type": "text/html" })
        .end(`<!doctype html><script type="module" src="${page}"></script>`);
    }
    if (path === page) {
      return res
        .writeHead(200, { "content-type": "text/javascript" })
        .end(await readFile(BUNDLE));
    }
    if (path.startsWith(GOLDEN_ROOT) && path.includes("/spec/golden/") && !path.includes("..")) {
      return res.writeHead(200).end(await readFile(join(ROOT, path.slice(GOLDEN_ROOT.length))));
    }
    res.writeHead(404).end();
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const { port } = server.address() as AddressInfo;
  // The profile lies in the checkout's tmp/ until the run ends.
  await mkdir(join(ROOT, "tmp"), { recursive: true });
  const profile = await mkdtemp(join(ROOT, "tmp", "browser-"));
  const chrome = spawn(
    CHROME,
    [
      "--headless=new",
      "--no-first-run",
      "--no-default-browser-check",
      "--enable-unsafe-webgpu",
      `--user-data-dir=${profile}`,
      `http://127.0.0.1:${port}/`,
    ],
    { stdio: "ignore" },
  );
  const exited = new Promise((done) => chrome.once("exit", done));
  try {
    const late = new Promise<string>((_, reject) =>
      setTimeout(
        () => reject(new Error(`no outcome from Chrome in ${SECONDS} s`)),
        SECONDS * 1000,
      ),
    );
    const said = await Promise.race([outcome, late]);
    console.log(
      said === "ok"
        ? `browser: ${TEST} ok in Chrome`
        : `browser: ${TEST} failed in Chrome:\n${said}`,
    );
    return said === "ok" ? 0 : 1;
  } finally {
    chrome.kill();
    await exited;
    server.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 10 });
  }
}

process.exit(await main());
