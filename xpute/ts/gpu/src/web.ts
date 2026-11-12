// @xpute/gpu/web.ts

/**
 * A page's GPU, served to a core in the page's worker (wit/gpu/gpu.wit): the
 * runtime (runtime.ts) runs the records the core lends to `gpu.run` on the
 * device the core's `device` holds, and draws on the canvases given beside
 * the link under their surface's handle (wit/gpu/surface.wit). The device is
 * asked for when the core asks it ready, and a refusal is answered, not
 * retried: the core asks again when it sees fit. A WebAssembly module reaches
 * WebGPU no other way.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import { type Link, uri } from "@xpute/runtime";
import * as gpu from "./abi/gpu.ts";
import type { GpuAsk, GpuConfig, GpuState } from "./gpu.ts";
import { GpuRuntime } from "./runtime.ts";
import { STATUS } from "./api.spec.ts";

/** The URI the core opens its device at. */
export const DEVICE = uri("xpute:gpu", "device");

export class WebGpu {
  private runtime: GpuRuntime | null = null;
  /** The canvases given, by their surface's handle. */
  private readonly canvases = new Map<u32, OffscreenCanvas>();
  /** What waits for the next completion, and for the device's loss. */
  private settled: (() => void)[] = [];
  private lost: (() => void)[] = [];
  /** Where the core lent the device's block, while it is lent. */
  private block: u32 | null = null;
  private readonly link: Link;
  private readonly ask: GpuAsk;
  private readonly cfg: GpuConfig;
  private readonly report: (state: GpuState) => void;

  /** `report` hears what the page has of a GPU, as each ask finds it. */
  constructor(link: Link, ask: GpuAsk, cfg: GpuConfig, report: (state: GpuState) => void) {
    this.link = link;
    this.ask = ask;
    this.cfg = cfg;
    this.report = report;
  }

  /** Mounts `gpu` and the device's opener on the link. */
  serve(): void {
    gpu.mount(this.link, {
      run: async (records) => {
        this.runtime?.run(new Uint32Array(this.link.mem, records.at, records.len / 4));
      },
    });
    this.link.mount(DEVICE, () =>
      gpu.device(this.link, {
        lost: (block) => {
          this.block = block.at;
          this.runtime?.attach(block.at);
          return new Promise<void>((done) => this.lost.push(done));
        },
        ready: () => this.acquire(),
        settled: () => new Promise<void>((done) => this.settled.push(done)),
      }),
    );
  }

  /** The canvas given under the surface `handle` (remote.ts `give`). */
  bind(handle: u32, canvas: OffscreenCanvas): void {
    this.canvases.set(handle, canvas);
    this.runtime?.bind(handle, canvas.getContext("webgpu")!);
  }

  /** The surface `handle` was released, and its canvas goes with it. */
  unbind(handle: u32): void {
    this.canvases.delete(handle);
    this.runtime?.unbind(handle);
  }

  /** Asks for a device as the core asks, and answers the block's STATUS once
   * it is written. A page with no WebGPU, or no adapter, answers ENODEV.
   * `navigator.gpu` exists only in a secure context, so a LAN address over
   * http has none while localhost has it. */
  private async acquire(): Promise<u32> {
    if (this.runtime !== null) return STATUS.READY;
    if (!("gpu" in navigator)) {
      const why = isSecureContext ? "unsupported" : "insecure";
      console.warn(`[gpu] cannot draw here: ${why}`);
      this.report(why);
      throw new FaultError(Errno.ENODEV);
    }
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: this.ask.power as GPUPowerPreference });
    if (!adapter) {
      console.warn("[gpu] cannot draw here: no adapter answered");
      this.report("missing");
      throw new FaultError(Errno.ENODEV);
    }
    const { vendor, architecture, device, description } = adapter.info;
    console.info(`[gpu] adapter: ${[vendor, architecture, device, description].filter(Boolean).join(" · ")}`);
    let d: GPUDevice;
    try {
      d = await adapter.requestDevice({
        requiredFeatures: [
          ...(this.ask.features.required as GPUFeatureName[]),
          ...(this.ask.features.optional as GPUFeatureName[]).filter((f) => adapter.features.has(f)),
        ],
      });
    } catch (e) {
      // A feature the core requires is missing, or the adapter refused.
      console.warn(`[gpu] no device: ${e instanceof Error ? e.message : String(e)}`);
      this.report("missing");
      throw new FaultError(Errno.ENODEV);
    }
    const r = new GpuRuntime(d, () => this.link.mem, () => this.tell(this.settled), this.cfg);
    d.lost.then((info) => {
      // Nothing here destroys the device, so every loss is the browser's;
      // the runtime has written LOST into the block.
      console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
      if (this.runtime !== r) return;
      this.runtime = null;
      this.block = null;
      // The core asks for a device again.
      this.report("waiting");
      this.tell(this.lost);
    });
    this.canvases.forEach((canvas, handle) => r.bind(handle, canvas.getContext("webgpu")!));
    this.runtime = r;
    if (this.block !== null) r.attach(this.block);
    this.report("present");
    return STATUS.READY;
  }

  private tell(waiting: (() => void)[]): void {
    for (const done of waiting.splice(0)) done();
  }
}
