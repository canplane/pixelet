// The worker's side: WebGPU on the canvas handed in, its own animation frames,
// the text, and the replies the page times.

import { measure } from "./text.js";

let device = null, ctx = null, format = null, presented = 0, readback = null;

async function start(canvas) {
  const out = {
    gpu: "gpu" in self.navigator,
    raf: typeof self.requestAnimationFrame === "function",
    offscreen: typeof OffscreenCanvas !== "undefined",
    fonts: "fonts" in self,
  };
  try { out.text = await measure(); } catch (e) { out.text_error = String(e); }
  if (!out.gpu) return out;
  try {
    const adapter = await navigator.gpu.requestAdapter();
    out.adapter = !!adapter;
    device = await adapter.requestDevice();
    out.device = true;
  } catch (e) { out.device_error = String(e); return out; }
  if (canvas) {
    try {
      ctx = canvas.getContext("webgpu");
      format = navigator.gpu.getPreferredCanvasFormat();
      ctx.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
      out.context = !!ctx;
      out.format = format;
    } catch (e) { out.context_error = String(e); }
  }
  return out;
}

/** One frame cleared to a known color; the first one also copied out to read. */
function draw(read) {
  if (!ctx) return;
  const tex = ctx.getCurrentTexture();
  const enc = device.createCommandEncoder();
  enc.beginRenderPass({ colorAttachments: [{ view: tex.createView(), loadOp: "clear", storeOp: "store", clearValue: { r: 1, g: 0.5, b: 0, a: 1 } }] }).end();
  if (read) {
    readback = device.createBuffer({ size: 256, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    enc.copyTextureToBuffer({ texture: tex }, { buffer: readback, bytesPerRow: 256 }, [1, 1]);
  }
  device.queue.submit([enc.finish()]);
  presented++;
}

function frames(ms) {
  return new Promise((done) => {
    const ts = [];
    const until = performance.now() + ms;
    if (typeof self.requestAnimationFrame !== "function") {
      const tick = () => { draw(false); performance.now() < until ? setTimeout(tick, 16) : done({ raf: false, presented }); };
      return tick();
    }
    const f = (t) => { ts.push(t); draw(false); performance.now() < until ? requestAnimationFrame(f) : done({ raf: true, ts, presented }); };
    requestAnimationFrame(f);
  });
}

async function pixel() {
  if (!ctx) return { none: true };
  draw(true);
  await readback.mapAsync(GPUMapMode.READ);
  const px = [...new Uint8Array(readback.getMappedRange(), 0, 4)];
  readback.unmap();
  return { format, px };
}

const mem = new ArrayBuffer(4 << 20);

self.onmessage = async ({ data }) => {
  const { id, kind } = data;
  if (kind === "echo") return postMessage({ id });
  if (kind === "start") return postMessage({ id, ...(await start(data.canvas)) });
  if (kind === "frames") return postMessage({ id, ...(await frames(data.ms)) });
  if (kind === "pixel") return postMessage({ id, ...(await pixel()) });
  if (kind === "slab") {
    const a = performance.now();
    const buf = mem.slice(0, data.bytes);
    const slice_ms = performance.now() - a;
    return postMessage({ id, buf, slice_ms }, [buf]);
  }
};
