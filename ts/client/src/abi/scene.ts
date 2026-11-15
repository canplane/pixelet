// @pxl/client/abi/scene.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { type SceneContent, of_SceneContent } from "./scene.enum.ts";
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

/** The interface's handle, named by `pxl:core/scene` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x7c289a;

/** `draw` */
export const DRAW: Method = 0;
/** `chat` */
export const CHAT: Method = 1;

export function load_SceneContent(r: Lift, at: number): SceneContent {
  const v = of_SceneContent(r.case(at, 2));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export async function draw(
  link_: Sender,
  surface: number,
  content: SceneContent,
  group: number | null,
  bg: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, DRAW, [
    16,
    (w_) => {
      w_.u32(0, surface);
      w_.u8(4, content);
      if (group !== null) {
        w_.u8(5, 1);
        w_.u8(6, group);
      }
      if (bg !== null) {
        w_.u8(8, 1);
        w_.u32(12, bg);
      }
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

export async function chat(link_: Sender, text: string): Promise<void> {
  const a_ = await link_.send(HANDLE, CHAT, [
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
