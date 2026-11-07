// @xpute/runtime/abi/gpu.spec.ts
//
// GENERATED from spec/abi/gpu.json

/**
 * The GPU device's protocol, on major GPU: its state and the surfaces a UI lends it. The
 * calls a core makes to the GPU are records of its stream (gpu/api.json), not commands.
 * GPU_READY tells the core the device the machine asked for answered. A surface is a
 * canvas or a view the UI hands over and the core alone draws from then on, by the handle
 * GPU_SURFACE_OPEN answers, the handle the UI binds the canvas under (@xpute/runtime
 * `Link.bind`) and the stream's CONFIGURE and CURRENT name. What it shows is the program's.
 */

import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./gpu/major.spec.ts";
import * as GpuMinor from "./gpu/gpu_minor.spec.ts";
import * as Command from "./gpu/command.spec.ts";
import type { Sender } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of GpuMinor's, as its enum was a type. */
type GpuMinor = (typeof GpuMinor)[keyof typeof GpuMinor];
export { GpuMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.GPU_SURFACE_RESIZE's packet, written in place into `to`. */
export function encode_gpu_surface_resize(
  to: PacketRoom,
  surface: u32,
  width: u32,
  height: u32,
  device_pixel_ratio: f64,
): PacketWriter {
  return to
    .writer(4)
    .u32(surface)
    .u32(width)
    .u32(height)
    .f64(device_pixel_ratio);
}

/** Command.GPU_SURFACE_CLOSE's packet, written in place into `to`. */
export function encode_gpu_surface_close(
  to: PacketRoom,
  surface: u32,
): PacketWriter {
  return to.writer(1).u32(surface);
}

/** What the runtime sends on the gpu protocol, bound to `runtime`: each
 * command sent, requested and answered later, or called and answered before
 * it returns. */
export function gpu_exports(runtime: Sender) {
  return {
    gpu_ready: (): void => runtime.send(Command.GPU_READY),
    gpu_surface_open: (): Promise<i32> =>
      runtime.request(Command.GPU_SURFACE_OPEN),
    gpu_surface_resize: (
      surface: u32,
      width: u32,
      height: u32,
      device_pixel_ratio: f64,
    ): void =>
      runtime.send(Command.GPU_SURFACE_RESIZE, (r) =>
        encode_gpu_surface_resize(
          r,
          surface,
          width,
          height,
          device_pixel_ratio,
        ),
      ),
    gpu_surface_close: (surface: u32): void =>
      runtime.send(Command.GPU_SURFACE_CLOSE, (r) =>
        encode_gpu_surface_close(r, surface),
      ),
  };
}

/** The gpu protocol's stubs, bound. */
export type GpuExports = ReturnType<typeof gpu_exports>;
