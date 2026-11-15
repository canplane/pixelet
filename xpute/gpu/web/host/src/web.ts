// @xpute/gpu-web-host/web.ts

/**
 * A page's GPU, served to a guest in the page's worker (gpu/wit/gpu.wit): the
 * host (host.ts) runs the records the guest lends to `gpu.run` on the device
 * the guest's `device` holds, and draws on the canvases given beside the link
 * under their surface's handle (gpu/wit/surface.wit). The device is asked for
 * when the guest asks it ready, and a refusal is answered, not retried: the
 * guest asks again when it sees fit. A WebAssembly module reaches
 * WebGPU no other way. What the page has of a GPU this server tells the page
 * itself as that changes, posted from the worker (`GpuReport`), as a device's
 * GPU host tells its app through its own module (xpute-gpu-native-host
 * native.rs `state_on`) and as the page's runtime posts its own
 * (@xpute/web-runtime worker.ts `tell`); the UI hears it (`state_on`).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError, type Side } from "@xpute/link";
import { message_on } from "@xpute/web-runtime";
import * as gpu from "@xpute/gpu-link/abi/gpu.ts";
import { ERROR_CODE_NO_DEVICE } from "@xpute/gpu-link/abi/gpu.enum.ts";
import { type GpuAsk, type GpuConfig, type GpuState } from "@xpute/gpu-link";
import { GpuHost } from "./host.ts";

/** What the worker posts the page as what it has of a GPU changes: an ask's
 * finding, or a device lost. */
export interface GpuReport {
  kind: "gpu";
  state: GpuState;
}

/** What the host has of a GPU, as the page was told it last, or null before,
 * as a device's GPU host keeps it (@xpute/gpu-native-runtime `state_on`). */
let state: GpuState | null = null;
message_on<GpuReport>("gpu", (m) => {
  state = m.state;
});

/** Adds `hear` to what the UI is told of what the host has of a GPU, as it
 * changes (`GpuReport`), and tells it the state now, as
 * @xpute/gpu-native-runtime `state_on` tells a device's UI. */
export function state_on(hear: (state: GpuState) => void): { remove(): void } {
  const sub = message_on<GpuReport>("gpu", (m) => hear(m.state));
  if (state !== null) hear(state);
  return sub;
}

export class WebGpu {
  private host: GpuHost | null = null;
  /** The canvases given, by their surface's handle. */
  private readonly canvases = new Map<u32, OffscreenCanvas>();
  /** What waits for the next completion, and for the device's loss. */
  private settled: (() => void)[] = [];
  private lost: (() => void)[] = [];
  /** Where the guest lent the device's block, while it is lent. */
  private block: u32 | null = null;
  private readonly link: Side;
  private readonly ask: GpuAsk;
  private readonly cfg: GpuConfig;

  constructor(link: Side, ask: GpuAsk, cfg: GpuConfig) {
    this.link = link;
    this.ask = ask;
    this.cfg = cfg;
  }

  /** Mounts `gpu` on the link, whose `get_device` serves the device under
   * a handle the link issues. */
  serve(): void {
    gpu.mount(this.link, {
      run: async (recs) => {
        this.host?.run(new Uint32Array(this.link.mem, recs.at, recs.len / 4));
      },
      get_device: () =>
        this.link.add(
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
            // The device is the host's, and outlives the guest's handle.
            drop: () => {},
          }),
        ),
    });
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

  /** Asks for a device as the guest asks, and answers once its block is
   * written; a page with no WebGPU, or no adapter, answers `no-device`.
   * `navigator.gpu` exists only in a secure context, so a LAN address over
   * http has none while localhost has it. */
  private async acquire(): Promise<void> {
    if (this.host !== null) return;
    if (!("gpu" in navigator)) {
      const why = isSecureContext ? "unsupported" : "insecure";
      console.warn(`[gpu] cannot draw here: ${why}`);
      this.report(why);
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
    const adapter = await navigator.gpu.requestAdapter({
      powerPreference: this.ask.power as GPUPowerPreference,
    });
    if (!adapter) {
      console.warn("[gpu] cannot draw here: no adapter answered");
      this.report("missing");
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
    const { vendor, architecture, device, description } = adapter.info;
    console.info(
      `[gpu] adapter: ${[vendor, architecture, device, description].filter(Boolean).join(" · ")}`,
    );
    let d: GPUDevice;
    try {
      d = await adapter.requestDevice({
        requiredFeatures: [
          ...(this.ask.required_feats as GPUFeatureName[]),
          ...(this.ask.optional_feats as GPUFeatureName[]).filter((f) =>
            adapter.features.has(f),
          ),
        ],
      });
    } catch (e) {
      // A feature the guest requires is missing, or the adapter refused.
      console.warn(
        `[gpu] no device: ${e instanceof Error ? e.message : String(e)}`,
      );
      this.report("missing");
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
    const r = new GpuHost(
      d,
      () => this.link.mem,
      () => this.tell(this.settled),
      this.cfg,
    );
    d.lost.then((info) => {
      // Nothing here destroys the device, so every loss is the browser's;
      // the host has written LOST into the block.
      console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
      if (this.host !== r) return;
      this.host = null;
      this.block = null;
      // The guest asks for a device again.
      this.tell(this.lost);
      this.report("waiting");
    });
    this.canvases.forEach((canvas, handle) =>
      r.bind(handle, canvas.getContext("webgpu")!),
    );
    this.host = r;
    if (this.block !== null) r.attach(this.block);
    this.report("present");
  }

  /** Posts the page what it has of a GPU (`GpuReport`). */
  private report(state: GpuState): void {
    postMessage({ kind: "gpu", state } satisfies GpuReport);
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
        fail(new XputeError(ERRNO_ECANCELED));
      });
    });
  }
}
