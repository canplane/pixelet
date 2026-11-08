// @pxl/client/runtime/page.ts

/**
 * What crosses between a page's UI and the core's worker besides xpute's
 * port: what a worker cannot read of the page, what the worker keeps in
 * storage, the files the core refused, the source as the worker has it, and
 * the profiler's reads.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { RefusedFile } from "@/abi/cmd.spec.ts";
import type { Source } from "@/abi/state.ts";
import type { Declaration } from "@/service/sys/fs/source.ts";

/** What a worker cannot read of the page, sent before anything else: its kept
 * keys, and whether the device is held in the hand
 * (service/platform/device.ts). */
export interface Page {
  kind: "page";
  keys: [string, string][];
  handheld: boolean;
}

/** What the UI sends the worker. */
export type ToCore =
  | Page
  | { kind: "refused"; files: readonly RefusedFile[] }
  | { kind: "perf"; id: u32; what: string; args: unknown[] };

/** What the worker sends the UI. */
export type ToUi =
  | { kind: "source"; state: Source; declaration: Declaration | null }
  | { kind: "keep"; key: string; value: string }
  | { kind: "perf"; id: u32; value: unknown };
