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
