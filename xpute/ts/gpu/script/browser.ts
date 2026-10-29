// Runs ts/gpu/src/host.test.ts where the web host runs: in Chrome, on its
// WebGPU. The test is bundled for the browser and served at its own path
// beside the tree's spec/, Chrome is started headless on the page, and the
// page posts its outcome back. No Chrome, or no WebGPU in it, fails: a
// device without WebGPU is not one xpute runs on.
//
//   pnpm --dir ts test:gpu:web          ($CHROME names the browser's binary)

import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

/** xpute's root, three up: the spec it serves, and ts/target/, which it bundles into. */
const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const TEST = "ts/gpu/src/host.test.ts";
const BUNDLE = join(ROOT, "ts/target/browser/host.test.js");
const CHROME =
  process.env.CHROME ??
  (process.platform === "darwin"
    ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    : "google-chrome");
const SECONDS = 60;

async function main(): Promise<number> {
  await build({
    entryPoints: [join(ROOT, TEST)],
    bundle: true,
    platform: "browser",
    format: "esm",
    outfile: BUNDLE,
    absWorkingDir: join(ROOT, "ts"),
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
    if (path.startsWith("/spec/") && !path.includes("..")) {
      return res.writeHead(200).end(await readFile(join(ROOT, path)));
    }
    res.writeHead(404).end();
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const { port } = server.address() as AddressInfo;
  const profile = await mkdtemp(join(tmpdir(), "xpute-browser-"));
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
      ).unref(),
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
    await rm(profile, { recursive: true });
  }
}

process.exit(await main());
