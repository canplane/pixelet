// node hot.mjs <port> <label> <drive seconds> : a fresh load on the S23's Chrome, pxl.perf.drive, its report,
// the GPU's clock cap and temperatures every 5 s.
import fs from "node:fs";
import { execSync } from "node:child_process";
const [VPORT, L, SECS] = process.argv.slice(2);
const OUT = process.env.OUT; fs.mkdirSync(OUT, { recursive: true });
const A = process.env.HOME + "/Library/Android/sdk/platform-tools/adb", PORT = 9333, O = `http://localhost:${VPORT}`;
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
function client(url) {
  const ws = new WebSocket(url); let id = 0; const w = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && w.has(d.id)) { w.get(d.id)(d); w.delete(d.id); } };
  const open = new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  return { open, call: (method, params = {}) => new Promise((r) => { const i = ++id; w.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); }), close: () => ws.close() };
}
async function ev(c, expression) {
  const r = await c.call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, timeout: 900000 });
  if (r.error) throw new Error(r.error.message);
  if (r.result.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 400));
  return r.result.result.value;
}
const sh = (s) => execSync(s, { stdio: ["ignore", "pipe", "ignore"] }).toString();
const therm = () => {
  const t = sh(`${A} shell dumpsys thermalservice`).split("\n").filter((l) => /Thermal Status|mName=(AP|SKIN)\b/.test(l)).slice(0, 3).map((s) => s.trim().replace(/Temperature\{mValue=|, mType=\d+|mName=|\}|, mStatus=\d/g, "")).join(" | ");
  const k = sh(`${A} shell cat /sys/class/kgsl/kgsl-3d0/max_gpuclk /sys/class/kgsl/kgsl-3d0/thermal_pwrlevel`).trim().split("\n").join(" ");
  return `${k} ${t}`;
};
sh(`${A} shell input keyevent KEYCODE_WAKEUP`);
sh(`${A} reverse tcp:${VPORT} tcp:${VPORT}`);
sh(`${A} reverse tcp:3001 tcp:3001`);
sh(`${A} forward tcp:${PORT} localabstract:chrome_devtools_remote`);
sh(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d about:blank`);
let tabs;
for (let i = 0; i < 30; i++) { await wait(1000); try { tabs = await (await fetch(`http://localhost:${PORT}/json`)).json(); if (tabs.some((x) => x.type === "page" && x.url === "about:blank")) break; } catch {} }
const t = tabs.find((x) => x.type === "page" && x.url === "about:blank");
if (!t) throw new Error("no blank tab");
for (const x of tabs.filter((x) => x.type === "page" && x.id !== t.id && (x.url.startsWith("http://localhost") || x.url === "about:blank"))) await fetch(`http://localhost:${PORT}/json/close/${x.id}`);
await fetch(`http://localhost:${PORT}/json/activate/${t.id}`);
const v = await (await fetch(`http://localhost:${PORT}/json/version`)).json();
const b = client(v.webSocketDebuggerUrl); await b.open;
await b.call("Browser.grantPermissions", { origin: O, permissions: ["geolocation"] });
const c = client(t.webSocketDebuggerUrl); await c.open;
const GEO = `(()=>{const pos=()=>({coords:{latitude:38.1193,longitude:128.4656,accuracy:10,altitude:null,altitudeAccuracy:null,heading:null,speed:null},timestamp:Date.now()});navigator.geolocation.getCurrentPosition=(s)=>setTimeout(()=>s(pos()),0);navigator.geolocation.watchPosition=(s)=>{setTimeout(()=>s(pos()),0);return 1};})()`;
await c.call("Page.enable"); await c.call("Page.addScriptToEvaluateOnNewDocument", { source: GEO });
await c.call("Input.setIgnoreInputEvents", { ignore: true });
await c.call("Emulation.setGeolocationOverride", { latitude: 38.1193, longitude: 128.4656, accuracy: 10 });
await c.call("Storage.clearDataForOrigin", { origin: O, storageTypes: "local_storage,indexeddb" });
await c.call("Page.bringToFront"); await c.call("Page.navigate", { url: O + "/" }); await wait(1500); await c.call("Page.bringToFront");
const log = []; const say = (s) => { log.push(s); fs.writeFileSync(`${OUT}/${L}.log`, log.join("\n") + "\n"); };
say("therm0 " + therm());
let ok = false;
for (let i = 0; i < 400; i++) { await wait(500); try { const s = await ev(c, `({p:typeof pxl,c:!!document.querySelector('canvas'),d:!!(window.pxl&&pxl.perf&&pxl.perf.drive)})`); if (s.p === "object" && s.c && s.d) { ok = true; break; } } catch {} }
say("page ready " + ok); if (!ok) process.exit(1);
await wait(35000);
const t0 = Date.now();
const clock = setInterval(() => { try { say(`t ${Math.round((Date.now() - t0) / 1000)} ${therm()}`); } catch {} }, 5000);
const text = await ev(c, `pxl.perf.drive(${Number(SECS)})`);
clearInterval(clock);
fs.writeFileSync(`${OUT}/${L}-report.txt`, String(text));
if (process.env.REST) { await wait(Number(process.env.REST) * 1000); fs.writeFileSync(`${OUT}/${L}-rest-report.txt`, String(await ev(c, `pxl.perf.report()`))); }
say("end " + therm());
await c.call("Page.navigate", { url: "about:blank" }).catch(() => 0);
c.close(); b.close();
sh(`${A} reverse --remove tcp:${VPORT}`); sh(`${A} forward --remove tcp:${PORT}`);
console.log(L, "done");
process.exit(0);
