// @pxl/client/abi/edit.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { type Tool, of_Tool } from "./edit.enum.ts";
import { load_ErrorCode } from "./types.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  ComponentError,
  type Handle,
  type Method,
  type Sender,
} from "@xpute/link/link.ts";

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

export function load_Tool(r: Lift, at: number): Tool {
  const v = of_Tool(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export async function hold(
  link_: Sender,
  tool: Tool,
  val: number,
  scale_step: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, HOLD, [
    12,
    (w_) => {
      w_.u8(0, tool);
      w_.u32(4, val);
      w_.i32(8, scale_step);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function marker_text(link_: Sender, text: string): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_TEXT, [
    8,
    (w_) => {
      w_.str(0, text);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  const v_ =
    r_.case(0, 2) === 0
      ? { tag: "ok" as const }
      : { tag: "err" as const, val: load_ErrorCode(r_, 1) };
  if (v_.tag === "err") throw new ComponentError(v_.val);
  return;
}

export async function marker_cancel(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_CANCEL, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function marker_commit(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, MARKER_COMMIT, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
