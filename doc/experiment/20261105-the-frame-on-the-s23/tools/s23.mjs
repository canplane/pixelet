// node s23.mjs : one fresh load of a page on the S23's Chrome over adb, then the scenes asked.
//
// env
//   VPORT     the page's port on the Mac, a Metro dev bundle (default 8081); `adb reverse`d.
//   LABEL     the run's name: files are OUT/LABEL*.
//   OUT       where data and shots go (default <repo>/out/s23).
//   SCENES    in order, comma-separated (default off,on,drive):
//               off, on   a 25 s circle with the shadows off / on, its frames' rows, then an 8 s
//                         window with ray_counts on (`counts off|on` in the log)
//               walk      the actor walked north 12 s, the camera still (shadows as last set)
//               trace     the profiler's window as a Perfetto trace, after the scene before it
//               drive     pxl.perf.drive(DRIVE_S)
//               hold-off, hold-on  a circle of HOLD_S seconds (default 60) with the shadows off / on,
//                         every frame's columns dumped (`-<scene>-frames.json`: the scale, the
//                         lattice, the kind, the GPU's), a shot at its end
//               rest      15 s with nothing moved, every frame's columns dumped (`-rest<n>-frames.json`)
//               set:K=V   a setting set to its value, 3 s to land
//               at:N      the wheel N steps out (in where negative), as the drive's, 3 s to land, then a
//                         10 s circle with ray_counts on, every frame's columns dumped
//                         (`-at<n>-frames.json`, n the window's number; the report's distance and
//                         polar in the log)
//               wheel:N   the wheel N steps out (in where negative), 10 s to land, a shot `-wheel<n>.png`
//               tilt      the view tilted up as the drive tilts it
//               cold      the drive with ray_counts on and every frame's columns dumped; give it
//                         first, on a fresh load, to see the cache's refill after the zoom-out
//   HOUR      the sky's hour for the scenes (noon by default; morning for low shadows)
//   RES       fixed (default), auto (the user's default) or low
//   OBSTACLE  orbit_obstacle: through, stop or climb (default the page's own)
//   SETTLE_MS wait after the renderer is live, for the strata's textures (default 25000;
//             shorter from the shelf), then a shot `-settled.png`
//   DRIVE_S   the drive's seconds (default 20); a sustained run is DRIVE_S=300 with
//             FRAMES=1, its last 120 s read off the frames, the phone warm
//   COLD=1    this tab's HTTP cache off: a first load
//   PROBE=1   rAF, submit, onSubmittedWorkDone, mapAsync and getCurrentTexture timed in the page,
//             kept a scene each in `-<scene>-probe.json`
//
// Only localhost and about:blank tabs are the run's; the phone's other tabs are never touched.
// The page's settings and profiler are reached through Metro's module table (`__r`), so the
// page must be a dev bundle.

import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const env = process.env;
const VPORT = Number(env.VPORT || 8081), PORT = 9333, O = `http://localhost:${VPORT}`;
const REPO = path.resolve(new URL(".", import.meta.url).pathname, "../../../..");
const OUT = env.OUT || path.join(REPO, "out/s23");
fs.mkdirSync(OUT, { recursive: true });
const L = env.LABEL || `run-${VPORT}`;
const SCENES = (env.SCENES || "off,on,drive").split(",");
const A = env.HOME + "/Library/Android/sdk/platform-tools/adb";
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

function client(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const w = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && w.has(d.id)) { w.get(d.id)(d); w.delete(d.id); }
  };
  const open = new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
  return {
    open,
    call: (method, params = {}) => new Promise((r) => { const i = ++id; w.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); }),
    close: () => ws.close(),
  };
}
async function ev(c, expression, timeout = 300000) {
  const r = await c.call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, timeout });
  if (r.error) throw new Error(r.error.message);
  if (r.result.exceptionDetails) throw new Error(JSON.stringify(r.result.exceptionDetails).slice(0, 400));
  return r.result.result.value;
}

const out = { port: VPORT, label: L, scenes: SCENES, log: [] };
const file = path.join(OUT, `${L}.json`);
const log = (k, v) => {
  out.log.push([k, v]);
  console.log(k, typeof v === "string" ? v.slice(0, 160) : JSON.stringify(v)?.slice(0, 160));
  fs.writeFileSync(file, JSON.stringify(out, null, 1));
};
const therm = () => execSync(`${A} shell dumpsys thermalservice`).toString().split("\n")
  .filter((l) => /Thermal Status|mName=(AP|SKIN)\b/.test(l)).slice(0, 3)
  .map((s) => s.trim().replace(/Temperature\{mValue=|, mType=\d+|mName=|\}|, mStatus=\d/g, "")).join(" | ");

