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

let host: GpuHost | null = null;
/** The kernel's memory, which the host reads the stream from and writes
 * answers into; given once by the guest's open (guest.ts), as it never moves. */
let memory: ArrayBuffer | null = null;
/** A canvas transfers once: an effect mounted again is handed the same one. */
const offscreens = new WeakMap<HTMLCanvasElement, OffscreenCanvas>();
/** In the order they were bound, so a device acquired again binds each slot's newest last. */
const canvases = new Map<u32, OffscreenCanvas>();

/** Why this page has no WebGPU, none where it has it: `navigator.gpu` exists only
 * in a secure context, so the LAN address over http has none where localhost has it. */
export function absent(): "unsupported" | "insecure" | null {
  if ("gpu" in navigator) return null;
  return isSecureContext ? "unsupported" : "insecure";
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
  const { vendor, architecture, device, description } = adapter.info;
  console.info(
    `[gpu] adapter: ${[vendor, architecture, device, description].filter(Boolean).join(" · ")}`,
  );
  // What the kernel asks for: a stencil beside the world's depth
  // (pxl-render gpu/api.rs `GATED_DEPTH`), and the timestamps its spans are
  // timed by where the adapter has them (`Device`).
  const wanted: GPUFeatureName[] = ["timestamp-query"];
  const d = await adapter.requestDevice({
    requiredFeatures: [
      "depth32float-stencil8",
      ...wanted.filter((f) => adapter.features.has(f)),
    ],
  });
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
  let offscreen = offscreens.get(canvas);
  if (!offscreen) {
    offscreen = canvas.transferControlToOffscreen();
    offscreens.set(canvas, offscreen);
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
