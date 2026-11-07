// @pxl/client/runtime/page.ts

/**
 * What crosses between a page's shell and the kernel's worker besides xpute's
 * port: what a worker cannot read of the page, what the worker keeps, the
 * files the kernel refused, the source as the worker has it, and the
 * profiler's reads.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import type { Source } from "@/abi/state.ts";
import type { Declaration } from "@/sys/fs/source.ts";

/** What a worker cannot read of the page, sent first: its kept keys, and
 * whether it is held in the hand (service/device.ts). */
export interface Page {
  kind: "page";
  keys: [string, string][];
  handheld: boolean;
}

/** The shell to the worker. */
export type ToKernel =
  | Page
  | { kind: "refused"; files: readonly RefusedFile[] }
  | { kind: "perf"; id: u32; what: string; args: unknown[] };

/** The worker to the shell. */
export type ToShell =
  | { kind: "source"; state: Source; declaration: Declaration | null }
  | { kind: "keep"; key: string; value: string }
  | { kind: "perf"; id: u32; value: unknown };