execSync(`${A} shell input keyevent KEYCODE_WAKEUP`);
execSync(`${A} reverse tcp:${VPORT} tcp:${VPORT}`);
execSync(`${A} forward tcp:${PORT} localabstract:chrome_devtools_remote`);
execSync(`${A} shell am start -a android.intent.action.VIEW -n com.android.chrome/com.google.android.apps.chrome.Main -d about:blank`);
let tabs;
for (let i = 0; i < 30; i++) {
  await wait(1000);
  try { tabs = await (await fetch(`http://localhost:${PORT}/json`)).json(); if (tabs.some((x) => x.type === "page" && x.url === "about:blank")) break; } catch {}
}
const t = tabs.find((x) => x.type === "page" && x.url === "about:blank");
if (!t) throw new Error("no blank tab");
for (const x of tabs.filter((x) => x.type === "page" && x.id !== t.id && (x.url.startsWith("http://localhost") || x.url === "about:blank")))
  await fetch(`http://localhost:${PORT}/json/close/${x.id}`);
await fetch(`http://localhost:${PORT}/json/activate/${t.id}`);
const v = await (await fetch(`http://localhost:${PORT}/json/version`)).json();
const b = client(v.webSocketDebuggerUrl);
await b.open;
log("grant", (await b.call("Browser.grantPermissions", { origin: O, permissions: ["geolocation"] })).error || "ok");
const c = client(t.webSocketDebuggerUrl);
await c.open;

// Before the page's own code: a fixed position, every error kept in window.__E, and the probe.
const GEO = `(()=>{const pos=()=>({coords:{latitude:38.1193,longitude:128.4656,accuracy:10,altitude:null,altitudeAccuracy:null,heading:null,speed:null},timestamp:Date.now()});const g=navigator.geolocation;let id=1;g.getCurrentPosition=(ok)=>{setTimeout(()=>ok(pos()),0)};g.watchPosition=(ok)=>{setTimeout(()=>ok(pos()),0);return id++};g.clearWatch=()=>{}})();`;
const ERR = `(()=>{window.__E=[];const push=(k,a)=>{if(window.__E.length<60)window.__E.push(k+': '+a.map(x=>String(x&&x.message||x)).join(' ').slice(0,400))};for(const k of ['error','warn']){const f=console[k];console[k]=(...a)=>{push(k,a);f.apply(console,a)}};addEventListener('error',e=>push('page',[e.message]));addEventListener('unhandledrejection',e=>push('rejection',[e.reason]));const rd=GPUAdapter.prototype.requestDevice;GPUAdapter.prototype.requestDevice=function(...a){return rd.apply(this,a).then(d=>{d.addEventListener('uncapturederror',e=>push('gpu',[e.error]));d.lost.then(i=>push('lost',[i.reason,i.message]));return d})}})();`;
const PROBE = `(()=>{const L=window.__G={raf:[],sub:[],done:[],map:[],tex:[]};const now=()=>performance.now();const cap=a=>a.length<60000;const raf=window.requestAnimationFrame.bind(window);window.requestAnimationFrame=cb=>raf(ts=>{const a=now();try{cb(ts)}finally{if(cap(L.raf))L.raf.push([ts,a,now()])}});const s=GPUQueue.prototype.submit;GPUQueue.prototype.submit=function(b){const a=now();const r=s.call(this,b);if(cap(L.sub))L.sub.push([a,now(),b.length]);return r};const w=GPUQueue.prototype.onSubmittedWorkDone;GPUQueue.prototype.onSubmittedWorkDone=function(){const a=now();return w.call(this).then(v=>{if(cap(L.done))L.done.push([a,now()]);return v})};const m=GPUBuffer.prototype.mapAsync;GPUBuffer.prototype.mapAsync=function(mode,o,sz){const a=now();const all=this.size;return m.apply(this,arguments).then(v=>{if(cap(L.map))L.map.push([a,now(),sz??all,1]);return v},e=>{if(cap(L.map))L.map.push([a,now(),sz??all,0]);throw e})};const g=GPUCanvasContext.prototype.getCurrentTexture;GPUCanvasContext.prototype.getCurrentTexture=function(){const a=now();const r=g.call(this);if(cap(L.tex))L.tex.push([a,now()]);return r}})();`;
await c.call("Page.enable");
await c.call("Network.enable");
await c.call("Network.setCacheDisabled", { cacheDisabled: env.COLD === "1" });
await c.call("Page.addScriptToEvaluateOnNewDocument", { source: ERR + GEO + (env.PROBE === "1" ? PROBE : "") });
await c.call("Input.setIgnoreInputEvents", { ignore: true });
await c.call("Emulation.setGeolocationOverride", { latitude: 38.1193, longitude: 128.4656, accuracy: 10 });
log("clear", (await c.call("Storage.clearDataForOrigin", { origin: O, storageTypes: "local_storage,indexeddb" })).error || "ok");
log("cold", env.COLD === "1");

