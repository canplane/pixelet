// @pxl/client/runtime/page.ts

/**
 * What crosses between a page's shell and the kernel's worker (runtime/worker.ts)
 * besides the kernel's frames, which go over xpute's remote of their own
 * (@xpute/runtime remote.ts): the start, what the shell asks of the runtime, a
 * canvas lent, and what the worker's devices say for the shell to show.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import type { Abort } from "@xpute/runtime";
import type { GpuState } from "@xpute/gpu/web.ts";
import type { Source } from "@/abi/state.ts";
import type { Declaration } from "@/sys/fs/source.ts";

/** The shell to the worker. */
export type ToKernel =
  /** The frames' port, and the page's kept keys, which a worker cannot read. */
  | { kind: "start"; frames: MessagePort; keys: [string, string][] }
  | { kind: "resume" }
  | { kind: "pause" }
  | { kind: "wake" }
  | { kind: "input" }
  | { kind: "visible"; visible: boolean }
  | { kind: "cmd"; cmd: u32; args: NodeValue<NodeView>[] }
  | { kind: "bind"; handle: u32; canvas: OffscreenCanvas }
  | { kind: "unbind"; handle: u32 }
  /** A profiler's reading (port/perf.ts `read`), by its `id`. */
  | { kind: "perf"; id: u32; what: string; args: unknown[] };

/** The worker to the shell. */
export type ToShell =
  | { kind: "started" }
  /** Settled: a GPU answered and the source is declared, or refused. */
  | { kind: "opened"; declared: boolean }
  | { kind: "failed"; why: string }
  | { kind: "gpu"; state: GpuState }
  | { kind: "online"; online: boolean }
  | { kind: "source"; state: Source; declaration: Declaration | null }
  | { kind: "abort"; report: Abort }
  /** A key the worker wrote, for the page to keep. */
  | { kind: "keep"; key: string; value: string }
  | { kind: "perf"; id: u32; value: unknown };
