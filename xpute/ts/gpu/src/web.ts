// @xpute/gpu/web.ts

/**
 * A page's GPU, granted to a core (@xpute/runtime `Gpu`). It asks for the
 * device the core's machine declares, and asks again after a loss or a
 * refusal while the page is shown. The runtime (runtime.ts) runs the core's
 * stream on the device as each turn returns. The core is told when a device
 * answers. The canvases the program binds are kept by the core's handle. A
 * WebAssembly module reaches WebGPU no other way.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Canvas, Gpu, GpuGrant, GpuState } from "@xpute/runtime";
import { each_record } from "@xpute/runtime/ipc/stream.ts";
import { GpuRuntime } from "./runtime.ts";

/** How long the page waits between asks for a device. Safari gives no device
 * for a while after it loses its GPU process, so the page keeps asking. The
 * first retry waits too, or an adapter that returns an already lost device
 * would loop at once. */
const RETRY_MS = 1000;

/** How many asks are made before the GPU is given up: about ten seconds of
 * them. */
const TRIES = 10;

export class WebGpu implements Gpu {
  private runtime: GpuRuntime | null = null;
  private granted: GpuGrant | null = null;
  /** Who is told of the GPU's state; `state` is the last one told. */
  private readonly _reports: ((state: GpuState) => void)[] = [];
  private state: GpuState | null = null;
  /** The asks made since a device last answered, so an outage is logged
   * once as it starts and once as it ends. */
  private tries = 0;
  /** A device has answered before, so giving up now is losing the GPU, not
   * lacking one. */
  private had = false;
  /** The bound canvases in the order they were bound, so a device acquired
   * again binds each slot's newest handle last. */
  private readonly canvases = new Map<u32, Canvas>();

  watch(report: (state: GpuState) => void): void {
    this._reports.push(report);
    if (this.state !== null) report(this.state);
  }

  grant(to: GpuGrant): void {
    this.granted = to;
    const why = absent();
    if (why !== null) {
      console.error(`[gpu] cannot draw here: ${why}`);
      this.tell(why);
      return;
    }
    this.request();
  }

  /** Runs the stream as the turn returns. With no device the records are
   * dropped and the stream is handed back unrun; the core remakes
   * everything on the next device. */
  after_turn(): void {
    if (!this.granted) return;
    if (this.runtime) this.runtime.run(this.granted.at);
    else each_record(this.granted.mem, this.granted.at, () => {});
  }

  /** `canvas` is drawn where it is: a page's own canvas, or one the page
   * transferred to the worker its runtime runs in. */
  bind(handle: u32, canvas: Canvas): void {
    this.canvases.delete(handle);
    this.canvases.set(handle, canvas);
    this.runtime?.bind(handle, canvas.getContext("webgpu")!);
  }

  unbind(handle: u32): void {
    this.canvases.delete(handle);
    this.runtime?.unbind(handle);
  }

  private tell(state: GpuState): void {
    this.state = state;
    for (const report of this._reports) report(state);
  }

  private request(): void {
    this.tries += 1;
    void this.acquire().then(
      (answered) => {
        if (!answered) {
          if (this.tries === 1) {
            console.warn(
              `[gpu] no adapter answered; asking every ${RETRY_MS} ms while the page is shown`,
            );
          }
          if (this.tries >= TRIES) {
            console.error(
              `[gpu] no adapter answered in ${this.tries} requests; nothing can be drawn`,
            );
            this.tell(this.had ? "lost" : "missing");
            return;
          }
          this.tell("waiting");
          return this.retry();
        }
        this.tell("present");
        this.had = true;
        if (this.tries > 1) {
          console.info(`[gpu] a device answered after ${this.tries} requests`);
        }
        this.tries = 0;
        this.granted?.ready();
      },
      (err) => {
        console.error(
          "[gpu] the device request failed; nothing can be drawn:",
          err,
        );
        this.tell(this.had ? "lost" : "missing");
      },
    );
  }

  /** A hidden page draws nothing, so an ask waits until the page is shown
   * and spends no try. A worker has no document, so the core's runtime says
   * whether the page is shown (`GpuGrant.shown`). */
  private retry(): void {
    setTimeout(
      () => (this.granted!.shown() ? this.request() : this.retry()),
      RETRY_MS,
    );
  }

  /** False where no adapter answered. The core learns what the device is
   * from the device's block, which it asks the runtime for. */
  private async acquire(): Promise<boolean> {
    const adapter = await navigator.gpu.requestAdapter({
      powerPreference: this.granted!.ask.power as GPUPowerPreference,
    });
    if (!adapter) return false;
    const { vendor, architecture, device, description } = adapter.info;
    console.info(
      `[gpu] adapter: ${[vendor, architecture, device, description].filter(Boolean).join(" · ")}`,
    );
    const d = await adapter.requestDevice({
      requiredFeatures: [
        ...(this.granted!.ask.features.required as GPUFeatureName[]),
        ...(this.granted!.ask.features.optional as GPUFeatureName[]).filter(
          (f) => adapter.features.has(f),
        ),
      ],
    });
    const h = new GpuRuntime(
      d,
      () => this.granted!.mem,
      () => this.granted?.wake(),
    );
    d.lost.then((info) => {
      // Nothing here destroys the device, so every loss is the browser's.
      // The runtime tells the core, and this class asks for a new device.
      console.warn(`[gpu] device lost (${info.reason}): ${info.message}`);
      if (this.runtime !== h) return;
      this.runtime = null;
      this.tell("waiting");
      this.retry();
    });
    d.onuncapturederror = (e) => console.error(`[gpu] ${e.error.message}`);
    this.canvases.forEach((canvas, handle) =>
      h.bind(handle, canvas.getContext("webgpu")!),
    );
    this.runtime = h;
    return true;
  }
}

/** Why this page has no WebGPU, or null where it has it. `navigator.gpu`
 * exists only in a secure context, so a LAN address over http has none while
 * localhost has it. */
function absent(): "unsupported" | "insecure" | null {
  if ("gpu" in navigator) return null;
  return isSecureContext ? "unsupported" : "insecure";
}