const bye = async (n) => {
  log("errs", await ev(c, `JSON.stringify(window.__E)`, 5000).catch((e) => String(e)));
  try { await c.call("Network.setCacheDisabled", { cacheDisabled: false }); await c.call("Page.navigate", { url: "about:blank" }); } catch {}
  c.close(); b.close();
  execSync(`${A} reverse --remove tcp:${VPORT}`);
  execSync(`${A} forward --remove tcp:${PORT}`);
  process.exit(n);
};

await c.call("Page.bringToFront");
const t0 = Date.now();
await c.call("Page.navigate", { url: O + "/" });
await wait(1500);
await c.call("Page.bringToFront");
await ev(c, `navigator.wakeLock.request('screen').then(l=>{window.__wl0=l;return 1}).catch(e=>String(e))`, 5000).then((v) => log("wake lock", v));
const T = () => Math.round((Date.now() - t0) / 100) / 10;
log("therm0", therm());
let first = null;
for (let i = 0; i < 600; i++) {
  await wait(500);
  let s;
  try { s = await ev(c, `({p:typeof pxl,c:!!document.querySelector('canvas'),l:document.body.innerText.includes('공간을 불러오고')})`, 5000); } catch { continue; }
  if (s.p === "object" && s.c && !s.l) { first = T(); break; }
}
log("world shown s", first);
if (first === null) { log("state", await ev(c, `document.body.innerText.slice(0,200)`, 5000).catch((e) => String(e))); await bye(1); }
await wait(15000);

// The page's own modules, by the path Metro names them.
log("page", await ev(c, `(async()=>{window.__wl=await navigator.wakeLock.request('screen').catch(()=>0);const ms=__r.getModules();const ent=ms instanceof Map?[...ms]:Object.entries(ms);const get=p=>{const e=ent.find(([,m])=>m&&m.verboseName===p);if(!e)throw new Error('no module '+p);return __r(Number(e[0]))};const S=get('src/port/setting.ts'),Q=get('src/abi/schema.spec.ts'),F=get('src/port/perf.ts'),I=get('src/port/input.ts'),H=get('src/services/share.ts');window.__m={set:S.set,SET:Q.SETTING,start:F.start,frames:F.perf_frames,keys:I.keys};H.save=async(name,json)=>{window.__trace=json};const cv=document.querySelector('canvas');return {vw:innerWidth,vh:innerHeight,dpr:devicePixelRatio,cv:[cv.width,cv.height]}})()`));
const set = (k, val) => ev(c, `__m.set(__m.SET.${k}.key,${val})`);
await set("perf", 1);
await wait(1000);
await ev(c, `__m.start()`);
await wait(5000);
let live;
for (let i = 0; i < 180; i++) {
  live = await ev(c, `(()=>{const r=pxl.perf.report();const v=(r.match(/view +pivot ([^ ]+)/)||[])[1];const f=pxl.perf.read().frames||{};return {read:f.gpu_read,n:f.n,pivot:v||'-'}})()`);
  if (live.read > 0 && live.pivot !== "-") break;
  if (i % 30 === 0) await ev(c, `__m.start()`);
  await wait(1000);
}
log("renderer live", live);
if (!(live.read > 0 && live.pivot !== "-")) await bye(2);

const shot = async (n) => {
  const r = await c.call("Page.captureScreenshot", { format: "png" });
  if (!r.error) fs.writeFileSync(path.join(OUT, `${L}-${n}.png`), Buffer.from(r.result.data, "base64"));
};
// The strata's textures land by load at their own time, and paint's taps cost real time.
await wait(Number(env.SETTLE_MS || 25000));
await shot("settled");

