// @pxl/client/abi/gpu.spec.ts
//
// GENERATED from spec/client/render/gpu/api.json
//
// pixelet's conventions over xpute's GPU calls (xpute/spec/gpu/api.json), which
// the renderer writes through xpute's encoder (gpu/api.rs): the stream it writes
// them into, its pipeline records' codes, and what a surface shows.

import * as surface_role from "./gpu/surface_role.spec.ts";
import * as stream from "./gpu/stream.spec.ts";

/** What a canvas the shell hands over shows (gpu/surface.rs): the world, or one thing previewed alone. */
export { surface_role };

/** The one place the renderer's calls cross (gpu/api.rs): a stream
 * (xpute-guest ipc/stream.rs) in a range of the kernel's memory, which the
 * kernel writes its calls into during a turn and the host runs once the turn
 * has returned.
 *
 * Its head is HEAD words, past the stream's END what the host writes for the
 * kernel: the device's block (xpute DEVICE) and the timed spans' reads, three
 * of them (gpu/api.rs `READS`), each its word, then each span's begin and end
 * ticks (gpu/api.rs lays them out and holds this number to it). */
export { stream };
