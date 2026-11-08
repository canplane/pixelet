// node run.js <chrome|safari|s23> : the probe page loaded in one browser, its result
// written to <repo>/out/worker-probe/<browser>.json.
//
// The page is served from this directory on PORT (default 8471):
//   python3 -m http.server 8471 --bind 127.0.0.1
// chrome  the Mac's Chrome, started here with its own profile and a debugging port
// safari  the Mac's Safari through safaridriver on 4444 (`safaridriver -p 4444`)
// s23     the S23's Chrome over adb, the port `adb reverse`d

import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execSync, spawn } from "node:child_process";

const which = process.argv[2];
const PORT = Number(process.env.PORT || 8471);
const URL_ = `http://localhost:${PORT}/index.html`;
const REPO = path.resolve(new URL(".", import.meta.url).pathname, "../../../..");
const OUT = path.join(REPO, "out/worker-probe");
fs.mkdirSync(OUT, { recursive: true });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

function client(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const w = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && w.has(d.id)) { w.get(d.id)(d); w.delete(d.id); } };
  const open = new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  return { open, call: (method, params = {}) => new Promise((r) => { const i = ++id; w.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); }), close: () => ws.close() };
}

async function cdp(port) {
  let tabs;
  for (let i = 0; i < 30; i++) {
    await wait(1000);
    try { tabs = await (await fetch(`http://localhost:${port}/json`)).json(); if (tabs.some((x) => x.type === "page")) break; } catch {}
  }
  const t = (await (await fetch(`http://localhost:${port}/json/new?about:blank`, { method: "PUT" })).json());
  const c = client(t.webSocketDebuggerUrl);
  await c.open;
  await c.call("Page.enable");
  await c.call("Page.navigate", { url: URL_ });
  let r = null;
  for (let i = 0; i < 60 && !r; i++) {
    await wait(1000);
    const v = await c.call("Runtime.evaluate", { expression: "window.__probe ?? null", returnByValue: true });
    r = v.result?.result?.value ?? null;
  }
  await fetch(`http://localhost:${port}/json/close/${t.id}`);
  c.close();
  return r;
}

let result;
if (which === "chrome") {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "probe-chrome-"));
  const p = spawn("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", [`--user-data-dir=${dir}`, "--remote-debugging-port=9334", "--no-first-run", "about:blank"], { stdio: "ignore" });
  try { result = await cdp(9334); } finally { p.kill(); }
} else if (which === "s23") {
  const A = process.env.HOME + "/Library/Android/sdk/platform-tools/adb";
  execSync(`${A} shell input keyevent KEYCODE_WAKEUP`);
  execSync(`${A} reverse tcp:${PORT} tcp:${PORT}`);
  execSync(`${A} forward tcp:9333 localabstract:chrome_devtools_remote`);
  execSync(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d about:blank`);
  result = await cdp(9333);
} else if (which === "safari") {
  const WD = "http://localhost:4444";
  const wd = async (m, p, b) => (await (await fetch(WD + p, { method: m, headers: { "content-type": "application/json" }, body: b && JSON.stringify(b) })).json()).value;
  const s = await wd("POST", "/session", { capabilities: { alwaysMatch: { browserName: "safari" } } });
  const S = `/session/${s.sessionId}`;
  try {
    await wd("POST", `${S}/url`, { url: URL_ });
    for (let i = 0; i < 60 && !result; i++) { await wait(1000); result = await wd("POST", `${S}/execute/sync`, { script: "return window.__probe ?? null", args: [] }); }
  } finally { await wd("DELETE", S); }
} else throw new Error("node run.js <chrome|safari|s23>");

const file = path.join(OUT, `${which}.json`);
fs.writeFileSync(file, JSON.stringify(result, null, 1));
console.log(file);
console.log(JSON.stringify({ worker: result && { ...result.worker, text: result.worker?.text ? "…" : null }, compare: result?.compare }, null, 1));
