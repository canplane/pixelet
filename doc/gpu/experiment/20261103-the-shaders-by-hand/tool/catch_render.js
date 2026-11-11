// Opens the app on the phone's Chrome with a hook that keeps each render
// pipeline the device refuses, its modules' WGSL, then writes them here.
import fs from "node:fs";
const PORT = 9333;
const tabs = await (await fetch(`http://localhost:${PORT}/json`)).json();
const t = tabs.find((x) => x.type === "page");
const ws = new WebSocket(t.webSocketDebuggerUrl);
let id = 0; const waits = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); } };
await new Promise((r) => (ws.onopen = r));
const call = (method, params) => new Promise((r) => { const i = ++id; waits.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const HOOK = `(() => {
  const codes = new WeakMap();
  window.__refused = [];
  const sm = GPUDevice.prototype.createShaderModule;
  GPUDevice.prototype.createShaderModule = function (d) { const m = sm.call(this, d); codes.set(m, d.code); return m; };
  const rp = GPUDevice.prototype.createRenderPipelineAsync;
  GPUDevice.prototype.createRenderPipelineAsync = function (d) {
    return rp.call(this, d).catch((e) => {
      window.__refused.push({ label: d.label, message: e.message, vertex: d.vertex.entryPoint, fragment: d.fragment && d.fragment.entryPoint,
        vcode: codes.get(d.vertex.module), fcode: d.fragment && d.fragment.module !== d.vertex.module ? codes.get(d.fragment.module) : null,
        targets: d.fragment && d.fragment.targets.map((x) => x && x.format), depth: d.depthStencil && d.depthStencil.format });
      throw e;
    });
  };
})();`;
await call("Page.enable", {});
await call("Page.addScriptToEvaluateOnNewDocument", { source: HOOK });
await call("Page.navigate", { url: "http://localhost:5173/" });
await new Promise((r) => setTimeout(r, Number(process.env.WAIT ?? 60000)));
const r = await call("Runtime.evaluate", { expression: "JSON.stringify(window.__refused)", returnByValue: true });
const refused = JSON.parse(r.result.result.value ?? "[]");
fs.mkdirSync("refused", { recursive: true });
refused.forEach((x, k) => {
  fs.writeFileSync(`refused/${k}.wgsl`, x.vcode ?? "");
  if (x.fcode) fs.writeFileSync(`refused/${k}.frag.wgsl`, x.fcode);
  console.log(k, x.label, x.vertex, x.fragment, x.targets, x.depth, (x.vcode ?? "").length, x.message.split("\n")[0]);
});
ws.close();
