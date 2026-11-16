// @pxl/gpu/host/src/web.ts

/**
 * A page's GPU, implemented for a guest in the page's worker
 * (spec/gpu/wit/gpu.wit): the host (host.ts) runs the records the guest lends
 * to `gpu.run` on the device the guest's `device` holds, and draws on the
 * canvases given beside the link under their surface's handle
 * (spec/gpu/wit/surface.wit). The device is asked for when the guest asks it
 * ready, and a refusal is answered, not retried: the guest asks again when it
 * sees fit. A WebAssembly module reaches WebGPU no other way. What the page has
 * of a GPU this host tells the page itself as that changes, posted from the
 * worker (`GpuReport`), as a device's GPU host tells its app through its own
 * module (pxl-gpu-host native.rs `state_on`); the program's assembly hears it
 * on the page.
 */

import type { u32, u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { ComponentError, type Side } from "@xpute/link";
import * as gpu from "./abi/gpu.ts";
import { ERROR_CODE_NO_DEVICE } from "./abi/gpu.enum.ts";
import {
  type GpuAsk,
  type GpuConfig,
  type GpuState,
} from "../../link/src/mod.ts";
import { GpuHost } from "./host.ts";

/** What the worker posts the page as what it has of a GPU changes: an ask's
 * finding, or a device lost. */
export interface GpuReport {
  kind: "gpu";
  state: GpuState;
}

export class WebGpu {
  private host: GpuHost | null = null;
  /** The canvases given, by their surface's handle. */
  private readonly canvases = new Map<u32, OffscreenCanvas>();
  /** What waits for the next completion, and for the device's loss. */
  private settled: (() => void)[] = [];
  private lost: (() => void)[] = [];
  /** Where the guest lent the device's block, while it is lent. */
  private block: u64 | null = null;
  private readonly side: Side;
  private readonly ask: GpuAsk;
  private readonly cfg: GpuConfig;

  constructor(side: Side, ask: GpuAsk, cfg: GpuConfig) {
    this.side = side;
    this.ask = ask;
    this.cfg = cfg;
  }

  /** Mounts `gpu` on the side, whose `get_device` adds the device under
   * a handle the side issues. */
  mount(): void {
    gpu.mount(this.side, {
      run: async (recs) => {
        this.host?.run(
          new Uint32Array(this.side.mem, Number(recs.at), recs.len / 4),
        );
      },
      get_device: () =>
        this.side.add(
          gpu.device(this.side, {
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
      this.report(why);
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
    const adapter = await navigator.gpu.requestAdapter({
      powerPreference: this.ask.power as GPUPowerPreference,
    });
    if (!adapter) {
      this.report("missing");
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
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
    } catch {
      // A feature the guest requires is missing, or the adapter refused.
      this.report("missing");
      throw new ComponentError(ERROR_CODE_NO_DEVICE);
    }
    // A device the host holds as lost has written LOST into the block, and
    // is let go; the guest asks for a device again.
    const r: GpuHost = new GpuHost(
      d,
      () => this.side.mem,
      () => this.tell(this.settled),
      () => {
        if (this.host !== r) return;
        this.host = null;
        this.block = null;
        this.tell(this.lost);
        this.report("waiting");
      },
      this.cfg,
    );
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
