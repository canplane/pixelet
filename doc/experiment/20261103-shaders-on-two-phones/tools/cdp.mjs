// Compiles WGSL files on the phone's Chrome (adb forward tcp:9333) and prints
// each result: node cdp.mjs a.wgsl [b.wgsl …]
import fs from "node:fs";

const PORT = 9333;
const PAGE = "http://localhost:5173/probe.html?only=__none";

async function target() {
  const tabs = await (await fetch(`http://localhost:${PORT}/json`)).json();
  const pages = tabs.filter((x) => x.type === "page");
  const t = pages.find((x) => x.url.startsWith("http://localhost:5173/")) ?? pages[0];
  return { t, fresh: !t.url.startsWith("http://localhost:5173/") };
}

function client(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const waits = new Map();
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); }
  };
  const open = new Promise((r) => (ws.onopen = r));
  return {
    open,
    call: (method, params) => new Promise((r) => { const i = ++id; waits.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); }),
    close: () => ws.close(),
  };
}

const COMPILE = `async (code, label) => {
  if (!window.__dev || window.__lost) {
    const a = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
    window.__dev = await a.requestDevice(); window.__lost = false; window.__dev.lost.then(() => (window.__lost = true));
  }
  const d = window.__dev;
  const cs = code.match(/@compute[^]*?\\bfn\\s+(\\w+)\\s*\\(/)?.[1];
  const t0 = performance.now();
  d.pushErrorScope("validation");
  const m = d.createShaderModule({ label, code });
  let said = "ok";
  try { await d.createComputePipelineAsync({ label, layout: "auto", compute: { module: m, entryPoint: cs } }); }
  catch (e) { said = (e.reason ?? "error") + ": " + e.message.split("\\n")[0]; }
  const v = await d.popErrorScope();
  if (v) said += " | validation: " + v.message.split("\\n")[0];
  return said + " (" + Math.round(performance.now() - t0) + " ms)";
}`;

const { t, fresh } = await target();
const c = client(t.webSocketDebuggerUrl);
await c.open;
if (fresh || process.env.RELOAD) {
  await c.call("Page.navigate", { url: PAGE });
  await new Promise((r) => setTimeout(r, 4000));
}
for (const file of process.argv.slice(2)) {
  const code = fs.readFileSync(file, "utf8");
  const r = await c.call("Runtime.evaluate", {
    expression: `(${COMPILE})(${JSON.stringify(code)}, ${JSON.stringify(file.split("/").pop())})`,
    awaitPromise: true,
    returnByValue: true,
  });
  { const v = r.result?.result?.value; console.log(`${file.split("/").pop()}: ${typeof v === "string" ? v : JSON.stringify(r).slice(0, 400)}`); }
}
c.close();
