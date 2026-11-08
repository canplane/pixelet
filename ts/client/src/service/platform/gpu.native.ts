// @pxl/client/service/platform/gpu.native.ts

/** Rust opens this device as the core's machine asks (xpute-gpu
 * runtime/native.rs `native_core!` and `Ask::of`), and writes what refused it
 * to the native log (`say_refused`). */

import { NativeGpu } from "@xpute/gpu/native.ts";

export const gpu = new NativeGpu();