const motion = `(async(secs,turn)=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();const [x,y]=[r.left+r.width/2,r.top+r.height/2];const p=(t,a,b)=>cv.dispatchEvent(new PointerEvent(t,{pointerId:1,pointerType:'mouse',isPrimary:true,clientX:a[0],clientY:a[1],buttons:b,button:2,bubbles:true,cancelable:true}));const on=a=>[x+100*Math.cos(a),y+100*Math.sin(a)];p('pointerdown',on(-Math.PI/2),2);const t0=performance.now();const s=setInterval(()=>p('pointermove',on(-Math.PI/2+(performance.now()-t0)/1000/turn*2*Math.PI),2),16);await new Promise(r=>setTimeout(r,secs*1000));clearInterval(s);p('pointerup',on(-Math.PI/2),0);return 1})`;
const rows = () => ev(c, `(()=>{const names=(pxl.perf.read().frames.gpu_spans||[]).map(x=>x.name);return JSON.stringify({names,rows:__m.frames().filter(f=>f.gpu_read).map(f=>[f.gpu_busy_ms,...f.gpu_spans])})})()`, 60000);
const keep = async (scene) => {
  fs.writeFileSync(path.join(OUT, `${L}-${scene}-rows.json`), await rows());
  if (env.PROBE === "1")
    fs.writeFileSync(path.join(OUT, `${L}-${scene}-probe.json`), JSON.stringify(await ev(c, `(()=>{const t0=window.__T0||0;const G=window.__G;const f=a=>a.filter(x=>x[0]>=t0);return {t0,raf:f(G.raf),sub:f(G.sub),done:f(G.done),map:f(G.map),tex:f(G.tex)}})()`, 60000)));
};
const begin = () => ev(c, `(__m.start(),window.__T0=performance.now(),1)`);
const columns = ["t", "interval_ms", "kind_name", "gpu_read", "gpu_frame", "gpu_busy_ms", "eye_cast", "eye_rays", "eye_beam", "eye_sample", "eye_miss", "eye_asked", "eye_reused", "eye_walked", "eye_sky", "terrain_landed", "pack_bytes", "scale", "lattice", "light_cast", "light_rays", "light_beam", "light_sample", "light_miss", "light_asked", "light_moved"];
const dump = async (scene) => fs.writeFileSync(path.join(OUT, `${L}-${scene}-frames.json`), await ev(c, `(()=>{const k=${JSON.stringify(columns)};const names=(pxl.perf.read().frames.gpu_spans||[]).map(x=>x.name);return JSON.stringify({keys:k,names,rows:__m.frames().map(f=>[...k.map(x=>f[x]),...(f.gpu_spans||[])])})})()`, 60000));

