#!/usr/bin/env node
// @pxl/web/script/gate.mjs
//
// The gate a change to the renderer or the kernel passes before it lands:
// the change's page against its base's, each served on its own port
// (.agents/skills/performance/SKILL.md, "Measuring in the app"), in a fresh
// Chrome a run, base and change by turns. Read off the app's own profiler
// (`pxl.perf.read`, `pxl.perf.drive`): the first load's longest frame, which
// its long-frame observer replays from load (perf/long-frame.ts), and the
// GPU's busy time over an orbit at noon and at 07:00. Fails where the
// change's median runs past the base's: its load's longest frame by more than
// `LOAD_SLACK_MS`, its GPU's p50 or p95 by more than `P50_SHARE` or
// `P95_SHARE`.
//
//   node ts/app/web/script/gate.mjs <base url> <change url> [rounds]

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const CHROME =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
/** Each run's own debugging port, so no run reaches a Chrome still closing. */
let port = 9335;
const SIZE = { width: 2056, height: 1113 };
/** Where the device stands: S1, the route the measurements here are taken on. */
const PLACE = { latitude: 37.2741, longitude: 127.0349 };
/** How much longer the change's first load may block the page than the base's. */
const LOAD_SLACK_MS = 3000;
/** How much more the change's GPU time may take than the base's. */
const P50_SHARE = 1.15;
const P95_SHARE = 1.3;
/** The page settled: no resource ends for this long, within `LOAD_CAP_MS`. */
const QUIET_MS = 8000;
const LOAD_CAP_MS = 90000;
const HOURS = ["noon", "dawn"];
/** The longest the page may take to answer one ask, a drive included. */
const EVAL_CAP_MS = 120000;
const ORBIT_S = 10;
/** A run that drew no frame is run again, up to this many times in all. */
const TRIES = 3;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function open(url) {
  const at = ++port;
  const dir = mkdtempSync(join(tmpdir(), "pxl-gate-"));
  const chrome = spawn(
    CHROME,
    [
      `--user-data-dir=${dir}`,
      `--remote-debugging-port=${at}`,
      "--no-first-run",
      "--no-default-browser-check",
      // A window behind another draws as one in front: no frame is throttled.
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--disable-background-timer-throttling",
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  let pages = [];
  for (let i = 0; i < 80 && !pages.some((p) => p.type === "page"); i++) {
    await sleep(250);
    pages = await fetch(`http://127.0.0.1:${at}/json`)
      .then((r) => r.json())
      .catch(() => []);
  }
  const ws = new WebSocket(pages.find((p) => p.type === "page").webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    waiting.get(d.id)?.(d);
    waiting.delete(d.id);
  };
  const send = (method, params = {}) =>
    new Promise((r) => {
      waiting.set(++id, r);
      ws.send(JSON.stringify({ id, method, params }));
    });
  // A page that stops answering fails its run rather than holding the gate.
  const ev = async (expression) => {
    const r = await Promise.race([
      send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }),
      sleep(EVAL_CAP_MS).then(() => {
        throw new Error(`no answer in ${EVAL_CAP_MS} ms: ${expression.slice(0, 40)}`);
      }),
    ]);
    if (r.result.exceptionDetails) throw new Error(r.result.exceptionDetails.text);
    return r.result.result.value;
  };
  await send("Emulation.setDeviceMetricsOverride", { ...SIZE, deviceScaleFactor: 0, mobile: false });
  // The device stands at `PLACE`, so the view opens there as a user's does.
  await send("Browser.grantPermissions", { origin: new URL(url).origin, permissions: ["geolocation"] });
  await send("Emulation.setGeolocationOverride", { ...PLACE, accuracy: 10 });
  // Every resource kept, past the browser's 250, so the load is seen to settle.
  await send("Page.addScriptToEvaluateOnNewDocument", { source: "performance.setResourceTimingBufferSize(1e6)" });
  await send("Page.navigate", { url });
  const close = async () => {
    ws.close();
    if (chrome.exitCode === null && chrome.signalCode === null) {
      const gone = new Promise((r) => chrome.once("exit", r));
      chrome.kill();
      await gone;
    }
    rmSync(dir, { recursive: true, force: true });
  };
  return { ev, close };
}

