// @xpute/gpu/abi.spec.ts
//
// GENERATED from spec/abi/gpu.json

/**
 * The GPU device's protocol, which tells the core the device's state and the
 * surfaces the UI lends it. The core's own calls to the GPU are not commands: they are
 * records in its GPU stream (gpu/api.json). GPU_READY tells the core that the device its
 * core asked for has answered. A surface is a canvas or a view that the UI hands over,
 * and from then on only the core draws into it. The surface is named by the handle that
 * GPU_SURFACE_OPEN answers with: the UI binds its canvas under that handle (@xpute/gpu
 * `gpu_connect`), and the stream's CONFIGURE and CURRENT records name the surface by it. What
 * a surface shows is the program's to decide. A command's number here is its minor, under the
 * one major the program gives the GPU (xpute_gpu `abi::apply`, @xpute/gpu `gpu_exports`).
 */

import * as Command from "./abi/command.spec.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import type { Link } from "@xpute/runtime";
import { CMD } from "@xpute/runtime";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** The gpu protocol's exports as the UI calls them over `link`: a send returns
 * nothing, and a request a Promise of its reply's value, or of the memory its reply
 * names, which rejects with the errno it was refused with. `fits` says whether a
 * send's packet fits a frame. */
export function gpu_exports(link: Link, major: u32) {
  return {
    /** The GPU the core asked for has answered, for the first time or again after the device
     * was lost. The STATUS word of the stream's device block tells the core the device's state. */
    gpu_ready: () => link.send(CMD(major, Command.GPU_READY)),
    /** Asks the core to open a surface. The core answers with the surface's handle, or with 0
     * when it opens none. */
    gpu_surface_open: () =>
      link.request(CMD(major, Command.GPU_SURFACE_OPEN)).then((r) => r.val),
    /** The surface's layout box in device pixels, ignoring transforms, and the device pixel
     * ratio. The size of the buffer the surface is drawn into is the core's to choose. */
    gpu_surface_resize: (
      surface: u32,
      width: u32,
      height: u32,
      device_pixel_ratio: f64,
    ) =>
      link.send(
        CMD(major, Command.GPU_SURFACE_RESIZE),
        surface,
        width,
        height,
        device_pixel_ratio,
      ),
    /** The surface is going away, and the core lets go of what it drew and held on the device
     * for it. */
    gpu_surface_close: (surface: u32) =>
      link.send(CMD(major, Command.GPU_SURFACE_CLOSE), surface),
    fits: {
      gpu_ready: () => link.fits(CMD(major, Command.GPU_READY)),
      gpu_surface_resize: (
        surface: u32,
        width: u32,
        height: u32,
        device_pixel_ratio: f64,
      ) =>
        link.fits(
          CMD(major, Command.GPU_SURFACE_RESIZE),
          surface,
          width,
          height,
          device_pixel_ratio,
        ),
      gpu_surface_close: (surface: u32) =>
        link.fits(CMD(major, Command.GPU_SURFACE_CLOSE), surface),
    },
  };
}

/** The gpu protocol's exports, as `gpu_exports` binds them. */
export type GpuExports = ReturnType<typeof gpu_exports>;
