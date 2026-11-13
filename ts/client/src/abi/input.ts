// @pxl/client/abi/input.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/input` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5ee0f8;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "input");

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
  Cancel: 4,
} as const;
export type PointerPhase = (typeof PointerPhase)[keyof typeof PointerPhase];

/** The name of the `PointerPhase` member numbered `n`, or undefined for a number
 * no member has. */
export function pointer_phase_name(
  n: number,
): keyof typeof PointerPhase | undefined {
  return (Object.keys(PointerPhase) as (keyof typeof PointerPhase)[]).find(
    (k) => PointerPhase[k] === n,
  );
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
  return (Object.keys(PointerType) as (keyof typeof PointerType)[]).find(
    (k) => PointerType[k] === n,
  );
}

/** The `PointerType` member `n` is, or null for a number no member has. */
export function pointer_type_of(n: number): PointerType | null {
  return pointer_type_name(n) === undefined ? null : (n as PointerType);
}

export interface Input {
  pointer(
    phase: PointerPhase,
    pointer_type: PointerType,
    id: number,
    button: number,
    x: number,
    y: number,
  ): void;
  typing(typing: boolean): void;
  wheel(delta_y: number): void;
  pinch(scale: number): void;
  move(x: number, y: number): void;
  xr_pose(
    x: number,
    y: number,
    z: number,
    qx: number,
    qy: number,
    qz: number,
    qw: number,
  ): void;
  xr_pose_end(): void;
}

/** The object over `imp`. */
function object(imp: Input): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case POINTER: {
            if (args === null) return Errno.EBADMSG;
            imp.pointer(
              args.at(0).get_number() as PointerPhase,
              args.at(1).get_number() as PointerType,
              args.at(2).get_number(),
              args.at(3).get_number(),
              args.at(4).get_number(),
              args.at(5).get_number(),
            );
            return 0;
          }
          case TYPING: {
            if (args === null) return Errno.EBADMSG;
            imp.typing(args.at(0).get_bool());
            return 0;
          }
          case WHEEL: {
            if (args === null) return Errno.EBADMSG;
            imp.wheel(args.at(0).get_number());
            return 0;
          }
          case PINCH: {
            if (args === null) return Errno.EBADMSG;
            imp.pinch(args.at(0).get_number());
            return 0;
          }
          case MOVE: {
            if (args === null) return Errno.EBADMSG;
            imp.move(args.at(0).get_number(), args.at(1).get_number());
            return 0;
          }
          case XR_POSE: {
            if (args === null) return Errno.EBADMSG;
            imp.xr_pose(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
              args.at(3).get_number(),
              args.at(4).get_number(),
              args.at(5).get_number(),
              args.at(6).get_number(),
            );
            return 0;
          }
          case XR_POSE_END: {
            imp.xr_pose_end();
            return 0;
          }
          default:
            return Errno.ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Input): void {
  l.mount_named(HANDLE, object(imp));
}

export async function pointer(
  link: Sender,
  phase: PointerPhase,
  pointer_type: PointerType,
  id: number,
  button: number,
  x: number,
  y: number,
): Promise<void> {
  const a = await link.send(HANDLE, POINTER, [
    phase,
    pointer_type,
    id,
    button,
    x,
    y,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function typing(link: Sender, typing: boolean): Promise<void> {
  const a = await link.send(HANDLE, TYPING, [typing]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function wheel(link: Sender, delta_y: number): Promise<void> {
  const a = await link.send(HANDLE, WHEEL, [delta_y]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function pinch(link: Sender, scale: number): Promise<void> {
  const a = await link.send(HANDLE, PINCH, [scale]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function move(link: Sender, x: number, y: number): Promise<void> {
  const a = await link.send(HANDLE, MOVE, [x, y]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function xr_pose(
  link: Sender,
  x: number,
  y: number,
  z: number,
  qx: number,
  qy: number,
  qz: number,
  qw: number,
): Promise<void> {
  const a = await link.send(HANDLE, XR_POSE, [x, y, z, qx, qy, qz, qw]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function xr_pose_end(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, XR_POSE_END, []);
  if (a.result < 0) throw new XputeError(a.result);
}
