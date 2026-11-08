// @xpute/runtime/abi/gpu.spec.ts
//
// GENERATED from spec/abi/gpu.json

/**
 * The GPU device's protocol on major GPU, which tells the core the device's state and the
 * surfaces the UI lends it. The core's own calls to the GPU are not commands: they are
 * records in its GPU stream (gpu/api.json). GPU_READY tells the core that the device its
 * machine asked for has answered. A surface is a canvas or a view that the UI hands over,
 * and from then on only the core draws into it. The surface is named by the handle that
 * GPU_SURFACE_OPEN answers with: the UI binds its canvas under that handle (@xpute/runtime
 * `Link.bind`), and the stream's CONFIGURE and CURRENT records name the surface by it. What
 * a surface shows is the program's to decide.
 */

import * as Major from "./gpu/major.spec.ts";
import * as GpuMinor from "./gpu/gpu_minor.spec.ts";
import * as Command from "./gpu/command.spec.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import type { Protocol } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of GpuMinor's, as its enum was a type. */
type GpuMinor = (typeof GpuMinor)[keyof typeof GpuMinor];
export { GpuMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** What the runtime sends on the gpu protocol: each command by its number, and
 * the fields its packet takes in order. It is sent, requested and answered
 * later, or called and answered before it returns. */
export type GpuSend = {
  [Command.GPU_READY]: [];
  [Command.GPU_SURFACE_RESIZE]: [
    surface: u32,
    width: u32,
    height: u32,
    device_pixel_ratio: f64,
  ];
  [Command.GPU_SURFACE_CLOSE]: [surface: u32];
};
export type GpuRequest = {
  [Command.GPU_SURFACE_OPEN]: [];
};
export type GpuCall = Record<never, never>;

/** The gpu protocol over a link. */
export type GpuProtocol = Protocol<GpuSend, GpuRequest, GpuCall>;
