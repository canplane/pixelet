// @pxl/client/abi/input.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  open as open_uri,
  refusal_of,
  type Ref,
  type Sender,
  uri,
} from "@xpute/runtime";

/** The singleton's URI. */
export const URI = uri("pxl:core", "input");

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

export const PointerType = {
  Mouse: 0,
  Touch: 1,
  Pen: 2,
} as const;
export type PointerType = (typeof PointerType)[keyof typeof PointerType];

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

/** Mounts `imp` at the URI, one object for every open. */
export function mount(link: Pick<Link, "mount">, imp: Input): void {
  const o = object(imp);
  link.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function pointer(
  ref: Ref,
  phase: PointerPhase,
  pointer_type: PointerType,
  id: number,
  button: number,
  x: number,
  y: number,
): Promise<void> {
  const a = await ref.link.send(ref.handle, POINTER, [
    phase,
    pointer_type,
    id,
    button,
    x,
    y,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function typing(ref: Ref, typing: boolean): Promise<void> {
  const a = await ref.link.send(ref.handle, TYPING, [typing]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function wheel(ref: Ref, delta_y: number): Promise<void> {
  const a = await ref.link.send(ref.handle, WHEEL, [delta_y]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function pinch(ref: Ref, scale: number): Promise<void> {
  const a = await ref.link.send(ref.handle, PINCH, [scale]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function move(ref: Ref, x: number, y: number): Promise<void> {
  const a = await ref.link.send(ref.handle, MOVE, [x, y]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function xr_pose(
  ref: Ref,
  x: number,
  y: number,
  z: number,
  qx: number,
  qy: number,
  qz: number,
  qw: number,
): Promise<void> {
  const a = await ref.link.send(ref.handle, XR_POSE, [x, y, z, qx, qy, qz, qw]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function xr_pose_end(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, XR_POSE_END, []);
  if (a.result < 0) throw new XputeError(a.result);
}
