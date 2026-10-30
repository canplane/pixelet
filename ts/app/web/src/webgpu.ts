// @pxl/web/webgpu.ts

/**
 * The page's WebGPU: the device, and xpute's host (@xpute/gpu/host.ts)
 * running on it the calls the kernel's renderer wrote in a turn. A wasm
 * module reaches WebGPU no other way. The canvases the shell hands over are
 * kept here by the kernel's handle, each bound to every device's host, which
 * guards its slot (GpuHost `bind`, `unbind`).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { GpuHost } from "@xpute/gpu/host.ts";

import { DEV_BUNDLE } from "./bundle.ts";

let host: GpuHost | null = null;
/** The kernel's memory, which the host reads the stream from and writes
 * answers into; given once by the guest's open (guest.ts), as it never moves. */
let memory: ArrayBuffer | null = null;
/** A canvas transfers once: an effect mounted again is handed the same one. */
const transferred = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
/** In the order they were bound, so a device acquired again binds each slot's newest last. */
const canvases = new Map<u32, OffscreenCanvas>();

export const enum Missing {
  WebGpu = "web.webgpu",
  Insecure = "web.insecure",
  Gpu = "web.gpu",
}

/** `navigator.gpu` exists only in a secure context, so the LAN address over
 * http has none where localhost has it. */
export async function missing(): Promise<Missing | null> {
  const absent = await look();
  if (absent !== null) {
    console.error(`[gpu] cannot draw here: ${MISSING[absent]}`);
  }
  return absent;
}

const MISSING: Record<Missing, string> = {
  [Missing.WebGpu]: "this browser has no WebGPU",
  [Missing.Insecure]: "WebGPU needs a secure context, and this page is not one",
  [Missing.Gpu]: "WebGPU is here but no adapter answered",
};

async function look(): Promise<Missing | null> {
  if (!("gpu" in navigator)) {
    return isSecureContext ? Missing.WebGpu : Missing.Insecure;
  }
  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: "high-performance",
  });
  return adapter ? null : Missing.Gpu;
}

/** False where no adapter answered. What the device is, the kernel reads from
 * the block its first call asks the host for. `wake` is what a completion
 * calls to ask the turn that reads it (@xpute/gpu/host.ts `GpuHost`). */
export async function acquire(
  lost: () => void,
  wake: () => void,
): Promise<boolean> {
  const adapter = await navigator.gpu.requestAdapter({
    powerPreference: "high-performance",
  });
  if (!adapter) return false;
  // What the kernel asks for where the adapter has it: the timestamps its
  // spans are timed by, and a stencil beside the world's depth (pxl-render
  // gpu/api.rs `Device`).
  const wanted: GPUFeatureName[] = ["timestamp-query", "depth32float-stencil8"];
  const d = await adapter.requestDevice({
    requiredFeatures: wanted.filter((f) => adapter.features.has(f)),
  });
  // A compatibility-mode WebGPU (GLES 3.1 hardware) may offer no storage
  // buffers in the vertex stage, which the terrain needs; it shows here only.
  if (DEV_BUNDLE) {
    const l = d.limits as unknown as Record<string, number>;
    const of = (k: string) => `${k}=${l[k] ?? "-"}`;
    const info = (adapter as unknown as { info?: Record<string, string> }).info;
    console.info(
      `[gpu] adapter ${
        info
          ? `${info.vendor ?? "?"}/${info.architecture ?? "?"}/${
              info.device ?? "?"
            }`
          : "?"
      } features [${[...d.features].join(",")}] ` +
        [
          "maxStorageBuffersPerShaderStage",
          "maxStorageBuffersInVertexStage",
          "maxStorageBufferBindingSize",
          "maxBufferSize",
          "minStorageBufferOffsetAlignment",
          "minUniformBufferOffsetAlignment",
          "maxBindGroups",
        ]
          .map(of)
          .join(" "),
    );
  }
  const h = new GpuHost(d, () => memory!, wake);
  d.lost.then((info) => {
    // Nothing here destroys the device, so a loss is always the browser's.
    console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
    if (host !== h) return;
    host = null;
    lost();
  });
  d.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
  canvases.forEach((canvas, handle) =>
    h.bind(handle, canvas.getContext("webgpu")!),
  );
  host = h;
  return true;
}

export function bind(handle: u32, canvas: HTMLCanvasElement): void {
  let offscreen = transferred.get(canvas);
  if (!offscreen) {
    offscreen = canvas.transferControlToOffscreen();
    transferred.set(canvas, offscreen);
  }
  canvases.delete(handle);
  canvases.set(handle, offscreen);
  host?.bind(handle, offscreen.getContext("webgpu")!);
}

export function unbind(handle: u32): void {
  canvases.delete(handle);
  host?.unbind(handle);
}

export function take(of: ArrayBuffer): void {
  memory = of;
}

/** With no device the records are dropped: the kernel remakes everything on the next one. */
export function run(at: u32, head: u32): void {
  host?.run(at, head);
}
