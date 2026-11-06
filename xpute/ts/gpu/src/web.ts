// @xpute/gpu/web.ts

/**
 * A page's GPU, granted to a core (@xpute/runtime `Gpu`): the device asked for
 * as the core's machine asks, asked again after a loss or a refusal while the
 * page is shown; the core's stream run on it by the runtime (runtime.ts) as each
 * turn returns; the core told when a device answers; and the canvases the
 * program binds by the core's handle. A wasm module reaches WebGPU no other
 * way.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Gpu, GpuGrant } from "@xpute/runtime";
import { each_record } from "@xpute/runtime/ipc/stream.ts";
import { GpuRuntime } from "./runtime.ts";

/** Whether there is a GPU: one, none yet while it asks again, none it will
 * ask for again, none again after one it drew with, no WebGPU here, or none
 * outside a secure context. */
export type GpuState =
  "present" | "waiting" | "missing" | "lost" | "unsupported" | "insecure";

/** Safari, after losing its GPU process, gave no device for a while, so the
 * page keeps asking. The first retry waits too, or an adapter returning an
 * already-lost device loops at once. */
const RETRY_MS = 1000;

/** Asks before the GPU is given up, about ten seconds of them. */
const TRIES = 10;

export class WebGpu implements Gpu {
  private runtime: GpuRuntime | null = null;
  private granted: GpuGrant | null = null;
  /** What the program is told of the GPU, and the last it was told. */
  private report: (state: GpuState) => void = () => {};
  private state: GpuState | null = null;
  /** So an outage is logged once at start and once at end. */
  private tries = 0;
  /** A device answered once: giving up after it is losing the GPU, not lacking one. */
  private had = false;
  /** In the order they were bound, so a device acquired again binds each
   * slot's newest last. */
  private readonly canvases = new Map<
    u32,
    HTMLCanvasElement | OffscreenCanvas
  >();

  /** `report` is told whether there is a GPU, the last word at once. */
  watch(report: (state: GpuState) => void): void {
    this.report = report;
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

  /** With no device the records are dropped, the stream handed back unrun:
   * the core remakes everything on the next one. */
  after_turn(): void {
    if (!this.granted) return;
    if (this.runtime) this.runtime.run(this.granted.at);
    else each_record(this.granted.mem, this.granted.at, () => {});
  }

  /** A canvas drawn where it is: a page's own, or the one a page transferred
   * to the worker its runtime runs in. */
  bind(handle: u32, canvas: HTMLCanvasElement | OffscreenCanvas): void {
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
    this.report(state);
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

  /** A hidden page draws nothing, so it waits to be shown; a worker, which
   * has no document, asks again on the timer. */
  private retry(): void {
    if (
      typeof document === "undefined" ||
      document.visibilityState === "visible"
    ) {
      setTimeout(() => this.request(), RETRY_MS);
      return;
    }
    const shown = () => {
      if (document.visibilityState !== "visible") return;
      document.removeEventListener("visibilitychange", shown);
      this.request();
    };
    document.addEventListener("visibilitychange", shown);
  }

  /** False where no adapter answered. What the device is, the core reads
   * from the block its first call asks the runtime for. */
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
      // Nothing here destroys the device, so a loss is always the browser's;
      // the core hears of it from the runtime, and asking again is the socket's.
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

/** Why this page has no WebGPU, none where it has it: `navigator.gpu` exists
 * only in a secure context, so a LAN address over http has none where
 * localhost has it. */
function absent(): "unsupported" | "insecure" | null {
  if ("gpu" in navigator) return null;
  return isSecureContext ? "unsupported" : "insecure";
}
