// @pxl/client/abi/input.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type Handle,
  type Method,
  type Sender,
  answer,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/input` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5ee0f8;

/** `pan` */
export const PAN: Method = 0;
/** `rotate` */
export const ROTATE: Method = 1;
/** `tilt` */
export const TILT: Method = 2;
/** `zoom` */
export const ZOOM: Method = 3;
/** `drag` */
export const DRAG: Method = 4;
/** `tap` */
export const TAP: Method = 5;
/** `hover` */
export const HOVER: Method = 6;
/** `out` */
export const OUT: Method = 7;
/** `move` */
export const MOVE: Method = 8;

export async function pan(
  link_: Sender,
  dx: number,
  dy: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, PAN, [
    16,
    (w_) => {
      w_.f64(0, dx);
      w_.f64(8, dy);
    },
  ]);
  answer(a_);
}

export async function rotate(link_: Sender, rad: number): Promise<void> {
  const a_ = await link_.send(HANDLE, ROTATE, [
    8,
    (w_) => {
      w_.f64(0, rad);
    },
  ]);
  answer(a_);
}

export async function tilt(link_: Sender, rad: number): Promise<void> {
  const a_ = await link_.send(HANDLE, TILT, [
    8,
    (w_) => {
      w_.f64(0, rad);
    },
  ]);
  answer(a_);
}

export async function zoom(link_: Sender, scale: number): Promise<void> {
  const a_ = await link_.send(HANDLE, ZOOM, [
    8,
    (w_) => {
      w_.f64(0, scale);
    },
  ]);
  answer(a_);
}

export async function drag(link_: Sender, dragging: boolean): Promise<void> {
  const a_ = await link_.send(HANDLE, DRAG, [
    1,
    (w_) => {
      w_.bool(0, dragging);
    },
  ]);
  answer(a_);
}

export async function tap(link_: Sender, x: number, y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, TAP, [
    16,
    (w_) => {
      w_.f64(0, x);
      w_.f64(8, y);
    },
  ]);
  answer(a_);
}

export async function hover(
  link_: Sender,
  x: number,
  y: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, HOVER, [
    16,
    (w_) => {
      w_.f64(0, x);
      w_.f64(8, y);
    },
  ]);
  answer(a_);
}

export async function out(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, OUT, null);
  answer(a_);
}

export async function move(link_: Sender, x: number, y: number): Promise<void> {
  const a_ = await link_.send(HANDLE, MOVE, [
    8,
    (w_) => {
      w_.i32(0, x);
      w_.i32(4, y);
    },
  ]);
  answer(a_);
}
