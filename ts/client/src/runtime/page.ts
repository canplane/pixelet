// @pxl/client/runtime/page.ts

/** These messages cross between a page's UI and the core's worker, beside
 * xpute's port. */

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

export type ToCore =
  | Page
  | { kind: "refused"; files: readonly RefusedFile[] }
  | { kind: "perf"; id: u32; what: string; args: unknown[] };

export type ToUi =
  | { kind: "source"; state: Source; decl: Declaration | null }
  | { kind: "keep"; key: string; val: string }
  | { kind: "perf"; id: u32; val: unknown };
