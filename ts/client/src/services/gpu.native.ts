// @pxl/client/services/gpu.native.ts

/** The app's GPU (@xpute/gpu `NativeGpu`): the device the kernel draws with,
 * opened in Rust as the kernel's machine asks (xpute-gpu host/native.rs
 * `native_guest!`, `Ask::of`), what refused it in the native log
 * (`say_refused`). */

import { NativeGpu } from "@xpute/gpu/native.ts";

export const gpu = new NativeGpu();
