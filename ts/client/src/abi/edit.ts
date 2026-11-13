// @pxl/client/abi/edit.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/edit` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x453522;

/** `hold` */
export const HOLD: Method = 0;
/** `marker-text` */
export const MARKER_TEXT: Method = 1;
/** `marker-cancel` */
export const MARKER_CANCEL: Method = 2;
/** `marker-commit` */
export const MARKER_COMMIT: Method = 3;

export const Tool = {
  Pointer: 0,
  Item: 1,
  Eraser: 2,
} as const;
export type Tool = (typeof Tool)[keyof typeof Tool];

/** The name of the `Tool` member numbered `n`, or undefined for a number
 * no member has. */
export function tool_name(n: number): keyof typeof Tool | undefined {
  for (const k in Tool)
    if (Tool[k as keyof typeof Tool] === n) return k as keyof typeof Tool;
  return undefined;
}

/** The `Tool` member `n` is, or null for a number no member has. */
export function tool_of(n: number): Tool | null {
  return tool_name(n) === undefined ? null : (n as Tool);
}

export async function hold(
  link_: Sender,
  tool: Tool,
  val: number,
  scale_steps: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, HOLD, [
    3,
    (w_) => {
      w_.u32(tool);
      w_.u32(val);
      w_.i32(scale_steps);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function marker_text(link_: Sender, text: string): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_TEXT, [
    1,
    (w_) => {
      w_.str(text);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function marker_cancel(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_CANCEL, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function marker_commit(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_COMMIT, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}
