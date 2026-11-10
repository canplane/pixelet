// @pxl/client/service/platform/gpu.native.ts

/** Rust opens this device as the core's machine asks (xpute-gpu
 * runtime/native.rs `native_core!` and `Ask::of`), and writes what refused it
 * to the native log (`say_refused`). */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { NativeGpu } from "@xpute/gpu/native.ts";

export const gpu = new NativeGpu();

export const after_turn = undefined;

/** A device's view binds its surface itself, so it lends no canvas. */
export const canvas_bind = (_handle: u32, _canvas: unknown): never => {
  throw new InvariantError(Errno.ENOTSUP);
};
export const canvas_unbind = (_handle: u32): never => {
  throw new InvariantError(Errno.ENOTSUP);
};
