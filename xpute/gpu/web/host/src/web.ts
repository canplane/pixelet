// @xpute/gpu-web-host/web.ts

/**
 * A page's GPU, served to a guest in the page's worker (gpu/wit/gpu.wit): the
 * host (host.ts) runs the records the guest lends to `gpu.run` on the device
 * the guest's `device` holds, and draws on the canvases given beside the link
 * under their surface's handle (gpu/wit/surface.wit). The device is asked for
 * when the guest asks it ready, and a refusal is answered, not retried: the
 * guest asks again when it sees fit. A WebAssembly module reaches
 * WebGPU no other way.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { Link } from "@xpute/link";
import * as gpu from "@xpute/gpu-link/abi/gpu.ts";
import { DEVICE_URI, type GpuAsk, type GpuConfig, type GpuState } from "@xpute/gpu-link";
import { GpuHost } from "./host.ts";
import { STATUS } from "@xpute/gpu-link/api.spec.ts";

export class WebGpu {
  private host: GpuHost | null = null;
  /** The canvases given, by their surface's handle. */
  private readonly canvases = new Map<u32, OffscreenCanvas>();
  /** What waits for the next completion, and for the device's loss. */
  private settled: (() => void)[] = [];
  private lost: (() => void)[] = [];
  /** Where the guest lent the device's block, while it is lent. */
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
        this.host?.run(new Uint32Array(this.link.mem, records.at, records.len / 4));
      },
    });
    this.link.mount(DEVICE_URI, () =>
      gpu.device(this.link, {
        lost: (block, signal) => {
          this.block = block.at;
          this.host?.attach(block.at);
          // A loss its caller cancels hands the block back, and the host
          // writes it no more.
          signal.addEventListener("abort", () => {
            this.block = null;
            this.host?.detach();
          });
          return this.wait(this.lost, signal);
        },
        ready: () => this.acquire(),
        settled: (signal) => this.wait(this.settled, signal),
      }),
    );
  }

  /** The canvas given under the surface `handle` (remote.ts `give`). */
  bind(handle: u32, canvas: OffscreenCanvas): void {
    this.canvases.set(handle, canvas);
    this.host?.bind(handle, canvas.getContext("webgpu")!);
  }

  /** The surface `handle` was released, and its canvas goes with it. */
  unbind(handle: u32): void {
    this.canvases.delete(handle);
    this.host?.unbind(handle);
  }

  /** Asks for a device as the guest asks, and answers the block's STATUS once
   * it is written. A page with no WebGPU, or no adapter, answers ENODEV.
   * `navigator.gpu` exists only in a secure context, so a LAN address over
   * http has none while localhost has it. */
  private async acquire(): Promise<u32> {
    if (this.host !== null) return STATUS.READY;
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
          ...(this.ask.required_feats as GPUFeatureName[]),
          ...(this.ask.optional_feats as GPUFeatureName[]).filter((f) => adapter.features.has(f)),
        ],
      });
    } catch (e) {
      // A feature the guest requires is missing, or the adapter refused.
      console.warn(`[gpu] no device: ${e instanceof Error ? e.message : String(e)}`);
      this.report("missing");
      throw new FaultError(Errno.ENODEV);
    }
    const r = new GpuHost(d, () => this.link.mem, () => this.tell(this.settled), this.cfg);
    d.lost.then((info) => {
      // Nothing here destroys the device, so every loss is the browser's;
      // the host has written LOST into the block.
      console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
      if (this.host !== r) return;
      this.host = null;
      this.block = null;
      // The guest asks for a device again.
      this.report("waiting");
      this.tell(this.lost);
    });
    this.canvases.forEach((canvas, handle) => r.bind(handle, canvas.getContext("webgpu")!));
    this.host = r;
    if (this.block !== null) r.attach(this.block);
    this.report("present");
    return STATUS.READY;
  }

  private tell(waiting: (() => void)[]): void {
    for (const done of waiting.splice(0)) done();
  }

  /** Waits in `waiting` until it is told, or answers ECANCELED once `signal`
   * aborts, its caller having canceled it. */
  private wait(waiting: (() => void)[], signal: AbortSignal): Promise<void> {
    return new Promise<void>((done, fail) => {
      waiting.push(done);
      signal.addEventListener("abort", () => {
        waiting.splice(waiting.indexOf(done) >>> 0, 1);
        fail(new FaultError(Errno.ECANCELED));
      });
    });
  }
}
