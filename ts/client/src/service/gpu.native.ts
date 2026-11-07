// @pxl/client/service/gpu.native.ts

/** The app's GPU (@xpute/gpu `NativeGpu`): the device the core draws with,
 * opened in Rust as the core's machine asks (xpute-gpu runtime/native.rs
 * `native_core!`, `Ask::of`), what refused it in the native log
 * (`say_refused`). */

import { NativeGpu } from "@xpute/gpu/native.ts";

export const gpu = new NativeGpu();
