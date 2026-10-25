#!/usr/bin/env -S deno run -A
// Runs ts/gpu/src/host.test.ts where the web host runs: in Chrome, on its
// WebGPU. The test is bundled for the browser and served at its own path
// beside the tree's spec/, Chrome is started headless on the page, and the
// page posts its outcome back. No Chrome, or no WebGPU in it, fails: a
// device without WebGPU is not one xpute runs on.
//
//   deno task --cwd ts test:gpu:web          ($CHROME names the browser's binary)

import { dirname, fromFileUrl, join } from "@std/path";

/** xpute's root, three up: the spec it serves and the target it bundles into. */
const ROOT = join(dirname(fromFileUrl(import.meta.url)), "../../..");
const TEST = "ts/gpu/src/host.test.ts";
const BUNDLE = join(ROOT, "target/web-gpu/host.test.js");
const CHROME = Deno.env.get("CHROME") ?? (Deno.build.os === "darwin" ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" : "google-chrome");
const SECONDS = 60;

async function bundle(): Promise<void> {
  const out = await new Deno.Command(Deno.execPath(), { args: ["bundle", "--platform", "browser", "-o", BUNDLE, join(ROOT, TEST)], cwd: join(ROOT, "ts"), stdout: "inherit", stderr: "inherit" })
    .output();
  if (!out.success) throw new Error(`bundling ${TEST} failed`);
}

async function main(): Promise<number> {
  await bundle();
  const { promise: outcome, resolve } = Promise.withResolvers<string>();
  const page = `/${TEST.replace(/\.ts$/, ".js")}`;
  const server = Deno.serve({ hostname: "127.0.0.1", port: 0, onListen: () => {} }, async (req) => {
    const path = new URL(req.url).pathname;
    if (req.method === "POST" && path === "/outcome") {
      resolve(await req.text());
      return new Response(null, { status: 204 });
    }
    if (path === "/") return new Response(`<!doctype html><script type="module" src="${page}"></script>`, { headers: { "content-type": "text/html" } });
    if (path === page) return new Response(await Deno.readFile(BUNDLE), { headers: { "content-type": "text/javascript" } });
    if (path.startsWith("/spec/") && !path.includes("..")) return new Response(await Deno.readFile(join(ROOT, path)));
    return new Response(null, { status: 404 });
  });
  const profile = await Deno.makeTempDir({ prefix: "xpute-web-gpu-" });
  const chrome = new Deno.Command(CHROME, {
    args: ["--headless=new", "--no-first-run", "--no-default-browser-check", "--enable-unsafe-webgpu", `--user-data-dir=${profile}`, `http://127.0.0.1:${server.addr.port}/`],
    stdout: "null",
    stderr: "null",
  }).spawn();
  try {
    const late = new Promise<string>((_, reject) => setTimeout(() => reject(new Error(`no outcome from Chrome in ${SECONDS} s`)), SECONDS * 1000));
    const said = await Promise.race([outcome, late]);
    console.log(said === "ok" ? `web-gpu: ${TEST} ok in Chrome` : `web-gpu: ${TEST} failed in Chrome:\n${said}`);
    return said === "ok" ? 0 : 1;
  } finally {
    chrome.kill();
    await chrome.status;
    await server.shutdown();
    await Deno.remove(profile, { recursive: true });
  }
}

Deno.exit(await main());
