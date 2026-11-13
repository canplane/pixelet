// @pxl/client/abi/input.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/input` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5ee0f8;

/** `pointer` */
export const POINTER: Method = 0;
/** `typing` */
export const TYPING: Method = 1;
/** `wheel` */
export const WHEEL: Method = 2;
/** `pinch` */
export const PINCH: Method = 3;
/** `move` */
export const MOVE: Method = 4;
/** `xr-pose` */
export const XR_POSE: Method = 5;
/** `xr-pose-end` */
export const XR_POSE_END: Method = 6;

export const PointerPhase = {
  Down: 0,
  Move: 1,
  Up: 2,
  Out: 3,
  /** The pointer's gesture ends with no tap: the platform took it, or the UI
   * spent it on something of its own. */
  Cancel: 4,
} as const;
export type PointerPhase = (typeof PointerPhase)[keyof typeof PointerPhase];

/** The name of the `PointerPhase` member numbered `n`, or undefined for a number
 * no member has. */
export function pointer_phase_name(
  n: number,
): keyof typeof PointerPhase | undefined {
  for (const k in PointerPhase)
    if (PointerPhase[k as keyof typeof PointerPhase] === n)
      return k as keyof typeof PointerPhase;
  return undefined;
}

/** The `PointerPhase` member `n` is, or null for a number no member has. */
export function pointer_phase_of(n: number): PointerPhase | null {
  return pointer_phase_name(n) === undefined ? null : (n as PointerPhase);
}

export const PointerType = {
  Mouse: 0,
  Touch: 1,
  Pen: 2,
} as const;
export type PointerType = (typeof PointerType)[keyof typeof PointerType];

/** The name of the `PointerType` member numbered `n`, or undefined for a number
 * no member has. */
export function pointer_type_name(
  n: number,
): keyof typeof PointerType | undefined {
  for (const k in PointerType)
    if (PointerType[k as keyof typeof PointerType] === n)
      return k as keyof typeof PointerType;
  return undefined;
}

/** The `PointerType` member `n` is, or null for a number no member has. */
export function pointer_type_of(n: number): PointerType | null {
  return pointer_type_name(n) === undefined ? null : (n as PointerType);
}

export async function pointer(
  link_: Sender,
  phase: PointerPhase,
  pointer_type: PointerType,
  id: number,
  button: number,
  x: number,
  y: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, POINTER, [
    6,
    (w_) => {
      w_.u32(phase);
      w_.u32(pointer_type);
      w_.i32(id);
      w_.i32(button);
      w_.f64(x);
      w_.f64(y);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function typing(link_: Sender, typing: boolean): Promise<void> {
  const a_ = await link_.send(HANDLE, TYPING, [
    1,
    (w_) => {
      w_.bool(typing);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function wheel(link_: Sender, delta_y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, WHEEL, [
    1,
    (w_) => {
      w_.f64(delta_y);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function pinch(link_: Sender, scale: number): Promise<void> {
  const a_ = await link_.send(HANDLE, PINCH, [
    1,
    (w_) => {
      w_.f64(scale);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function move(link_: Sender, x: number, y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, MOVE, [
    2,
    (w_) => {
      w_.i32(x);
      w_.i32(y);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
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
    7,
    (w_) => {
      w_.f64(x);
      w_.f64(y);
      w_.f64(z);
      w_.f64(qx);
      w_.f64(qy);
      w_.f64(qz);
      w_.f64(qw);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function xr_pose_end(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, XR_POSE_END, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}
