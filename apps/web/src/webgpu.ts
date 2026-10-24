// @pxl/web/webgpu.ts

/**
 * The page's WebGPU: the device, and xpute's host (@xpute/gpu/host.ts)
 * running on it the calls the kernel's renderer wrote in a turn. A wasm
 * module reaches WebGPU no other way. The canvases the shell hands over are
 * kept here, each bound to every device's host by its handle's slot.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { handle_slot } from "@xpute/host/abi/handle.ts";
import { GpuHost } from "@xpute/gpu/host.ts";

import { DEV_BUNDLE } from "@pxl/client/config/bundle.ts";
import { stream } from "@pxl/client/vmm/render/gpu/api.spec.ts";

let host: GpuHost | null = null;
/** The kernel's memory, which the host reads the stream from and writes answers into. */
let moduleMemory: ArrayBuffer | null = null;
/** A canvas transfers once: an effect mounted again is handed the same one. */
const transferred = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
const canvases: (OffscreenCanvas | undefined)[] = [];

export const enum Missing {
  WebGpu = "web.webgpu",
  Insecure = "web.insecure",
  Gpu = "web.gpu",
}

/** `navigator.gpu` exists only in a secure context, so the LAN address over
 * http has none where localhost has it. */
export async function whatIsMissing(): Promise<Missing | null> {
  const missing = await missingOf();
  if (missing !== null) console.error(`[gpu] cannot draw here: ${MISSING[missing]}`);
  return missing;
}

const MISSING: Record<Missing, string> = {
  [Missing.WebGpu]: "this browser has no WebGPU",
  [Missing.Insecure]: "WebGPU needs a secure context, and this page is not one",
  [Missing.Gpu]: "WebGPU is here but no adapter answered",
};

async function missingOf(): Promise<Missing | null> {
  if (!("gpu" in navigator)) return isSecureContext ? Missing.WebGpu : Missing.Insecure;
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  return adapter ? null : Missing.Gpu;
}

/** False where no adapter answered. What the device is, the kernel reads from
 * the block its first call asks the host for. */
/** `wake`: what a completion calls to ask the turn that reads it (xpute GpuHost). */
export async function acquireDevice(lost: () => void, wake: () => void): Promise<boolean> {
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  if (!adapter) return false;
  const timestamps = adapter.features.has("timestamp-query");
  const d = await adapter.requestDevice({ requiredFeatures: timestamps ? ["timestamp-query"] : [] });
  // A compatibility-mode WebGPU (GLES 3.1 hardware) may offer no storage
  // buffers in the vertex stage, which the terrain needs; it shows here only.
  if (DEV_BUNDLE) {
    const l = d.limits as unknown as Record<string, number>;
    const of = (k: string) => `${k}=${l[k] ?? "-"}`;
    const info = (adapter as unknown as { info?: Record<string, string> }).info;
    console.info(
      `[gpu] adapter ${info ? `${info.vendor ?? "?"}/${info.architecture ?? "?"}/${info.device ?? "?"}` : "?"} features [${[...d.features].join(",")}] ` +
        [
          "maxStorageBuffersPerShaderStage",
          "maxStorageBuffersInVertexStage",
          "maxStorageBufferBindingSize",
          "maxBufferSize",
          "minStorageBufferOffsetAlignment",
          "minUniformBufferOffsetAlignment",
          "maxBindGroups",
        ].map(of).join(" "),
    );
  }
  const h = new GpuHost(d, () => moduleMemory!, wake);
  d.lost.then((info) => {
    // Nothing here destroys the device, so a loss is always the browser's.
    console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
    if (host !== h) return;
    host = null;
    lost();
  });
  d.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
  canvases.forEach((canvas, slot) => {
    if (canvas) h.bindSurface(slot, canvas.getContext("webgpu")!);
  });
  host = h;
  return true;
}

export function bindSurface(handle: u32, canvas: HTMLCanvasElement): void {
  let offscreen = transferred.get(canvas);
  if (!offscreen) {
    offscreen = canvas.transferControlToOffscreen();
    transferred.set(canvas, offscreen);
  }
  const slot = handle_slot(handle);
  canvases[slot] = offscreen;
  host?.bindSurface(slot, offscreen.getContext("webgpu")!);
}

export function unbindSurface(handle: u32): void {
  const slot = handle_slot(handle);
  canvases[slot] = undefined;
  host?.unbindSurface(slot);
}

/** With no device the records are dropped: the kernel remakes everything on the next one. */
export function runGpuStream(memory: ArrayBuffer, at: u32): void {
  moduleMemory = memory;
  host?.run(at, stream.HEAD);
}
