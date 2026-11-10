// @xpute/gpu/gpu.ts

/**
 * The GPU as a plugin of a core's runtime: what a platform's GPU is, and how it
 * is granted the core. @xpute/runtime knows none of it.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Arg } from "@xpute/kit/wire/xtp/mod.ts";
import type { Link, Runtime } from "@xpute/runtime/runtime.ts";
import { Command as GpuCommand, type GpuProtocol } from "./abi.spec.ts";

/** This is what a core asks of its GPU (its machine's `gpu`), by WebGPU's
 * names. */
export interface GpuAsk {
  power: string;
  features: { required: readonly string[]; optional: readonly string[] };
}

/** This is a platform's GPU. `grant` opens a device as the core asks. `watch`
 * reports when a device answers and whether there is one. */
export interface Gpu {
  grant(to: GpuGrant): void;
  watch(report: (state: GpuState) => void): void;
}

export type GpuState =
  "present" | "waiting" | "missing" | "lost" | "unsupported" | "insecure";

/** This is a page's canvas, or one transferred to the worker its runtime runs
 * in. */
export type Canvas = HTMLCanvasElement | OffscreenCanvas;

/** The GPU is granted the core's memory, the offset where the core's stream
 * begins, and what the core asks. `ready` tells the core a device answered
 * (GPU_READY), `wake` asks for a turn, and `shown` says whether the UI shows
 * the core (`Link.visible`). */
export interface GpuGrant {
  mem: ArrayBuffer;
  at: u32;
  ask: GpuAsk;
  ready(): void;
  wake(): void;
  shown(): boolean;
}

/** The GPU's commands over `link`, as the minors of the program's `major`. */
export function gpu_protocol(link: Link, major: u32): GpuProtocol {
  const cmd = (minor: u32): u32 => (major << 8) | minor;
  return {
    send: (c, ...args) => link.send(cmd(c), ...args),
    request: (c, ...args) => link.request(cmd(c), ...args),
    call: (c, ...args) => link.call(cmd(c), ...(args as Arg[])),
    fits: (c, ...args) => link.fits(cmd(c), ...args),
  };
}

/** Grants `gpu` the core's memory and stream on `r`, the runtime beside it, its
 * commands on the program's `major`. */
export function gpu_grant(gpu: Gpu, r: Runtime, ask: GpuAsk, major: u32): void {
  gpu.grant({
    mem: r.core.mem,
    at: r.head.stream_at,
    ask,
    ready: () => gpu_protocol(r, major).send(GpuCommand.GPU_READY),
    wake: r.wake,
    shown: r.shown,
  });
}
