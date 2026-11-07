// @pxl/client/runtime/page.ts

/**
 * What crosses between a page's shell and the kernel's worker besides xpute's
 * port: the page's kept keys, what the worker keeps, the source as the worker
 * has it, and the profiler's reads.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Source } from "@/abi/state.ts";
import type { Declaration } from "@/sys/fs/source.ts";

/** The shell to the worker. */
export type ToKernel =
  | { kind: "keys"; keys: [string, string][] }
  | { kind: "perf"; id: u32; what: string; args: unknown[] };

/** The worker to the shell. */
export type ToShell =
  | { kind: "source"; state: Source; declaration: Declaration | null }
  | { kind: "keep"; key: string; value: string }
  | { kind: "perf"; id: u32; value: unknown };
