// Shrinks a WGSL the phone refuses while it still refuses it: node reduce.js in.wgsl out.wgsl
import fs from "node:fs";
const PORT = 9333;
const tabs = await (await fetch(`http://localhost:${PORT}/json`)).json();
const t = tabs.find((x) => x.type === "page" && x.url.startsWith("http://localhost:5173/"));
const ws = new WebSocket(t.webSocketDebuggerUrl);
let id = 0; const waits = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); } };
await new Promise((r) => (ws.onopen = r));
const call = (method, params) => new Promise((r) => { const i = ++id; waits.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const COMPILE = `async (code) => {
  if (!window.__dev) { const a = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" }); window.__dev = await a.requestDevice(); }
  const d = window.__dev;
  const cs = code.match(/@compute[^]*?\\bfn\\s+(\\w+)\\s*\\(/)?.[1];
  d.pushErrorScope("validation");
  const m = d.createShaderModule({ code });
  let said = "ok";
  try { await d.createComputePipelineAsync({ layout: "auto", compute: { module: m, entryPoint: cs } }); }
  catch (e) { said = (e.reason ?? "error") + ": " + e.message.split("\\n")[0]; }
  const v = await d.popErrorScope();
  if (v) said = "validation: " + v.message.split("\\n")[0];
  return said;
}`;
let tries = 0;
async function said(code) {
  tries++;
  const r = await call("Runtime.evaluate", { expression: `(${COMPILE})(${JSON.stringify(code)})`, awaitPromise: true, returnByValue: true });
  return r.result?.result?.value ?? "?";
}
const refuses = async (lines) => (await said(lines.join("\n"))).startsWith("internal");

let lines = fs.readFileSync(process.argv[2], "utf8").split("\n");
if (!(await refuses(lines))) { console.log("not refused to begin with"); process.exit(1); }

function blocks(ls) {
  const st = [], out = [];
  ls.forEach((l, i) => { const t = l.trim(); if (t.startsWith("}") && st.length) out.push([st.pop(), i]); if (t.endsWith("{")) st.push(i); });
  return out;
}
function candidates(ls) {
  const c = [];
  for (const [o, e] of blocks(ls)) {
    const t = ls[o].trim();
    if (t.startsWith("fn ") || t.startsWith("continuing") || t.startsWith("struct") || t.includes("guard_")) continue;
    if (t.startsWith("loop")) c.push({ size: e - o + 1, apply: (x) => [...x.slice(0, o), ...x.slice(e + 1)] });
    else if (e - o > 1) c.push({ size: e - o - 1, apply: (x) => [...x.slice(0, o + 1), ...x.slice(e)] });
  }
  ls.forEach((l, i) => {
    if (/^\s*(phi_\w+|local(_\d+)?|global_\d+\.member\[.*\]) = .*;$/.test(l)) c.push({ size: 1, apply: (x) => [...x.slice(0, i), ...x.slice(i + 1)] });
  });
  ls.forEach((l, i) => {
    const m = l.match(/^(\s*)if (.+) \{$/);
    if (m && !l.includes("guard_") && m[2] !== "true" && m[2] !== "false") {
      for (const v of ["true", "false"]) c.push({ size: 0.5, apply: (x) => { const y = [...x]; y[i] = `${m[1]}if ${v} {`; return y; } });
    }
    const d = l.match(/^\s*let (_e\d+) = /);
    if (d && !ls.some((o, j) => j !== i && new RegExp(`\\b${d[1]}\\b`).test(o))) {
      c.push({ size: 0.9, apply: (x) => [...x.slice(0, i), ...x.slice(i + 1)] });
    }
    const v = l.match(/^\s*var (\w+): /);
    if (v && !v[1].startsWith("guard") && !l.includes("<") && !ls.some((o, j) => j !== i && new RegExp(`\\b${v[1]}\\b`).test(o))) {
      c.push({ size: 0.9, apply: (x) => [...x.slice(0, i), ...x.slice(i + 1)] });
    }
  });
  return c.sort((a, b) => b.size - a.size);
}
let changed = true;
while (changed) {
  changed = false;
  for (const c of candidates(lines)) {
    const next = c.apply(lines);
    if (await refuses(next)) { lines = next; changed = true; console.log(`kept a cut of ${c.size}; ${lines.length} lines; ${tries} tries`); break; }
  }
}
fs.writeFileSync(process.argv[3], lines.join("\n"));
console.log(`done: ${lines.length} lines after ${tries} tries`);
ws.close();
