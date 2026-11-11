// @xpute/gpu/gpu.ts

/**
 * The GPU as a plugin of a core's runtime: what a platform's GPU is, and how it
 * is granted the core. @xpute/runtime knows none of it.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Runtime } from "@xpute/runtime/runtime.ts";
import { say } from "@xpute/runtime/say.ts";
import { gpu_exports } from "./abi.spec.ts";

/** This is what a core asks of its GPU (its manifest's `gpu`), by WebGPU's
 * names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** These are the settings of a page's GPU runtime that the program chooses,
 * as xpute-gpu's `CoreConfig` gives a device's runtime its own. */
export interface GpuConfig {
  /** The size of one chunk that buffer writes are staged through (belt.ts),
   * as large as one frame's writes; a larger write is staged across several
   * chunks. */
  upload_chunk_bytes: u32;
  /** What the runtime does on the thread that runs it that takes longer than
   * this many milliseconds is said, as `[gpu] <what> took <ms> ms`, the whole
   * milliseconds: a record of the stream (`op <op>`, by its number), the
   * stream's run as a whole (`run`), and a completion's landing: a read's
   * (`map`), a staging chunk's mapped again (`remap`) and an image's
   * (`image`). With null, nothing is timed. */
  slow_ms: u32 | null;
}

/** Runs `f`, and where `slow_ms` is not null and `f` took longer, says
 * `[gpu] <what> took <ms> ms`, the whole milliseconds (`GpuConfig.slow_ms`). */
export function timed<T>(slow_ms: u32 | null, what: string, f: () => T): T {
  if (slow_ms === null) return f();
  const t0 = performance.now();
  const v = f();
  const ms = Math.trunc(performance.now() - t0);
  if (ms > slow_ms) say(`[gpu] ${what} took ${ms} ms`);
  return v;
}

/** This is a platform's GPU. `grant` opens a device as the core asks. `watch`
 * reports when a device answers and whether there is one. */
export interface Gpu {
  grant(to: GpuGrant): void;
  watch(report: (state: GpuState) => void): void;
}

export type GpuState =
  "present" | "waiting" | "missing" | "lost" | "unsupported" | "insecure";

/** This is how long a platform's GPU waits between asks for a device, while
 * none answers and after a loss. Safari gives no device for a while after it
 * loses its GPU process, so the page keeps asking. The first retry waits too,
 * or an adapter that returns an already lost device would loop at once. */
export const RETRY_MS = 1000;

/** This is how many asks are made before the GPU is given up: about ten
 * seconds of them. */
export const TRIES = 10;

/** This is what a platform's GPU gives the loop that asks it for a device.
 * `acquire` asks once and answers whether a device answered; a rejection is a
 * request that failed, not a device that was absent. `tell` hears the GPU's
 * state, `ready` is the grant's, and `shown` says whether the core is shown. */
export interface Acquire {
  acquire(): Promise<boolean>;
  tell(state: GpuState): void;
  ready(): void;
  shown(): boolean;
}

/** This asks a platform for a device until one answers, the first time and
 * after each loss, as every platform's GPU does. A device that answers is
 * "present" and told the core (`ready`); while none does the GPU is
 * "waiting", and it asks again every `RETRY_MS`. An ask waits while the core
 * is hidden, which draws nothing, and spends no try. Once `TRIES` asks are
 * spent, or a request fails, the GPU is given up: "lost" where a device has
 * answered before, and "missing" where none ever has. */
export class Acquirer {
  /** This counts the asks made since a device last answered, so an outage is
   * logged once as it starts and once as it ends. */
  private tries = 0;
  /** A device has answered before, so giving up now is losing the GPU, not
   * lacking one. */
  private had = false;

  private readonly p: Acquire;

  constructor(p: Acquire) {
    this.p = p;
  }

  /** Asks now. */
  request(): void {
    this.tries += 1;
    void this.p.acquire().then(
      (answered) => {
        if (!answered) {
          if (this.tries === 1) {
            console.warn(
              `[gpu] no device answered; asking every ${RETRY_MS} ms while the core is shown`,
            );
          }
          if (this.tries >= TRIES) {
            say(
              `[gpu] no device answered in ${this.tries} asks; nothing can be drawn`,
            );
            this.p.tell(this.had ? "lost" : "missing");
            return;
          }
          this.p.tell("waiting");
          return this.retry();
        }
        this.p.tell("present");
        this.had = true;
        if (this.tries > 1) {
          console.info(`[gpu] a device answered after ${this.tries} asks`);
        }
        this.tries = 0;
        this.p.ready();
      },
      (err) => {
        say(
          `[gpu] the device request failed; nothing can be drawn: ${String(err)}`,
        );
        this.p.tell(this.had ? "lost" : "missing");
      },
    );
  }

  /** The device that answered is lost, so the GPU waits and asks again. */
  lost(): void {
    this.p.tell("waiting");
    this.retry();
  }

  private retry(): void {
    setTimeout(
      () => (this.p.shown() ? this.request() : this.retry()),
      RETRY_MS,
    );
  }
}

/** This is a page's canvas, or one transferred to the worker its runtime runs
 * in. */
export type Canvas = HTMLCanvasElement | OffscreenCanvas;

/** The GPU is granted the core's memory, the offset where the core's stream
 * begins, and what the core asks. `ready` tells the core a device answered
 * (GPU_READY), `wake` asks for a turn, and `shown` says whether the UI shows
 * the core (`Link.shown`). */
export interface GpuGrant {
  mem: ArrayBuffer;
  at: u32;
  ask: GpuAsk;
  cfg: GpuConfig;
  ready(): void;
  wake(): void;
  shown(): boolean;
}

/** Grants `gpu` the core's memory and stream on `r`, the runtime beside it, its
 * commands on the program's `major`, and the program's settings `cfg`. */
export function gpu_grant(
  gpu: Gpu,
  r: Runtime,
  ask: GpuAsk,
  major: u32,
  cfg: GpuConfig,
): void {
  gpu.grant({
    mem: r.core.mem,
    at: r.head.stream_at,
    ask,
    cfg,
    ready: () => gpu_exports(r, major).gpu_ready(),
    wake: r.wake,
    shown: r.shown,
  });
}