await set("ray_counts", 0);
await set("res", `__m.SET.res.options.${env.RES || "fixed"}`);
await set("sky_time", `__m.SET.sky_time.options.${env.HOUR || "noon"}`);
if (env.OBSTACLE) await set("orbit_obstacle", `__m.SET.orbit_obstacle.options.${env.OBSTACLE}`);
let last = "settled";
let rests = 0;
let ats = 0;
let wheels = 0;
for (const scene of SCENES) {
  if (scene === "off" || scene === "on") {
    await set("shadows", scene === "on" ? 1 : 0);
    await wait(10000);
    await begin();
    await ev(c, motion + `(25,6)`, 240000);
    log(`circle ${scene}`, await ev(c, `pxl.perf.report()`));
    await keep(scene);
    if (env.FRAMES === "1") await dump(scene);
    await shot(scene);
    await set("ray_counts", 1);
    await wait(1000);
    await ev(c, `__m.start()`);
    await ev(c, motion + `(8,6)`, 240000);
    log(`counts ${scene}`, await ev(c, `pxl.perf.report()`));
    await set("ray_counts", 0);
  } else if (scene === "hold-off" || scene === "hold-on") {
    await set("shadows", scene === "hold-on" ? 1 : 0);
    await wait(10000);
    await begin();
    await ev(c, motion + `(${Number(env.HOLD_S || 60)},6)`, 400000);
    log(scene, await ev(c, `pxl.perf.report()`));
    await dump(scene);
    await shot(scene);
  } else if (scene.startsWith("set:")) {
    const [k, val] = scene.slice(4).split("=");
    await set(k, val);
    await wait(3000);
  } else if (scene === "rest") {
    rests += 1;
    await wait(5000);
    await begin();
    await wait(15000);
    log(`rest${rests}`, await ev(c, `pxl.perf.report()`));
    await dump(`rest${rests}`);
  } else if (scene === "walk") {
    await wait(5000);
    await shot("before-walk");
    await begin();
    await ev(c, `__m.keys(0,-1)`);
    await wait(12000);
    await ev(c, `__m.keys(0,0)`);
    await wait(1000);
    log("walk", await ev(c, `pxl.perf.report()`));
    await keep("walk");
    if (env.FRAMES === "1") await dump("walk");
    await wait(4000);
    await shot("walked");
  } else if (scene === "drive") {
    await begin();
    if (env.VIEW === "1") await ev(c, `(()=>{window.__V=[];window.__Vt=setInterval(()=>{const m=pxl.perf.report().match(/distance ([0-9.]+) u +polar ([-0-9.]+). +azimuth ([-0-9.]+)/);window.__V.push(m?[Math.round(performance.now()),+m[1],+m[2],+m[3]]:null)},200);return 1})()`);
    log("drive", await ev(c, `pxl.perf.drive(${Number(env.DRIVE_S || 20)})`, 1000 * Number(env.DRIVE_S || 20) + 120000));
    if (env.VIEW === "1") log("drive view", await ev(c, `(clearInterval(window.__Vt),JSON.stringify(window.__V))`));
    await keep("drive");
    if (env.FRAMES === "1") await dump("drive");
  } else if (scene.startsWith("at:")) {
    ats += 1;
    const n = Number(scene.slice(3));
    await ev(c, `(async(n)=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();for(let i=0;i<Math.abs(n);i++){cv.dispatchEvent(new WheelEvent('wheel',{deltaY:Math.sign(n)*100,clientX:r.left+r.width/2,clientY:r.top+r.height/2,bubbles:true,cancelable:true}));await new Promise(d=>setTimeout(d,16))}})(${n})`, 60000);
    await set("ray_counts", 1);
    await wait(3000);
    await begin();
    await ev(c, motion + `(10,6)`, 240000);
    const report = await ev(c, `pxl.perf.report()`);
    log(`at${ats} wheel ${n}`, (report.match(/distance [0-9.]+ u +polar [-0-9.]+./) || ["?"])[0]);
    await dump(`at${ats}`);
    await shot(`at${ats}`);
    await set("ray_counts", 0);
  } else if (scene.startsWith("wheel:")) {
    wheels += 1;
    const n = Number(scene.slice(6));
    await ev(c, `(async(n)=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();for(let i=0;i<Math.abs(n);i++){cv.dispatchEvent(new WheelEvent('wheel',{deltaY:Math.sign(n)*100,clientX:r.left+r.width/2,clientY:r.top+r.height/2,bubbles:true,cancelable:true}));await new Promise(d=>setTimeout(d,16))}})(${n})`, 60000);
    await wait(10000);
    await shot(`wheel${wheels}`);
  } else if (scene === "tilt") {
    await ev(c, `(async()=>{const cv=document.querySelector('canvas');const r=cv.getBoundingClientRect();const [x,y]=[r.left+r.width/2,r.top+r.height/2];const p=(t,yy,b)=>cv.dispatchEvent(new PointerEvent(t,{pointerId:1,pointerType:'mouse',isPrimary:true,clientX:x,clientY:yy,buttons:b,button:2,bubbles:true,cancelable:true}));p('pointerdown',y,2);for(let n=1;n<=60;n++){p('pointermove',y-0.06*r.height*n/60,2);await new Promise(d=>setTimeout(d,16))}p('pointerup',y-0.06*r.height,0)})()`, 60000);
    await wait(1000);
  } else if (scene === "cold") {
    await set("ray_counts", 1);
    await wait(2000);
    log("cold drive", await ev(c, `pxl.perf.drive(${Number(env.DRIVE_S || 20)})`, 300000));
    await dump("cold");
    await set("ray_counts", 0);
  } else if (scene === "trace") {
    log("trace saved", await ev(c, `pxl.perf.trace()`, 120000));
    const n = await ev(c, `(window.__trace||'').length`);
    let s = "";
    for (let k = 0; k < n; k += 4e6) s += await ev(c, `window.__trace.slice(${k},${k + 4e6})`, 120000);
    fs.writeFileSync(path.join(OUT, `${L}-${last}.trace.json`), s);
    await ev(c, `(window.__trace=null,1)`);
  } else throw new Error(`no scene ${scene}`);
  log(`therm ${scene}`, therm());
  last = scene;
}
log("done s", T());
await bye(0);
