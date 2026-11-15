// @pxl/client/abi/input.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type Drag,
  type PointerPhase,
  of_Drag,
  of_PointerPhase,
} from "./input.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5ee0f8;

/** `pointer` */
export const POINTER: Method = 0;
/** `wheel` */
export const WHEEL: Method = 1;
/** `pinch` */
export const PINCH: Method = 2;
/** `move` */
export const MOVE: Method = 3;
/** `xr-pose` */
export const XR_POSE: Method = 4;
/** `xr-pose-end` */
export const XR_POSE_END: Method = 5;

export function load_PointerPhase(r: Lift, at: number): PointerPhase {
  const v = of_PointerPhase(r.case(at, 5));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_Drag(r: Lift, at: number): Drag {
  const v = of_Drag(r.case(at, 3));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export async function pointer(
  link_: Sender,
  phase: PointerPhase,
  touch: boolean,
  id: number,
  drag: Drag,
  x: number,
  y: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, POINTER, [
    32,
    (w_) => {
      w_.u8(0, phase);
      w_.bool(1, touch);
      w_.i32(4, id);
      w_.u8(8, drag);
      w_.f64(16, x);
      w_.f64(24, y);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function wheel(link_: Sender, delta_y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, WHEEL, [
    8,
    (w_) => {
      w_.f64(0, delta_y);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function pinch(link_: Sender, scale: number): Promise<void> {
  const a_ = await link_.send(HANDLE, PINCH, [
    8,
    (w_) => {
      w_.f64(0, scale);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function move(link_: Sender, x: number, y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, MOVE, [
    8,
    (w_) => {
      w_.i32(0, x);
      w_.i32(4, y);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function xr_pose(
  link_: Sender,
  x: number,
  y: number,
  z: number,
  qx: number,
  qy: number,
  qz: number,
  qw: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, XR_POSE, [
    56,
    (w_) => {
      w_.f64(0, x);
      w_.f64(8, y);
      w_.f64(16, z);
      w_.f64(24, qx);
      w_.f64(32, qy);
      w_.f64(40, qz);
      w_.f64(48, qw);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function xr_pose_end(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, XR_POSE_END, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