/** One run of a page at `hour`: its load's longest frame and its GPU busy time.
 * One drive a page: a window started again on a page that recorded one
 * already timed no GPU frame in these runs. */
async function run(url, hour) {
  for (let i = 1; ; i++) {
    const r = await once(url, hour);
    if (r.gpu[2] > 0 || i === TRIES) return r;
  }
}

async function once(url, hour) {
  const page = await open(url);
  try {
    let last = 0;
    for (let quiet = 0, t = 0; quiet < QUIET_MS && t < LOAD_CAP_MS; t += 1000) {
      await sleep(1000);
      const end = await page
        .ev("Math.max(0, ...performance.getEntriesByType('resource').map((e) => e.responseEnd))")
        .catch(() => 0);
      quiet = end > last ? 0 : quiet + 1000;
      last = Math.max(last, end);
    }
    // The drive turns the profiler on, which installs the long-frame
    // observers, and they replay the load.
    await page.ev(`pxl.perf.drive("orbit", ${ORBIT_S}, ${JSON.stringify(hour)})`);
    return await page.ev(`(() => {
      const r = pxl.perf.read();
      const f = r.frames;
      return {
        load: Math.max(0, ...r.longs.map((l) => l.durationMs)),
        gpu: f ? [f.gpu_busy.p50, f.gpu_busy.p95, f.n, f.gpu_read] : [NaN, NaN, 0, 0],
      };
    })()`);
  } finally {
    await page.close();
  }
}

// A figure the window could not give (null) counts as none: NaN fails every test.
const median = (v) => {
  const s = v.map((x) => x ?? NaN).sort((a, b) => a - b);
  return s[Math.floor((s.length - 1) / 2)];
};

async function main() {
  const [base, change, rounds = "2"] = process.argv.slice(2);
  if (!base || !change) {
    console.error("gate.mjs <base url> <change url> [rounds]");
    return 2;
  }
  // Each server's first page builds what it serves; no run counts it.
  for (const url of [base, change]) await once(url, HOURS[0]);
  const runs = { base: {}, change: {} };
  for (let i = 0; i < Number(rounds); i++) {
    for (const hour of HOURS) {
      for (const [who, url] of [["base", base], ["change", change]]) {
        (runs[who][hour] ??= []).push(await run(url, hour));
      }
    }
  }
  if (process.env.GATE_RUNS) console.log(JSON.stringify(runs));
  const of = (who, f) => median(Object.values(runs[who]).flat().map(f));
  const fails = [];
  const load = [of("base", (r) => r.load), of("change", (r) => r.load)];
  console.log(`first load, longest frame ms: ${load[0].toFixed(0)} -> ${load[1].toFixed(0)}`);
  if (!(load[1] <= load[0] + LOAD_SLACK_MS)) fails.push(`the first load blocks ${load[1].toFixed(0)} ms`);
  for (const hour of HOURS) {
    const [b, c] = ["base", "change"].map((w) => [0, 1, 2, 3].map((k) => median(runs[w][hour].map((r) => r.gpu[k]))));
    if (!(b[3] > 0 && c[3] > 0)) fails.push(`${hour}: no GPU frame timed`);
    console.log(
      `${hour}: GPU p50 ${b[0].toFixed(2)} -> ${c[0].toFixed(2)}, p95 ${b[1].toFixed(2)} -> ${c[1].toFixed(2)}, frames ${b[2]} -> ${c[2]}, timed ${b[3]} -> ${c[3]}`,
    );
    if (!(c[0] <= b[0] * P50_SHARE)) fails.push(`${hour}: GPU p50 past ${P50_SHARE}× the base's`);
    if (!(c[1] <= b[1] * P95_SHARE)) fails.push(`${hour}: GPU p95 past ${P95_SHARE}× the base's`);
  }
  for (const f of fails) console.log(`FAIL ${f}`);
  console.log(fails.length ? "gate: fail" : "gate: pass");
  return fails.length ? 1 : 0;
}

process.exit(await main());
